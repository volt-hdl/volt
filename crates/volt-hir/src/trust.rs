//! Bilgi akışı denetimi — güven seviyeleri (ADR-0052,
//! docs/spec/domain-inference.md K11).
//!
//! Domain'in dördüncü boyutu: `trust_level = secret | confidential |
//! public`. Bu geçit saat çıkarımından SONRA koşar ve onun sonucunu
//! kullanır: her sinyalin güven seviyesi ALANININ trust_level'ıdır (saat
//! için K1/K2/K4 neyse güven için de o); bir ifade operandlarının en
//! yükseğini taşır (K5'in kafes eşleniği); etiket atamalar, `let`
//! bağlamaları, `if`/`match` koşulları (örtük akış) ve örnek portları
//! boyunca izlenir (K6-K8); `sync()` etiketi korur (K9 saati değiştirir,
//! güveni değil). Yüksekten düşüğe akış E3009; tek meşru yol
//! `declassify(expr, "gerekçe")` — her çağrı W3008 iz kaydı bırakır.
//!
//! Alanı trust_level taşımayan sinyal SINIFLANDIRILMAMIŞTIR: kendisine
//! yazılan en yüksek seviyeyi alır (sabit nokta), böylece anotasyonsuz
//! bir register ya da alt modül bir sırrı aklayamaz. Dosyada hiç
//! trust_level ve declassify yoksa geçit hiç koşmaz — davranış değişmez
//! (UX Anayasası: yazılmayan şey yok sayılır).

use std::collections::{HashMap, HashSet};

use volt_ast::{
    Block, BlockStmt, ElseBranch, Expr, ExprKind, Idx, IfStmt, ItemKind, LValue, LValueSuffix,
    MatchArmBody, ModuleDecl, Port, PortDir, SourceFile, Stmt, StmtKind, TrustLevel,
};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use crate::builtin::PortKind;
use crate::domain::{DomainId, DomainResult};
use crate::resolve::{BuiltinKind, DefId, DefKind, ResolveResult};
use crate::ty::Ty;
use crate::typeck::TypeckResult;

/// Sabit nokta üst sınırı: kafes yüksekliği 3, her tur en az bir sinyali
/// yükseltir; pratikte 2-3 turda biter.
const MAX_ITERATIONS: usize = 64;

/// Bir değerin taşıdığı güven etiketi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Tag {
    level: TrustLevel,
    /// Tanık: seviyeyi taşıyan (ilk) alt ifade — E3009 ikincil etiketi.
    span: Span,
    /// Seviyeyi veren domain (`DomainResult::domains` indeksi); çıkarımla
    /// yayılmış etiketlerde kaynağın domain'i, `declassify` sonucunda yok.
    origin: Option<u32>,
}

/// Kafes birleşimi: en yüksek seviye kazanır, eşitlikte soldaki (ilk
/// tanık) kalır.
fn join(a: Option<Tag>, b: Option<Tag>) -> Option<Tag> {
    match (a, b) {
        (None, x) | (x, None) => x,
        (Some(x), Some(y)) => Some(if y.level > x.level { y } else { x }),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Sınıflandırılmamış sinyallerin etiketlerini biriktir (sabit nokta).
    Infer,
    /// Sınıflandırılmış hedeflere akışı denetle, E3009/W3008 üret.
    Check,
}

/// Dosyadaki tüm modüllerde bilgi akışını denetler.
pub fn check_trust(
    ast: &SourceFile,
    res: &ResolveResult,
    tyck: &TypeckResult,
    domain: &DomainResult,
) -> Vec<Diagnostic> {
    let has_trust = domain.domains.iter().any(|d| d.trust.is_some());
    if !has_trust && ast.trust.declassify.is_empty() {
        return Vec::new();
    }
    let mut c = Checker {
        ast,
        res,
        tyck,
        domain,
        declared: HashMap::new(),
        inferred: HashMap::new(),
        instance_tags: HashMap::new(),
        mode: Mode::Infer,
        changed: false,
        diagnostics: Vec::new(),
    };
    c.collect_declared();
    for _ in 0..MAX_ITERATIONS {
        c.changed = false;
        c.walk_file();
        if !c.changed {
            break;
        }
    }
    c.mode = Mode::Check;
    c.walk_file();
    c.report_declassify();
    c.diagnostics
}

struct Checker<'a> {
    ast: &'a SourceFile,
    res: &'a ResolveResult,
    tyck: &'a TypeckResult,
    domain: &'a DomainResult,
    /// Alanı sınıflandırılmış sinyal → (seviye, domain indeksi).
    declared: HashMap<DefId, (TrustLevel, u32)>,
    /// Sınıflandırılmamış sinyal → yazılan en yüksek etiket.
    inferred: HashMap<DefId, Tag>,
    /// Örnek → sınıflandırılmış giriş bağlamalarının en yükseği
    /// (sınıflandırılmamış çıkışlar için tutucu özet).
    instance_tags: HashMap<DefId, Tag>,
    mode: Mode,
    changed: bool,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Checker<'a> {
    // ═══ Yardımcılar ══════════════════════════════════════════════

    fn decl_def(&self, span: Span) -> Option<DefId> {
        self.res.decl_spans.get(&span).copied()
    }

    fn use_def(&self, span: Span) -> Option<DefId> {
        self.res.use_spans.get(&span).copied()
    }

    fn is_clock_def(&self, def: DefId) -> bool {
        self.tyck
            .def_types
            .get(&def)
            .is_some_and(|&t| matches!(self.tyck.types.ty(t), Ty::Clock))
    }

    fn domain_trust(&self, idx: u32) -> Option<(TrustLevel, u32)> {
        self.domain.domains[idx as usize].trust.map(|t| (t, idx))
    }

    /// Sinyalin saat alanının güven seviyesi (K2/K4 sonucu üzerinden).
    fn domain_trust_of_def(&self, def: DefId) -> Option<(TrustLevel, u32)> {
        match self.domain.signal_domains.get(&def) {
            Some(DomainId::Explicit(idx)) => self.domain_trust(*idx),
            _ => None,
        }
    }

    /// `@Ad` anotasyonunun güven seviyesi: domain bildirimi (K1) ya da
    /// clock portu (`@clk`) üzerinden o portun alanı.
    fn annotation_trust(&self, ann_span: Span) -> Option<(TrustLevel, u32)> {
        let def = self.use_def(ann_span)?;
        match self.res.def_kind(def) {
            DefKind::Domain => self
                .domain
                .decl_domains
                .get(&def)
                .and_then(|&idx| self.domain_trust(idx)),
            DefKind::Port { .. } => self.domain_trust_of_def(def),
            _ => None,
        }
    }

    /// Portun bildirilmiş seviyesi: anotasyon, yoksa alanı; extern
    /// portlarında alan kaydı yoktur — tek clock portunun anotasyonu.
    fn port_declared(&self, p: &Port, siblings: &[Port]) -> Option<(TrustLevel, u32)> {
        if let Some(ann) = &p.domain {
            return self.annotation_trust(ann.span);
        }
        let def = self.decl_def(p.name.span)?;
        if let Some(t) = self.domain_trust_of_def(def) {
            return Some(t);
        }
        let clocks: Vec<&Port> = siblings
            .iter()
            .filter(|c| {
                self.decl_def(c.name.span)
                    .is_some_and(|d| self.is_clock_def(d))
            })
            .collect();
        match clocks.as_slice() {
            [single] => single
                .domain
                .as_ref()
                .and_then(|ann| self.annotation_trust(ann.span)),
            _ => None,
        }
    }

    /// Bildirilmiş seviyeleri toplar: portlar (modül + extern), register
    /// (`reg(X)` ya da yazıcı alanı) ve wire (modülün alanı).
    fn collect_declared(&mut self) {
        for &item_idx in &self.ast.items {
            let (ports, body): (&[Port], &[Idx<Stmt>]) = match &self.ast.items_arena[item_idx].kind
            {
                ItemKind::Module(m) => (&m.ports, &m.body),
                ItemKind::Extern(x) => (&x.ports, &[]),
                _ => continue,
            };
            for p in ports {
                let Some(def) = self.decl_def(p.name.span) else {
                    continue;
                };
                if self.is_clock_def(def) {
                    continue;
                }
                if let Some(t) = self.port_declared(p, ports) {
                    self.declared.insert(def, t);
                }
            }
            for &stmt in body {
                match &self.ast.stmts[stmt].kind {
                    StmtKind::Reg(r) => {
                        let Some(def) = self.decl_def(r.name.span) else {
                            continue;
                        };
                        let t = match &r.domain {
                            Some(ann) => self.annotation_trust(ann.span),
                            None => self.domain_trust_of_def(def),
                        };
                        if let Some(t) = t {
                            self.declared.insert(def, t);
                        }
                    }
                    StmtKind::Wire(w) => {
                        let Some(def) = self.decl_def(w.name.span) else {
                            continue;
                        };
                        if let Some(t) = self.domain_trust_of_def(def) {
                            self.declared.insert(def, t);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// Bir sinyal okumasının etiketi.
    fn sig_tag(&self, def: DefId, span: Span) -> Option<Tag> {
        if self.is_clock_def(def) {
            return None;
        }
        if let Some(&(level, idx)) = self.declared.get(&def) {
            return Some(Tag {
                level,
                span,
                origin: Some(idx),
            });
        }
        self.inferred.get(&def).map(|t| Tag { span, ..*t })
    }

    /// Sınıflandırılmamış sinyalin etiketini yükselt (yalnız Infer).
    fn raise_inferred(&mut self, def: DefId, tag: Option<Tag>) {
        let Some(tag) = tag else {
            return;
        };
        if self.mode != Mode::Infer {
            return;
        }
        let raise = match self.inferred.get(&def) {
            Some(cur) => tag.level > cur.level,
            None => true,
        };
        if raise {
            self.inferred.insert(def, tag);
            self.changed = true;
        }
    }

    // ═══ İfade etiketi (K5 eşleniği) ══════════════════════════════

    /// `declassify` uygulanmış ifade public'tir; geri kalanı ham etiket.
    fn expr_tag(&mut self, e: Idx<Expr>) -> Option<Tag> {
        if let Some(site) = self.ast.trust.declassify.get(&e) {
            return Some(Tag {
                level: TrustLevel::Public,
                span: site.span,
                origin: None,
            });
        }
        self.expr_tag_raw(e)
    }

    fn expr_tag_raw(&mut self, e: Idx<Expr>) -> Option<Tag> {
        let expr = &self.ast.exprs[e];
        let span = expr.span;
        match &expr.kind {
            ExprKind::IntLit { .. }
            | ExprKind::BoolLit(_)
            | ExprKind::StringLit(_)
            | ExprKind::Todo { .. }
            | ExprKind::Concat(_)
            | ExprKind::Error => None,
            ExprKind::Path(_) => self
                .res
                .resolutions
                .get(&e)
                .and_then(|&def| self.sig_tag(def, span)),
            ExprKind::Binary { lhs, rhs, .. } => {
                let (lhs, rhs) = (*lhs, *rhs);
                let a = self.expr_tag(lhs);
                let b = self.expr_tag(rhs);
                join(a, b)
            }
            ExprKind::Unary { operand, .. } => self.expr_tag(*operand),
            ExprKind::Cast { expr: inner, .. } => self.expr_tag(*inner),
            ExprKind::Index { base, index } => {
                let (base, index) = (*base, *index);
                let a = self.expr_tag(base);
                let b = self.expr_tag(index);
                join(a, b)
            }
            ExprKind::Range { base, .. } => self.expr_tag(*base),
            ExprKind::PartSelect { base, start, .. } => {
                let (base, start) = (*base, *start);
                let a = self.expr_tag(base);
                let b = self.expr_tag(start);
                join(a, b)
            }
            ExprKind::Field { base, field } => {
                let (base, field) = (*base, field.text.clone());
                if let Some(&base_def) = self.res.resolutions.get(&base) {
                    if self.res.def_kind(base_def) == DefKind::Instance {
                        return self.instance_port_tag(base_def, &field, span);
                    }
                }
                self.expr_tag(base)
            }
            ExprKind::Call { callee, args } => {
                let (callee, args) = (*callee, args.clone());
                let builtin =
                    self.res
                        .resolutions
                        .get(&callee)
                        .and_then(|&d| match self.res.def_kind(d) {
                            DefKind::Builtin(k) => Some(k),
                            _ => None,
                        });
                // sync(): saat değişir, güven etiketi korunur (K9).
                // prev(x): x'in etiketi (ADR-0040).
                if matches!(
                    builtin,
                    Some(BuiltinKind::Sync | BuiltinKind::Sync3 | BuiltinKind::Prev)
                ) {
                    return args.first().and_then(|&a| self.expr_tag(a));
                }
                let mut tag = None;
                for a in args {
                    let t = self.expr_tag(a);
                    tag = join(tag, t);
                }
                tag
            }
            ExprKind::If {
                cond,
                then_expr,
                else_expr,
            } => {
                let (cond, then_expr, else_expr) = (*cond, *then_expr, *else_expr);
                let c = self.expr_tag(cond);
                let t = self.expr_tag(then_expr);
                let e2 = self.expr_tag(else_expr);
                join(join(c, t), e2)
            }
            ExprKind::Match { scrutinee, arms } => {
                let scrutinee = *scrutinee;
                let arm_exprs: Vec<Idx<Expr>> = arms
                    .iter()
                    .filter_map(|a| match &a.body {
                        MatchArmBody::Expr(e) => Some(*e),
                        MatchArmBody::Block(_) => None,
                    })
                    .collect();
                let mut tag = self.expr_tag(scrutinee);
                for a in arm_exprs {
                    let t = self.expr_tag(a);
                    tag = join(tag, t);
                }
                tag
            }
            ExprKind::StructLit { fields, .. } => {
                let exprs: Vec<Idx<Expr>> = fields.iter().filter_map(|f| f.value).collect();
                self.join_list(&exprs)
            }
            ExprKind::ArrayLit(volt_ast::ArrayLitKind::List(items)) | ExprKind::TupleLit(items) => {
                let items = items.clone();
                self.join_list(&items)
            }
            ExprKind::ArrayLit(volt_ast::ArrayLitKind::Repeat { value, .. }) => {
                self.expr_tag(*value)
            }
        }
    }

    fn join_list(&mut self, exprs: &[Idx<Expr>]) -> Option<Tag> {
        let mut tag = None;
        for &e in exprs {
            let t = self.expr_tag(e);
            tag = join(tag, t);
        }
        tag
    }

    /// `inst.port` okuması (K8): hedef portun bildirilmiş seviyesi; yoksa
    /// tutucu özet — örneğin sınıflandırılmış girişlerinin en yükseği ile
    /// hedef modülün kendi çıkarımının birleşimi.
    fn instance_port_tag(&mut self, inst_def: DefId, field: &str, span: Span) -> Option<Tag> {
        let summary = self
            .instance_tags
            .get(&inst_def)
            .map(|t| Tag { span, ..*t });
        let Some(&target) = self.res.instance_module.get(&inst_def) else {
            return summary; // yerleşik primitif
        };
        let Some(&item_idx) = self.res.item_of_def.get(&target) else {
            return summary;
        };
        let ports: &[Port] = match &self.ast.items_arena[item_idx].kind {
            ItemKind::Module(m) => &m.ports,
            ItemKind::Extern(x) => &x.ports,
            _ => return summary,
        };
        let Some(port) = ports.iter().find(|p| p.name.text == field) else {
            return summary;
        };
        if let Some((level, idx)) = self.port_declared(port, ports) {
            return Some(Tag {
                level,
                span,
                origin: Some(idx),
            });
        }
        let own = self
            .decl_def(port.name.span)
            .and_then(|d| self.inferred.get(&d).map(|t| Tag { span, ..*t }));
        join(summary, own)
    }

    // ═══ Deyim yürüyüşü (K6/K7 eşleniği) ══════════════════════════

    fn walk_file(&mut self) {
        for &item_idx in &self.ast.items {
            if let ItemKind::Module(m) = &self.ast.items_arena[item_idx].kind {
                self.walk_module(m);
            }
        }
    }

    fn walk_module(&mut self, m: &ModuleDecl) {
        for &stmt in &m.body {
            self.walk_stmt(stmt);
        }
        // Kontratlar gözlemdir, akış değil: denetlenmez (ADR-0052 §5).
    }

    fn walk_stmt(&mut self, stmt_idx: Idx<Stmt>) {
        match &self.ast.stmts[stmt_idx].kind {
            StmtKind::Let(l) => {
                let tag = self.expr_tag(l.value);
                if let Some(def) = self.decl_def(l.name.span) {
                    self.raise_inferred(def, tag);
                }
            }
            StmtKind::Instance(inst) => self.check_instance(inst),
            StmtKind::On(on) => self.walk_block(on.body, None),
            StmtKind::Comb(block) => self.walk_block(*block, None),
            StmtKind::Assign(a) => self.flow(&a.lhs, a.rhs, None),
            StmtKind::For(f) => self.walk_block(f.body, None),
            StmtKind::Reg(_) | StmtKind::Wire(_) | StmtKind::Expr(_) | StmtKind::Error => {}
        }
    }

    /// `pc`: içinde bulunulan dalın koşul etiketi — koşula bağlı her
    /// yazma o koşulun bilgisini taşır (örtük akış).
    fn walk_block(&mut self, block_idx: Idx<Block>, pc: Option<Tag>) {
        let block = &self.ast.blocks[block_idx];
        for stmt in &block.stmts {
            match stmt {
                BlockStmt::NonBlockAssign { lhs, rhs, .. }
                | BlockStmt::BlockAssign { lhs, rhs, .. } => self.flow(lhs, *rhs, pc),
                BlockStmt::If(if_stmt) => self.walk_if(if_stmt, pc),
                BlockStmt::Match(mt) => {
                    let s = self.expr_tag(mt.scrutinee);
                    let pc = join(pc, s);
                    for arm in &mt.arms {
                        match &arm.body {
                            MatchArmBody::Block(b) => self.walk_block(*b, pc),
                            MatchArmBody::Expr(e) => {
                                self.expr_tag(*e);
                            }
                        }
                    }
                }
                BlockStmt::Let(l) => {
                    let tag = self.expr_tag(l.value);
                    if let Some(def) = self.decl_def(l.name.span) {
                        self.raise_inferred(def, join(pc, tag));
                    }
                }
                BlockStmt::For(f) => self.walk_block(f.body, pc),
                BlockStmt::Error => {}
            }
        }
    }

    fn walk_if(&mut self, if_stmt: &IfStmt, pc: Option<Tag>) {
        let c = self.expr_tag(if_stmt.cond);
        let pc = join(pc, c);
        self.walk_block(if_stmt.then_block, pc);
        match &if_stmt.else_branch {
            Some(ElseBranch::Block(b)) => self.walk_block(*b, pc),
            Some(ElseBranch::If(nested)) => self.walk_if(nested, pc),
            None => {}
        }
    }

    /// Atama: kaynak etiketi = sağ taraf ⊔ dal koşulu ⊔ hedef indeksleri
    /// (gizli adrese yazmak da sızdırır).
    fn flow(&mut self, lhs: &LValue, rhs: Idx<Expr>, pc: Option<Tag>) {
        let mut src = join(self.expr_tag(rhs), pc);
        for suffix in &lhs.suffixes {
            match suffix {
                LValueSuffix::Index(e) => {
                    let t = self.expr_tag(*e);
                    src = join(src, t);
                }
                LValueSuffix::PartSelect { start, .. } => {
                    let t = self.expr_tag(*start);
                    src = join(src, t);
                }
                LValueSuffix::Range { .. } | LValueSuffix::Field(_) => {}
            }
        }
        let Some(sink) = self.use_def(lhs.base.span) else {
            return;
        };
        self.flow_into(sink, lhs.span, src);
    }

    /// `src` etiketli bilgi `sink` sinyaline ulaşıyor: sınıflandırılmışsa
    /// seviye karşılaştır (E3009), değilse etiketi yükselt.
    fn flow_into(&mut self, sink: DefId, sink_span: Span, src: Option<Tag>) {
        let Some(src) = src else {
            return;
        };
        match self.declared.get(&sink).copied() {
            Some((level, idx)) => {
                if src.level > level && self.mode == Mode::Check {
                    let kind = self.sink_kind(sink);
                    self.err_leak(sink_span, kind, level, Some(idx), src);
                }
            }
            None => self.raise_inferred(sink, Some(src)),
        }
    }

    fn sink_kind(&self, def: DefId) -> String {
        match self.res.def_kind(def) {
            DefKind::Port { dir: PortDir::Out } => lstr!(en: "output"; tr: "çıkış"),
            DefKind::Port { .. } => lstr!(en: "port"; tr: "port"),
            DefKind::Register => lstr!(en: "register"; tr: "register"),
            DefKind::Wire => lstr!(en: "wire"; tr: "wire"),
            _ => lstr!(en: "signal"; tr: "sinyal"),
        }
    }

    // ═══ K8 — modül örnekleme ═════════════════════════════════════

    fn check_instance(&mut self, inst: &volt_ast::InstanceDecl) {
        let Some(inst_def) = self.decl_def(inst.name.span) else {
            return;
        };
        let target_ports: Option<&[Port]> = self
            .res
            .instance_module
            .get(&inst_def)
            .and_then(|t| self.res.item_of_def.get(t))
            .and_then(|&i| match &self.ast.items_arena[i].kind {
                ItemKind::Module(m) => Some(m.ports.as_slice()),
                ItemKind::Extern(x) => Some(x.ports.as_slice()),
                _ => None,
            });
        let prim = self.res.instance_builtin.get(&inst_def).copied();

        // Her bağlama için (yön, saat mı, hedef portun bildirilmiş seviyesi).
        struct Bound {
            dir: PortDir,
            is_clock: bool,
            declared: Option<(TrustLevel, u32)>,
            own_def: Option<DefId>,
        }
        let bounds: Vec<Bound> = inst
            .bindings
            .iter()
            .map(|b| {
                if let Some(ports) = target_ports {
                    match ports.iter().find(|p| p.name.text == b.port_name.text) {
                        Some(p) => {
                            let def = self.decl_def(p.name.span);
                            Bound {
                                dir: p.direction,
                                is_clock: def.is_some_and(|d| self.is_clock_def(d)),
                                declared: self.port_declared(p, ports),
                                own_def: def,
                            }
                        }
                        None => Bound {
                            dir: PortDir::In,
                            is_clock: false,
                            declared: None,
                            own_def: None,
                        },
                    }
                } else if let Some(prim) = prim {
                    match prim.port(&b.port_name.text) {
                        Some(p) => Bound {
                            dir: p.dir,
                            is_clock: p.kind == PortKind::Clock,
                            declared: None,
                            own_def: None,
                        },
                        None => Bound {
                            dir: PortDir::In,
                            is_clock: false,
                            declared: None,
                            own_def: None,
                        },
                    }
                } else {
                    Bound {
                        dir: PortDir::In,
                        is_clock: false,
                        declared: None,
                        own_def: None,
                    }
                }
            })
            .collect();

        // 1. Özet: sınıflandırılmış giriş bağlamalarının en yükseği.
        let mut summary = None;
        for (b, bound) in inst.bindings.iter().zip(&bounds) {
            if bound.is_clock || bound.dir == PortDir::Out {
                continue;
            }
            let t = self.binding_tag(b);
            summary = join(summary, t);
        }
        if self.mode == Mode::Infer {
            let prev = self.instance_tags.get(&inst_def).copied();
            if prev.map(|t| t.level) != summary.map(|t| t.level) {
                match summary {
                    Some(t) => {
                        self.instance_tags.insert(inst_def, t);
                    }
                    None => {
                        self.instance_tags.remove(&inst_def);
                    }
                }
                self.changed = true;
            }
        }

        // 2. Akışlar: giriş portuna (üst → alt) ve çıkış portundan (alt → üst).
        for (b, bound) in inst.bindings.iter().zip(&bounds) {
            if bound.is_clock {
                continue;
            }
            let into_port = bound.dir != PortDir::Out;
            let from_port = bound.dir != PortDir::In;
            if into_port {
                if let Some((level, idx)) = bound.declared {
                    if let Some(actual) = self.binding_tag(b) {
                        if actual.level > level && self.mode == Mode::Check {
                            let kind = lstr!(en: "port"; tr: "port");
                            self.err_leak(b.span, kind, level, Some(idx), actual);
                        }
                    }
                }
            }
            if from_port {
                let src = match bound.declared {
                    Some((level, idx)) => Some(Tag {
                        level,
                        span: b.port_name.span,
                        origin: Some(idx),
                    }),
                    None => {
                        let own = bound
                            .own_def
                            .and_then(|d| self.inferred.get(&d).copied())
                            .map(|t| Tag {
                                span: b.port_name.span,
                                ..t
                            });
                        let sum = self.instance_tags.get(&inst_def).map(|t| Tag {
                            span: b.port_name.span,
                            ..*t
                        });
                        join(sum, own)
                    }
                };
                if let Some(sink) = self.binding_sink(b) {
                    let sink_span = match b.value {
                        Some(e) => self.ast.exprs[e].span,
                        None => b.port_name.span,
                    };
                    self.flow_into(sink, sink_span, src);
                }
            }
        }
    }

    fn binding_tag(&mut self, b: &volt_ast::PortBinding) -> Option<Tag> {
        match b.value {
            Some(e) => self.expr_tag(e),
            None => self
                .use_def(b.port_name.span)
                .and_then(|def| self.sig_tag(def, b.port_name.span)),
        }
    }

    /// Çıkış bağlamasının sürdüğü yerel sinyal (yalnız çıplak isim).
    fn binding_sink(&self, b: &volt_ast::PortBinding) -> Option<DefId> {
        match b.value {
            Some(e) => match &self.ast.exprs[e].kind {
                ExprKind::Path(_) => self.res.resolutions.get(&e).copied(),
                _ => None,
            },
            None => self.use_def(b.port_name.span),
        }
    }

    // ═══ Tanılar ══════════════════════════════════════════════════

    fn level_label(&self, level: TrustLevel, origin: Option<u32>) -> String {
        match origin {
            Some(idx) => format!(
                "@{} ({})",
                self.domain.domains[idx as usize].name,
                level.as_str()
            ),
            None => format!("({})", level.as_str()),
        }
    }

    fn trust_decl_span(&self, idx: u32) -> Span {
        let d = &self.domain.domains[idx as usize];
        d.trust_span.unwrap_or(d.span)
    }

    /// E3009 (5 parça): kod, hedef konumu, kaynak konumu + iki domain
    /// tanım satırı, neden, çözüm (declassify).
    fn err_leak(
        &mut self,
        sink_span: Span,
        sink_kind: String,
        sink_level: TrustLevel,
        sink_origin: Option<u32>,
        src: Tag,
    ) {
        let (s, d) = (src.level.as_str(), sink_level.as_str());
        let mut diag = Diagnostic::error(
            ErrorCode::E3009,
            lstr!(en: "{s} data flows to a {d} {sink_kind}";
                  tr: "{s} veri {d} {sink_kind} hedefine akıyor"),
            LabeledSpan::primary(sink_span, self.level_label(sink_level, sink_origin)),
            lstr!(en: "if intentional, use declassify(expr, \"reason\")";
                  tr: "bilinçliyse declassify(ifade, \"gerekçe\") kullanın"),
        )
        .with_secondary(src.span, self.level_label(src.level, src.origin));
        if let Some(idx) = src.origin {
            diag = diag.with_secondary(
                self.trust_decl_span(idx),
                lstr!(en: "source trust level here"; tr: "kaynak güven seviyesi burada"),
            );
        }
        if let Some(idx) = sink_origin {
            diag = diag.with_secondary(
                self.trust_decl_span(idx),
                lstr!(en: "destination trust level here"; tr: "hedef güven seviyesi burada"),
            );
        }
        diag = diag.with_note(
            NoteKind::Reason,
            lstr!(en: "information from a higher trust level cannot reach a lower one; \
                       this could leak key material (ADR-0052)";
                  tr: "yüksek güven seviyesindeki bilgi daha düşüğüne ulaşamaz; \
                       bu, anahtar malzemesini sızdırabilir (ADR-0052)"),
        );
        self.diagnostics.push(diag);
    }

    /// W3008 — her `declassify` için iz kaydı: kaynak seviye ve gerekçe.
    fn report_declassify(&mut self) {
        let mut sites: Vec<(Idx<Expr>, volt_ast::DeclassifySite)> = self
            .ast
            .trust
            .declassify
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();
        sites.sort_by_key(|(_, s)| (s.span.file.0, s.span.start));
        let mut seen: HashSet<Span> = HashSet::new();
        for (inner, site) in sites {
            if !seen.insert(site.span) {
                continue;
            }
            let from = self
                .expr_tag_raw(inner)
                .map(|t| self.level_label(t.level, t.origin))
                .unwrap_or_else(|| lstr!(en: "unclassified"; tr: "sınıflandırılmamış"));
            let reason = site.reason.clone();
            self.diagnostics.push(
                Diagnostic::warning(
                    ErrorCode::W3008,
                    lstr!(en: "deliberate trust downgrade: \"{reason}\"";
                          tr: "bilinçli güven düşürme: \"{reason}\""),
                    LabeledSpan::primary(
                        site.span,
                        lstr!(en: "{from} → public here"; tr: "{from} → public burada"),
                    ),
                    lstr!(en: "review this declassification; remove the call to make the flow an E3009 error again";
                          tr: "bu düşürmeyi gözden geçirin; çağrıyı kaldırırsanız akış yeniden E3009 hatası olur"),
                )
                .with_secondary(
                    site.reason_span,
                    lstr!(en: "reason recorded for review"; tr: "gözden geçirme için kaydedilen gerekçe"),
                )
                .with_note(
                    NoteKind::Reason,
                    lstr!(en: "declassify is the only sanctioned path from a higher trust level to a lower one; \
                               the warning is the audit trail, not a defect (ADR-0052)";
                          tr: "declassify yüksek güven seviyesinden düşüğe inen tek meşru yoldur; \
                               uyarı bir kusur değil iz kaydıdır (ADR-0052)"),
                ),
            );
        }
    }
}
