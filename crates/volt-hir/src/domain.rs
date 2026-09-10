//! Saat alanı (domain) çıkarımı ve CDC kontrolü
//! (docs/spec/domain-inference.md K1-K9).
//!
//! Domain her sinyalde VAR ama çoğu zaman YAZILMIYOR (UX Anayasası):
//! tek saatli modülde anotasyonsuz her sinyal o saatin alanına atanır,
//! kullanıcı 'domain' kelimesini hiç görmez (K2). Kullanıcı domain
//! kavramıyla yalnız gerçek bir sorun olduğunda karşılaşır: çoklu saatte
//! belirsizlik (E3010) veya CDC ihlali (E3001).
//!
//! Geçiş tip kontrolünden SONRA koşar; saat portları `Ty::Clock`
//! üzerinden bulunur, sync() genişliği `expr_types`'tan okunur.

use std::collections::HashMap;

use volt_ast::{
    Block, BlockStmt, ClockEdge, DomainKey, DomainValue, ElseBranch, Expr, ExprKind, Idx, IfStmt,
    ItemKind, LValue, LValueSuffix, MatchArmBody, ModuleDecl, Name, OnTrigger, Port, ResetPolarity,
    ResetSpec, ResetSync, SourceFile, Stmt, StmtKind,
};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use crate::resolve::{BuiltinKind, DefId, DefKind, ResolveResult};
use crate::ty::Ty;
use crate::typeck::TypeckResult;

/// Çözülmemiş domain değişkeni — çıkarım sırasında kısıt biriktirir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InferVar(pub u32);

/// domain-inference.md §1 — sinyalin ait olduğu saat alanı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DomainId {
    /// Belirli bir saat alanı (`DomainResult::domains` indeksi).
    Explicit(u32),
    /// Saatten bağımsız — sabitler, saf kombinasyonel.
    Timeless,
    /// Henüz çözülmemiş (çıkarım sırasında).
    Unresolved(InferVar),
    /// Hata kurtarma — her domainle uyumlu.
    Error,
}

/// Saat kenarı bilgisi (domain-inference.md §1 ClockSpec).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockSpec {
    pub edge: ClockEdge,
}

/// Bir saat alanının tanım bilgisi.
#[derive(Debug, Clone)]
pub struct DomainInfo {
    pub name: String,
    pub clock: ClockSpec,
    pub reset: ResetSpec,
    /// Tanım satırı — E3001/E3010 ikincil etiketleri buraya bağlanır.
    pub span: Span,
    pub source: DomainSource,
}

/// Domain nereden geliyor: açık `domain` bildirimi veya anotasyonsuz
/// clock portunun ürettiği örtük alan (K2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainSource {
    Decl(DefId),
    ClockPort(DefId),
}

/// Domain çıkarımı çıktısı.
#[derive(Debug, Default)]
pub struct DomainResult {
    pub domains: Vec<DomainInfo>,
    /// Sinyal tanımı → çıkarılan saat alanı.
    pub signal_domains: HashMap<DefId, DomainId>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Dosyadaki tüm modüllerin saat alanlarını çıkarır ve CDC denetler.
pub fn infer_domains(ast: &SourceFile, res: &ResolveResult, tyck: &TypeckResult) -> DomainResult {
    let mut inf = Inferencer {
        ast,
        res,
        tyck,
        domains: Vec::new(),
        by_decl: HashMap::new(),
        by_clock_port: HashMap::new(),
        signal_domains: HashMap::new(),
        expr_domains: HashMap::new(),
        vars: Vec::new(),
        instance_ports: HashMap::new(),
        diagnostics: Vec::new(),
        default_domain: DomainId::Timeless,
        multi_clock: false,
        clock_candidates: Vec::new(),
    };
    inf.collect_domain_decls();
    for &item_idx in &ast.items {
        if let ItemKind::Module(m) = &ast.items_arena[item_idx].kind {
            inf.infer_module(m);
        }
    }
    DomainResult {
        domains: inf.domains,
        signal_domains: inf.signal_domains,
        diagnostics: inf.diagnostics,
    }
}

struct Inferencer<'a> {
    ast: &'a SourceFile,
    res: &'a ResolveResult,
    tyck: &'a TypeckResult,
    domains: Vec<DomainInfo>,
    /// `domain Ad { ... }` bildirimi → domain indeksi.
    by_decl: HashMap<DefId, u32>,
    /// Anotasyonsuz clock portu → örtük domain indeksi.
    by_clock_port: HashMap<DefId, u32>,
    signal_domains: HashMap<DefId, DomainId>,
    expr_domains: HashMap<Idx<Expr>, DomainId>,
    /// Unresolved değişken bağlamaları (kısıt çözümü).
    vars: Vec<Option<DomainId>>,
    /// Instance tanımı → hedef port adı → beklenen domain (K8).
    instance_ports: HashMap<DefId, HashMap<String, DomainId>>,
    diagnostics: Vec<Diagnostic>,

    // ── Modül bağlamı ──
    default_domain: DomainId,
    multi_clock: bool,
    /// E3010 aday listesi: (port span, domain görünen adı).
    clock_candidates: Vec<(Span, String)>,
}

impl<'a> Inferencer<'a> {
    // ═══ Domain tabloları ═════════════════════════════════════════

    fn collect_domain_decls(&mut self) {
        for &item_idx in &self.ast.items {
            let ItemKind::Domain(d) = &self.ast.items_arena[item_idx].kind else {
                continue;
            };
            let Some(&def) = self.res.decl_spans.get(&d.name.span) else {
                continue;
            };
            let mut clock = ClockSpec {
                edge: ClockEdge::Posedge,
            };
            let mut reset = ResetSpec {
                sync: ResetSync::None,
                polarity: ResetPolarity::ActiveHigh,
            };
            for field in &d.fields {
                match (&field.key, &field.value) {
                    (DomainKey::Clock, DomainValue::ClockEdge(edge)) => clock.edge = *edge,
                    (DomainKey::Reset, DomainValue::Reset(spec)) => reset = *spec,
                    _ => {}
                }
            }
            let id = self.domains.len() as u32;
            self.domains.push(DomainInfo {
                name: d.name.text.clone(),
                clock,
                reset,
                span: d.name.span,
                source: DomainSource::Decl(def),
            });
            self.by_decl.insert(def, id);
        }
    }

    /// Anotasyonsuz clock portuna örtük domain açar (K2).
    fn implicit_clock_domain(&mut self, port_def: DefId, name: &str, span: Span) -> u32 {
        if let Some(&id) = self.by_clock_port.get(&port_def) {
            return id;
        }
        let id = self.domains.len() as u32;
        self.domains.push(DomainInfo {
            name: name.to_string(),
            clock: ClockSpec {
                edge: ClockEdge::Posedge,
            },
            reset: ResetSpec {
                sync: ResetSync::None,
                polarity: ResetPolarity::ActiveHigh,
            },
            span,
            source: DomainSource::ClockPort(port_def),
        });
        self.by_clock_port.insert(port_def, id);
        id
    }

    fn domain_name(&self, id: u32) -> &str {
        &self.domains[id as usize].name
    }

    fn domain_span(&self, id: u32) -> Span {
        self.domains[id as usize].span
    }

    fn display(&self, d: DomainId) -> String {
        match self.resolve_dom(d) {
            DomainId::Explicit(id) => format!("@{}", self.domain_name(id)),
            DomainId::Timeless => lstr!(en: "clockless (constant)"; tr: "saatsiz (sabit)"),
            DomainId::Unresolved(_) => lstr!(en: "<unresolved>"; tr: "<belirsiz>"),
            DomainId::Error => lstr!(en: "<error>"; tr: "<hata>"),
        }
    }

    // ═══ Yardımcılar ══════════════════════════════════════════════

    fn is_clock_def(&self, def: DefId) -> bool {
        self.tyck
            .def_types
            .get(&def)
            .is_some_and(|&t| matches!(self.tyck.types.ty(t), Ty::Clock))
    }

    fn decl_def(&self, name: &Name) -> Option<DefId> {
        self.res.decl_spans.get(&name.span).copied()
    }

    fn use_def(&self, span: Span) -> Option<DefId> {
        self.res.use_spans.get(&span).copied()
    }

    /// Unresolved değişkenleri bağlarına kadar izler.
    fn resolve_dom(&self, d: DomainId) -> DomainId {
        let mut cur = d;
        // Bağlama zinciri kısadır; döngü koruması için sınırlı adım.
        for _ in 0..64 {
            match cur {
                DomainId::Unresolved(v) => match self.vars[v.0 as usize] {
                    Some(next) => cur = next,
                    None => return cur,
                },
                other => return other,
            }
        }
        cur
    }

    fn fresh_var(&mut self) -> DomainId {
        let v = InferVar(self.vars.len() as u32);
        self.vars.push(None);
        DomainId::Unresolved(v)
    }

    /// `@Ad` veya `reg(clk)` anotasyonunu domain'e çevirir (K1).
    /// Çözülemeyen isim E3002'yi isim çözümlemede almıştır → Error.
    fn annotation_domain(&mut self, name: &Name) -> DomainId {
        let Some(def) = self.use_def(name.span) else {
            return DomainId::Error;
        };
        match self.res.def_kind(def) {
            DefKind::Domain => match self.by_decl.get(&def) {
                Some(&id) => DomainId::Explicit(id),
                None => DomainId::Error,
            },
            DefKind::Port { .. } if self.is_clock_def(def) => self
                .signal_domains
                .get(&def)
                .copied()
                .unwrap_or(DomainId::Error),
            // İsim çözümleme portları `reg(clk)` için kabul eder; clock
            // tipinde olmayan port burada yakalanır.
            DefKind::Port { .. } => {
                self.diagnostics.push(Diagnostic::error(
                    ErrorCode::E3002,
                    lstr!(
                        en: "'{}' is not a clock domain", name.text;
                        tr: "'{}' bir saat alanı değil", name.text
                    ),
                    LabeledSpan::primary(
                        name.span,
                        lstr!(en: "not of clock type"; tr: "clock tipinde değil"),
                    ),
                    lstr!(
                        en: "use a port of clock type or a domain definition";
                        tr: "clock tipinde bir port ya da domain tanımı kullanın"
                    ),
                ));
                DomainId::Error
            }
            // Diğer türler için E3002 isim çözümlemede üretildi.
            _ => DomainId::Error,
        }
    }

    // ═══ Modül çıkarımı (§3 akışı) ════════════════════════════════

    fn infer_module(&mut self, m: &ModuleDecl) {
        // 1. MODÜL TARAMASI — clock portlarını topla (K2).
        let mut clocks: Vec<(DefId, &Port)> = Vec::new();
        for p in &m.ports {
            if let Some(def) = self.decl_def(&p.name) {
                if self.is_clock_def(def) {
                    clocks.push((def, p));
                }
            }
        }

        self.clock_candidates.clear();
        for &(def, p) in &clocks {
            let dom = match &p.domain {
                Some(ann) => self.annotation_domain(&ann.clone()),
                None => {
                    let id = self.implicit_clock_domain(def, &p.name.text, p.name.span);
                    DomainId::Explicit(id)
                }
            };
            self.signal_domains.insert(def, dom);
            self.clock_candidates.push((p.name.span, self.display(dom)));
        }

        self.multi_clock = clocks.len() > 1;
        self.default_domain = match clocks.as_slice() {
            [] => DomainId::Timeless,
            [(def, _)] => self.signal_domains[def],
            _ => DomainId::Error, // çoklu saat: varsayılan yok (K3)
        };

        // 2. PORT ATAMASI (K1, K2, K3).
        for p in &m.ports {
            let Some(def) = self.decl_def(&p.name) else {
                continue;
            };
            if self.is_clock_def(def) {
                continue;
            }
            let dom = match &p.domain {
                Some(ann) => self.annotation_domain(&ann.clone()),
                None if self.multi_clock => {
                    self.err_ambiguous(&p.name.clone());
                    DomainId::Error
                }
                None => self.default_domain,
            };
            self.signal_domains.insert(def, dom);
        }

        // 3. REGISTER ATAMASI (K4) — önce açık `reg(clk)`, sonra
        //    'on' bloğu yazıcılarından çıkarım.
        self.assign_reg_domains(m);

        // 4-6. YAYILIM + ATAMA + ÖRNEK KONTROLÜ (K5-K9).
        for &stmt in &m.body {
            self.walk_stmt(stmt);
        }

        // 7. KONTRATLAR (F4a) — kontrat ifadesindeki sinyaller aynı
        //    alanda olmalı; karışım join üzerinden E3001 üretir.
        for c in &m.contracts {
            self.expr_domain(c.expr);
        }
    }

    /// K3 — çoklu saatte anotasyonsuz sinyal (E3010, 5 parça).
    fn err_ambiguous(&mut self, name: &Name) {
        let candidate = self.clock_candidates.first().map(|(_, d)| d.clone());
        let mut diag = Diagnostic::error(
            ErrorCode::E3010,
            lstr!(
                en: "cannot determine the signal's clock domain";
                tr: "sinyalin saat alanı belirlenemiyor"
            ),
            LabeledSpan::primary(
                name.span,
                lstr!(
                    en: "ambiguous which domain this belongs to";
                    tr: "hangi alana ait olduğu belirsiz"
                ),
            ),
            lstr!(
                en: "add an explicit annotation: {} : <type> {}",
                    name.text,
                    candidate.clone().unwrap_or_else(|| "@Domain".to_string());
                tr: "açık anotasyon ekleyin: {} : <tip> {}",
                    name.text,
                    candidate.clone().unwrap_or_else(|| "@Alan".to_string())
            ),
        )
        .with_note(
            NoteKind::Reason,
            lstr!(
                en: "the module has more than one clock, so it cannot be inferred \
                     which one the signal belongs to";
                tr: "modülde birden fazla saat var, sinyalin hangisine \
                     ait olduğu çıkarılamıyor"
            ),
        );
        for (span, dom) in self.clock_candidates.clone() {
            diag = diag.with_secondary(span, lstr!(en: "candidate: {dom}"; tr: "aday: {dom}"));
        }
        self.diagnostics.push(diag);
    }

    // ═══ K4 — register domain'i ═══════════════════════════════════

    fn assign_reg_domains(&mut self, m: &ModuleDecl) {
        // Açık `reg(clk)` anotasyonları önce (K1).
        let mut inferred: Vec<(DefId, &volt_ast::RegDecl)> = Vec::new();
        for &stmt in &m.body {
            let StmtKind::Reg(r) = &self.ast.stmts[stmt].kind else {
                continue;
            };
            let Some(def) = self.decl_def(&r.name) else {
                continue;
            };
            match &r.domain {
                Some(ann) => {
                    let dom = self.annotation_domain(&ann.clone());
                    self.signal_domains.insert(def, dom);
                }
                None => inferred.push((def, r)),
            }
        }
        if inferred.is_empty() {
            return;
        }

        // Yazıcı taraması: her 'on' bloğunun domain'i + yazdığı tanımlar.
        let mut writers: HashMap<DefId, Vec<(DomainId, Span)>> = HashMap::new();
        for &stmt in &m.body {
            let StmtKind::On(on) = &self.ast.stmts[stmt].kind else {
                continue;
            };
            let (dom, trigger_span) = self.on_block_domain(&on.trigger);
            let mut written = Vec::new();
            self.collect_writes(on.body, &mut written);
            for def in written {
                writers.entry(def).or_default().push((dom, trigger_span));
            }
        }

        for (def, r) in inferred {
            let blocks = writers.get(&def).cloned().unwrap_or_default();
            let dom = if blocks.is_empty() {
                // Hiç yazılmıyor → sabit gibi (W3001).
                self.diagnostics.push(
                    Diagnostic::warning(
                        ErrorCode::W3001,
                        lstr!(
                            en: "register is never written in any 'on' block: '{}'",
                                r.name.text;
                            tr: "register hiçbir 'on' bloğunda yazılmıyor: '{}'",
                                r.name.text
                        ),
                        LabeledSpan::primary(
                            r.name.span,
                            lstr!(
                                en: "no sequential assignment to this register";
                                tr: "bu register'a sıralı atama yok"
                            ),
                        ),
                        lstr!(
                            en: "write it with '<=' in an 'on <clock>' block \
                                 or use 'let' if it is a constant";
                            tr: "bir 'on <saat>' bloğunda '<=' ile yazın \
                                 ya da sabitse 'let' kullanın"
                        ),
                    )
                    .with_note(
                        NoteKind::Reason,
                        lstr!(
                            en: "a register that is never written produces a constant value";
                            tr: "yazılmayan register sabit bir değer üretir"
                        ),
                    ),
                );
                DomainId::Timeless
            } else {
                let mut distinct: Vec<(u32, Span)> = Vec::new();
                for (dom, span) in &blocks {
                    if let DomainId::Explicit(id) = self.resolve_dom(*dom) {
                        if !distinct.iter().any(|&(d, _)| d == id) {
                            distinct.push((id, *span));
                        }
                    }
                }
                match distinct.len() {
                    0 => DomainId::Error, // yazıcılar hatalı — kaskad bastır
                    1 => DomainId::Explicit(distinct[0].0),
                    // ÇOK CİDDİ HATA: aynı register iki saatten yazılıyor.
                    _ => {
                        self.err_two_domains(r, &distinct);
                        DomainId::Error
                    }
                }
            };
            self.signal_domains.insert(def, dom);
        }
    }

    /// K4 — E3011, 5 parça: kod, konum, neden, çözüm, spec referansı.
    fn err_two_domains(&mut self, r: &volt_ast::RegDecl, distinct: &[(u32, Span)]) {
        let names: Vec<String> = distinct
            .iter()
            .map(|(id, _)| format!("@{}", self.domain_name(*id)))
            .collect();
        let mut diag = Diagnostic::error(
            ErrorCode::E3011,
            lstr!(
                en: "register is written from more than one clock domain";
                tr: "register birden fazla saat alanından yazılıyor"
            ),
            LabeledSpan::primary(
                r.name.span,
                lstr!(
                    en: "'{}' is written from these domains: {}",
                        r.name.text, names.join(", ");
                    tr: "'{}' şu alanlardan yazılıyor: {}",
                        r.name.text, names.join(", ")
                ),
            ),
            lstr!(
                en: "each register must belong to a single clock domain — \
                     split the register or feed it from one domain with sync()";
                tr: "her register tek bir saat alanına ait olmalı — \
                     register'ı bölün ya da sync() ile tek alandan besleyin"
            ),
        )
        .with_note(
            NoteKind::Reason,
            lstr!(
                en: "a register written from two clocks cannot be synthesized \
                     in hardware; it is ambiguous which edge wins";
                tr: "iki saatten yazılan register donanımda sentezlenemez; \
                     hangi kenarın kazanacağı belirsizdir"
            ),
        );
        for (id, span) in distinct {
            diag = diag.with_secondary(
                *span,
                lstr!(
                    en: "@{} writes from here", self.domain_name(*id);
                    tr: "@{} buradan yazıyor", self.domain_name(*id)
                ),
            );
        }
        for (id, _) in distinct {
            diag = diag.with_secondary(
                self.domain_span(*id),
                lstr!(
                    en: "@{} defined here", self.domain_name(*id);
                    tr: "@{} burada tanımlı", self.domain_name(*id)
                ),
            );
        }
        self.diagnostics.push(diag);
    }

    /// 'on' bloğunun domain'i tetikleyici saatten gelir.
    fn on_block_domain(&mut self, trigger: &OnTrigger) -> (DomainId, Span) {
        match trigger {
            OnTrigger::Clock(name) | OnTrigger::Reset(name) => {
                let dom = self
                    .use_def(name.span)
                    .and_then(|def| self.signal_domains.get(&def).copied())
                    .unwrap_or(DomainId::Error);
                (dom, name.span)
            }
            OnTrigger::Error => (
                DomainId::Error,
                Span::new(volt_span::FileId(u32::MAX), 0, 0),
            ),
        }
    }

    /// Blok içinde sıralı yazılan tanımları toplar (K4 yazıcı taraması).
    fn collect_writes(&self, block_idx: Idx<Block>, out: &mut Vec<DefId>) {
        let block = &self.ast.blocks[block_idx];
        for stmt in &block.stmts {
            match stmt {
                BlockStmt::NonBlockAssign { lhs, .. } | BlockStmt::BlockAssign { lhs, .. } => {
                    if let Some(def) = self.use_def(lhs.base.span) {
                        if self.res.def_kind(def) == DefKind::Register {
                            out.push(def);
                        }
                    }
                }
                BlockStmt::If(if_stmt) => self.collect_writes_if(if_stmt, out),
                BlockStmt::Match(mt) => {
                    for arm in &mt.arms {
                        if let MatchArmBody::Block(b) = &arm.body {
                            self.collect_writes(*b, out);
                        }
                    }
                }
                BlockStmt::For(f) => self.collect_writes(f.body, out),
                BlockStmt::Let(_) | BlockStmt::Error => {}
            }
        }
    }

    fn collect_writes_if(&self, if_stmt: &IfStmt, out: &mut Vec<DefId>) {
        self.collect_writes(if_stmt.then_block, out);
        match &if_stmt.else_branch {
            Some(ElseBranch::Block(b)) => self.collect_writes(*b, out),
            Some(ElseBranch::If(nested)) => self.collect_writes_if(nested, out),
            None => {}
        }
    }

    // ═══ Deyim yürüyüşü (K5-K9) ═══════════════════════════════════

    fn walk_stmt(&mut self, stmt_idx: Idx<Stmt>) {
        match &self.ast.stmts[stmt_idx].kind {
            StmtKind::Reg(r) => {
                // init sabittir; yine de yayılım için hesaplanır.
                self.expr_domain(r.init);
            }
            StmtKind::Let(l) => {
                let dom = self.expr_domain(l.value);
                if let Some(def) = self.decl_def(&l.name) {
                    self.signal_domains.insert(def, dom);
                }
            }
            StmtKind::Wire(w) => {
                // Anotasyon sözdizimi yok: çoklu saatte kısıt değişkeni,
                // tek saatte varsayılan alan.
                if let Some(def) = self.decl_def(&w.name) {
                    let dom = if self.multi_clock {
                        self.fresh_var()
                    } else {
                        self.default_domain
                    };
                    self.signal_domains.insert(def, dom);
                }
            }
            StmtKind::Instance(inst) => self.check_instance(inst),
            StmtKind::On(on) => {
                let (dom, span) = self.on_block_domain(&on.trigger);
                self.walk_block(on.body, Some((dom, span)));
            }
            StmtKind::Comb(block) => self.walk_block(*block, None),
            StmtKind::Assign(a) => self.check_assign(&a.lhs, a.rhs, None),
            StmtKind::For(f) => {
                self.expr_domain(f.start);
                self.expr_domain(f.end);
                self.walk_block(f.body, None);
            }
            StmtKind::Expr(e) => {
                self.expr_domain(*e);
            }
            StmtKind::Error => {}
        }
    }

    fn walk_block(&mut self, block_idx: Idx<Block>, ctx: Option<(DomainId, Span)>) {
        let block = &self.ast.blocks[block_idx];
        for stmt in &block.stmts {
            match stmt {
                BlockStmt::NonBlockAssign { lhs, rhs, .. }
                | BlockStmt::BlockAssign { lhs, rhs, .. } => self.check_assign(lhs, *rhs, ctx),
                BlockStmt::If(if_stmt) => self.walk_if(if_stmt, ctx),
                BlockStmt::Match(mt) => {
                    let dom = self.expr_domain(mt.scrutinee);
                    if let Some(ctx) = ctx {
                        self.check_foreign_read(dom, self.ast.exprs[mt.scrutinee].span, ctx);
                    }
                    for arm in &mt.arms {
                        match &arm.body {
                            MatchArmBody::Block(b) => self.walk_block(*b, ctx),
                            MatchArmBody::Expr(e) => {
                                self.expr_domain(*e);
                            }
                        }
                    }
                }
                BlockStmt::Let(l) => {
                    let dom = self.expr_domain(l.value);
                    if let Some(def) = self.decl_def(&l.name) {
                        self.signal_domains.insert(def, dom);
                    }
                }
                BlockStmt::For(f) => {
                    self.expr_domain(f.start);
                    self.expr_domain(f.end);
                    self.walk_block(f.body, ctx);
                }
                BlockStmt::Error => {}
            }
        }
        if let Some(tail) = block.tail {
            self.expr_domain(tail);
        }
    }

    fn walk_if(&mut self, if_stmt: &IfStmt, ctx: Option<(DomainId, Span)>) {
        let dom = self.expr_domain(if_stmt.cond);
        if let Some(ctx) = ctx {
            self.check_foreign_read(dom, self.ast.exprs[if_stmt.cond].span, ctx);
        }
        self.walk_block(if_stmt.then_block, ctx);
        match &if_stmt.else_branch {
            Some(ElseBranch::Block(b)) => self.walk_block(*b, ctx),
            Some(ElseBranch::If(nested)) => self.walk_if(nested, ctx),
            None => {}
        }
    }

    /// K7 — 'on' bloğu koşulunda/seçicisinde yabancı domain (E3012).
    fn check_foreign_read(&mut self, dom: DomainId, span: Span, ctx: (DomainId, Span)) {
        let (block_dom, trigger_span) = ctx;
        let (d, b) = (self.resolve_dom(dom), self.resolve_dom(block_dom));
        let (DomainId::Explicit(x), DomainId::Explicit(y)) = (d, b) else {
            self.bind_if_var(dom, block_dom);
            return;
        };
        if x == y {
            return;
        }
        self.diagnostics.push(
            Diagnostic::error(
                ErrorCode::E3012,
                lstr!(
                    en: "a signal from a foreign clock domain is read in an 'on' block";
                    tr: "'on' bloğunda yabancı saat alanından sinyal okunuyor"
                ),
                LabeledSpan::primary(
                    span,
                    lstr!(
                        en: "comes from the {} domain", self.display(d);
                        tr: "{} alanından geliyor", self.display(d)
                    ),
                ),
                lstr!(
                    en: "synchronize it first: sync(<signal>, <clock of {}>)",
                        self.domain_name(y);
                    tr: "önce senkronize edin: sync(<sinyal>, <{} saati>)",
                        self.domain_name(y)
                ),
            )
            .with_secondary(
                trigger_span,
                lstr!(
                    en: "the block is in the @{} domain", self.domain_name(y);
                    tr: "blok @{} alanında", self.domain_name(y)
                ),
            )
            .with_secondary(
                self.domain_span(x),
                lstr!(
                    en: "@{} defined here", self.domain_name(x);
                    tr: "@{} burada tanımlı", self.domain_name(x)
                ),
            )
            .with_note(
                NoteKind::Reason,
                lstr!(
                    en: "a signal arriving from a foreign clock may be sampled during \
                         an unstable window by this block's registers (metastability)";
                    tr: "yabancı saatten gelen sinyal bu bloğun register'larında \
                         kararsız anda yakalanabilir (metastabilite)"
                ),
            ),
        );
    }

    /// Unresolved tarafı varsa kısıt olarak bağla.
    fn bind_if_var(&mut self, a: DomainId, b: DomainId) {
        match (self.resolve_dom(a), self.resolve_dom(b)) {
            (DomainId::Unresolved(v), x) | (x, DomainId::Unresolved(v)) if !matches!(x, DomainId::Unresolved(w) if w == v) =>
            {
                self.vars[v.0 as usize] = Some(x);
            }
            _ => {}
        }
    }

    // ═══ K6/K7 — atama kontrolü ═══════════════════════════════════

    fn check_assign(&mut self, lhs: &LValue, rhs: Idx<Expr>, ctx: Option<(DomainId, Span)>) {
        let rhs_dom = self.expr_domain(rhs);
        let lhs_dom = self
            .use_def(lhs.base.span)
            .and_then(|def| self.signal_domains.get(&def).copied())
            .unwrap_or(DomainId::Timeless);

        // İndeks/aralık ifadeleri de okumadır — domain'leri hesaplanır.
        for suffix in &lhs.suffixes {
            match suffix {
                LValueSuffix::Index(e) => {
                    let d = self.expr_domain(*e);
                    self.check_compat(lhs_dom, d, lhs.span, self.ast.exprs[*e].span);
                }
                LValueSuffix::Range { hi, lo } => {
                    self.expr_domain(*hi);
                    self.expr_domain(*lo);
                }
                LValueSuffix::PartSelect { start, width, .. } => {
                    let d = self.expr_domain(*start);
                    self.check_compat(lhs_dom, d, lhs.span, self.ast.exprs[*start].span);
                    self.expr_domain(*width);
                }
                LValueSuffix::Field(_) => {}
            }
        }

        // 'on' bloğunda yazılan sinyal bloğun domain'inde olmalı (K7).
        if let Some((block_dom, trigger_span)) = ctx {
            let (l, b) = (self.resolve_dom(lhs_dom), self.resolve_dom(block_dom));
            if let (DomainId::Explicit(x), DomainId::Explicit(y)) = (l, b) {
                if x != y {
                    self.err_cdc_assign(lhs_dom, block_dom, lhs.span, trigger_span);
                    return;
                }
            }
        }

        self.check_compat(lhs_dom, rhs_dom, lhs.span, self.ast.exprs[rhs].span);
    }

    /// K6 — hedef ile kaynak aynı alanda mı? Timeless muaf.
    fn check_compat(&mut self, dst: DomainId, src: DomainId, dst_span: Span, src_span: Span) {
        let (d, s) = (self.resolve_dom(dst), self.resolve_dom(src));
        match (d, s) {
            // Sabit her yere atanabilir; hata kaskadı bastırılır.
            (_, DomainId::Timeless) | (DomainId::Timeless, _) => {}
            (DomainId::Error, _) | (_, DomainId::Error) => {}
            (DomainId::Explicit(x), DomainId::Explicit(y)) if x == y => {}
            (DomainId::Explicit(_), DomainId::Explicit(_)) => {
                self.err_cdc_assign(d, s, dst_span, src_span);
            }
            _ => self.bind_if_var(d, s),
        }
    }

    /// E3001 (atama biçimi) — iki span: hedef ve kaynak alanları,
    /// domain tanım satırlarına ikincil etiketler (§4).
    fn err_cdc_assign(&mut self, dst: DomainId, src: DomainId, dst_span: Span, src_span: Span) {
        let (d, s) = (self.resolve_dom(dst), self.resolve_dom(src));
        let dst_name = match d {
            DomainId::Explicit(id) => Some(self.domain_name(id).to_string()),
            _ => None,
        };
        let mut diag = Diagnostic::error(
            ErrorCode::E3001,
            lstr!(
                en: "direct assignment between clock domains";
                tr: "saat alanları arasında doğrudan atama"
            ),
            LabeledSpan::primary(dst_span, self.display(d)),
            lstr!(
                en: "synchronize into the target domain with sync(): \
                     dest = sync(src, <clock of {}>)",
                    dst_name.clone().unwrap_or_else(|| "dest".to_string());
                tr: "sync() ile hedef alana senkronize edin: \
                     hedef = sync(kaynak, <{} saati>)",
                    dst_name.clone().unwrap_or_else(|| "hedef".to_string())
            ),
        )
        .with_secondary(src_span, self.display(s))
        .with_note(
            NoteKind::Reason,
            lstr!(
                en: "the destination register may sample the source signal \
                     during an unstable window (metastability)";
                tr: "hedef register kaynak sinyali kararsız anda \
                     yakalayabilir (metastabilite)"
            ),
        )
        .with_note(
            NoteKind::Note,
            lstr!(
                en: "for multi-bit data, AsyncFifo may be safer";
                tr: "çok bitli veri için AsyncFifo daha güvenli olabilir"
            ),
        );
        if let DomainId::Explicit(id) = d {
            diag = diag.with_secondary(
                self.domain_span(id),
                lstr!(
                    en: "destination @{} defined here", self.domain_name(id);
                    tr: "hedef @{} burada tanımlı", self.domain_name(id)
                ),
            );
        }
        if let DomainId::Explicit(id) = s {
            diag = diag.with_secondary(
                self.domain_span(id),
                lstr!(
                    en: "source @{} defined here", self.domain_name(id);
                    tr: "kaynak @{} burada tanımlı", self.domain_name(id)
                ),
            );
        }
        self.diagnostics.push(diag);
    }

    // ═══ K5 — kombinasyonel yayılım ═══════════════════════════════

    fn expr_domain(&mut self, e: Idx<Expr>) -> DomainId {
        if let Some(&d) = self.expr_domains.get(&e) {
            return d;
        }
        let d = self.compute_expr_domain(e);
        self.expr_domains.insert(e, d);
        d
    }

    fn compute_expr_domain(&mut self, e: Idx<Expr>) -> DomainId {
        let expr = &self.ast.exprs[e];
        match &expr.kind {
            ExprKind::IntLit { .. }
            | ExprKind::BoolLit(_)
            | ExprKind::StringLit(_)
            | ExprKind::Todo { .. }
            | ExprKind::Error => DomainId::Timeless,
            ExprKind::Path(_) => match self.res.resolutions.get(&e) {
                Some(&def) => self.def_domain(def),
                None => DomainId::Timeless,
            },
            ExprKind::Binary { lhs, rhs, .. } => {
                let (lhs, rhs) = (*lhs, *rhs);
                let a = self.expr_domain(lhs);
                let b = self.expr_domain(rhs);
                self.join(a, b, self.ast.exprs[lhs].span, self.ast.exprs[rhs].span)
            }
            ExprKind::Unary { operand, .. } => self.expr_domain(*operand),
            ExprKind::Cast { expr: inner, .. } => self.expr_domain(*inner),
            ExprKind::Index { base, index } => {
                let (base, index) = (*base, *index);
                let a = self.expr_domain(base);
                let b = self.expr_domain(index);
                self.join(a, b, self.ast.exprs[base].span, self.ast.exprs[index].span)
            }
            ExprKind::Range { base, hi, lo } => {
                let (base, hi, lo) = (*base, *hi, *lo);
                self.expr_domain(hi);
                self.expr_domain(lo);
                self.expr_domain(base)
            }
            // Başlangıç indeksi çalışma zamanı okumasıdır; genişlik sabittir.
            ExprKind::PartSelect {
                base, start, width, ..
            } => {
                let (base, start, width) = (*base, *start, *width);
                self.expr_domain(width);
                let a = self.expr_domain(base);
                let b = self.expr_domain(start);
                self.join(a, b, self.ast.exprs[base].span, self.ast.exprs[start].span)
            }
            ExprKind::Field { base, field } => {
                let (base, field) = (*base, field.clone());
                // Instance port okuması: K8 haritasından (uart.busy).
                if let Some(&base_def) = self.res.resolutions.get(&base) {
                    if let Some(ports) = self.instance_ports.get(&base_def) {
                        return ports
                            .get(&field.text)
                            .copied()
                            .unwrap_or(DomainId::Timeless);
                    }
                }
                self.expr_domain(base)
            }
            ExprKind::Call { callee, args } => {
                let (callee, args) = (*callee, args.clone());
                if let Some(kind) = self.builtin_of(callee) {
                    if matches!(kind, BuiltinKind::Sync | BuiltinKind::Sync3) {
                        return self.sync_domain(&args, expr.span);
                    }
                }
                let mut dom = DomainId::Timeless;
                let mut dom_span = expr.span;
                for &a in &args {
                    let d = self.expr_domain(a);
                    let a_span = self.ast.exprs[a].span;
                    dom = self.join(dom, d, dom_span, a_span);
                    if matches!(self.resolve_dom(d), DomainId::Explicit(_)) {
                        dom_span = a_span;
                    }
                }
                dom
            }
            ExprKind::If {
                cond,
                then_expr,
                else_expr,
            } => {
                let (cond, then_expr, else_expr) = (*cond, *then_expr, *else_expr);
                let c = self.expr_domain(cond);
                let t = self.expr_domain(then_expr);
                let e2 = self.expr_domain(else_expr);
                let ct = self.join(
                    c,
                    t,
                    self.ast.exprs[cond].span,
                    self.ast.exprs[then_expr].span,
                );
                self.join(
                    ct,
                    e2,
                    self.ast.exprs[cond].span,
                    self.ast.exprs[else_expr].span,
                )
            }
            ExprKind::Match { scrutinee, arms } => {
                let scrutinee = *scrutinee;
                let mut dom = self.expr_domain(scrutinee);
                let s_span = self.ast.exprs[scrutinee].span;
                let arm_exprs: Vec<Idx<Expr>> = arms
                    .iter()
                    .filter_map(|a| match &a.body {
                        MatchArmBody::Expr(e) => Some(*e),
                        MatchArmBody::Block(_) => None,
                    })
                    .collect();
                for a in arm_exprs {
                    let d = self.expr_domain(a);
                    dom = self.join(dom, d, s_span, self.ast.exprs[a].span);
                }
                dom
            }
            ExprKind::StructLit { fields, .. } => {
                let exprs: Vec<Idx<Expr>> = fields.iter().filter_map(|f| f.value).collect();
                self.join_list(&exprs, expr.span)
            }
            ExprKind::ArrayLit(volt_ast::ArrayLitKind::List(items)) => {
                let items = items.clone();
                self.join_list(&items, expr.span)
            }
            ExprKind::ArrayLit(volt_ast::ArrayLitKind::Repeat { value, count }) => {
                let (value, count) = (*value, *count);
                self.expr_domain(count);
                self.expr_domain(value)
            }
            ExprKind::TupleLit(items) => {
                let items = items.clone();
                self.join_list(&items, expr.span)
            }
        }
    }

    fn join_list(&mut self, exprs: &[Idx<Expr>], fallback_span: Span) -> DomainId {
        let mut dom = DomainId::Timeless;
        let mut dom_span = fallback_span;
        for &e in exprs {
            let d = self.expr_domain(e);
            let e_span = self.ast.exprs[e].span;
            dom = self.join(dom, d, dom_span, e_span);
            if matches!(self.resolve_dom(d), DomainId::Explicit(_)) {
                dom_span = e_span;
            }
        }
        dom
    }

    fn def_domain(&mut self, def: DefId) -> DomainId {
        if let Some(&d) = self.signal_domains.get(&def) {
            return d;
        }
        // Sabitler, enum varyantları, generic'ler, döngü değişkenleri:
        // saatten bağımsız.
        DomainId::Timeless
    }

    fn builtin_of(&self, callee: Idx<Expr>) -> Option<BuiltinKind> {
        let def = self.res.resolutions.get(&callee)?;
        match self.res.def_kind(*def) {
            DefKind::Builtin(kind) => Some(kind),
            _ => None,
        }
    }

    /// join_domains — çıkarımın kalbi (K5).
    fn join(&mut self, a: DomainId, b: DomainId, sa: Span, sb: Span) -> DomainId {
        let (ra, rb) = (self.resolve_dom(a), self.resolve_dom(b));
        match (ra, rb) {
            // Hata yayılımı.
            (DomainId::Error, _) | (_, DomainId::Error) => DomainId::Error,
            // Timeless her şeyle birleşir (sabitler).
            (DomainId::Timeless, x) | (x, DomainId::Timeless) => x,
            // Aynı domain → sorun yok.
            (DomainId::Explicit(x), DomainId::Explicit(y)) if x == y => ra,
            // FARKLI DOMAIN → CDC İHLALİ (glitch riski).
            (DomainId::Explicit(x), DomainId::Explicit(y)) => {
                self.err_cdc_combinational(x, y, sa, sb);
                DomainId::Error
            }
            // Çözülmemiş → kısıt biriktir.
            (DomainId::Unresolved(v), x) | (x, DomainId::Unresolved(v)) => {
                if !matches!(x, DomainId::Unresolved(w) if w == v) {
                    self.vars[v.0 as usize] = Some(x);
                }
                x
            }
        }
    }

    /// E3001 (kombinasyonel biçim, §4) — iki operand span'i + domain
    /// tanım satırlarına ikincil etiketler.
    fn err_cdc_combinational(&mut self, x: u32, y: u32, sa: Span, sb: Span) {
        let diag = Diagnostic::error(
            ErrorCode::E3001,
            lstr!(
                en: "different clock domains cannot be combined combinationally";
                tr: "farklı saat alanları kombinasyonel olarak birleşemez"
            ),
            LabeledSpan::primary(sa, format!("@{}", self.domain_name(x))),
            lstr!(
                en: "synchronize first: let s = sync(<signal>, <clock of {}>); \
                     then combine",
                    self.domain_name(y);
                tr: "önce senkronize edin: let s = sync(<sinyal>, <{} saati>); \
                     sonra birleştirin",
                    self.domain_name(y)
            ),
        )
        .with_secondary(sb, format!("@{}", self.domain_name(y)))
        .with_secondary(
            self.domain_span(x),
            lstr!(
                en: "@{} defined here", self.domain_name(x);
                tr: "@{} burada tanımlı", self.domain_name(x)
            ),
        )
        .with_secondary(
            self.domain_span(y),
            lstr!(
                en: "@{} defined here", self.domain_name(y);
                tr: "@{} burada tanımlı", self.domain_name(y)
            ),
        )
        .with_note(
            NoteKind::Reason,
            lstr!(
                en: "when signals from two clock domains meet at a gate they \
                     produce a transient pulse (glitch); the next \
                     register captures this pulse incorrectly";
                tr: "iki saat alanından gelen sinyaller kapıda birleşince \
                     geçici darbe (glitch) üretir; bu darbe sonraki \
                     register'da yanlış yakalanır"
            ),
        );
        self.diagnostics.push(diag);
    }

    // ═══ K9 — sync() köprüsü ══════════════════════════════════════

    /// Tek meşru CDC geçiş yolu: çıkış hedef saat alanındadır.
    fn sync_domain(&mut self, args: &[Idx<Expr>], span: Span) -> DomainId {
        let Some(&data) = args.first() else {
            return DomainId::Error;
        };
        let src = self.expr_domain(data);
        let dst = match args.get(1) {
            Some(&clk) => self.expr_domain(clk),
            None => DomainId::Error,
        };

        // Aynı domain → gereksiz senkronizatör (W3002).
        if let (DomainId::Explicit(a), DomainId::Explicit(b)) =
            (self.resolve_dom(src), self.resolve_dom(dst))
        {
            if a == b {
                self.diagnostics.push(
                    Diagnostic::warning(
                        ErrorCode::W3002,
                        lstr!(
                            en: "sync() is unnecessary within the same clock domain";
                            tr: "sync() aynı saat alanı içinde gereksiz"
                        ),
                        LabeledSpan::primary(
                            span,
                            lstr!(
                                en: "source and destination are both @{}", self.domain_name(a);
                                tr: "kaynak ve hedef @{}", self.domain_name(a)
                            ),
                        ),
                        lstr!(
                            en: "a direct assignment is enough — remove the sync() call";
                            tr: "doğrudan atama yeterli — sync() çağrısını kaldırın"
                        ),
                    )
                    .with_note(
                        NoteKind::Reason,
                        lstr!(
                            en: "a synchronizer is only needed when crossing between \
                                 domains; within the same domain it adds 2 cycles of latency";
                            tr: "senkronizatör yalnız alanlar arası geçişte gerekir; \
                                 aynı alanda 2 çevrim gecikme ekler"
                        ),
                    ),
                );
            }
        }

        // Çok bitli veri uyarısı (W3003).
        if let Some(&ty) = self.tyck.expr_types.get(&data) {
            if let Some(w) = self.tyck.types.width_of(ty) {
                if w > 1 {
                    self.diagnostics.push(
                        Diagnostic::warning(
                            ErrorCode::W3003,
                            lstr!(
                                en: "two-flop synchronization does not guarantee \
                                     bit coherence for {w}-bit signals";
                                tr: "{w}-bit sinyal için iki-flop senkronizasyonu \
                                     bit tutarlılığı garanti etmez"
                            ),
                            LabeledSpan::primary(
                                span,
                                lstr!(
                                    en: "bits may be captured on different edges";
                                    tr: "bitler farklı kenarlarda yakalanabilir"
                                ),
                            ),
                            lstr!(
                                en: "for multi-bit data use one of:\n           \
                                     AsyncFifo<T, N>   — data streams\n           \
                                     HandshakeSync<T>  — single transfers\n           \
                                     gray coding       — counters";
                                tr: "çok bitli veri için şunlardan birini kullanın:\n           \
                                     AsyncFifo<T, N>   — veri akışları\n           \
                                     HandshakeSync<T>  — tek transferler\n           \
                                     gray kodlama      — sayaçlar"
                            ),
                        )
                        .with_note(
                            NoteKind::Reason,
                            lstr!(
                                en: "a two-flop synchronizer synchronizes each bit \
                                     independently; when bits are captured on different \
                                     clock edges an invalid intermediate value appears";
                                tr: "iki-flop senkronizatör her biti bağımsız senkronize \
                                     eder; bitler farklı saat kenarlarında yakalanınca \
                                     geçersiz ara değer oluşur"
                            ),
                        ),
                    );
                }
            }
        }

        dst
    }

    // ═══ K8 — modül örnekleme ═════════════════════════════════════

    fn check_instance(&mut self, inst: &volt_ast::InstanceDecl) {
        let Some(inst_def) = self.decl_def(&inst.name) else {
            return;
        };
        if let Some(&prim) = self.res.instance_builtin.get(&inst_def) {
            self.check_builtin_instance(inst, inst_def, prim);
            return;
        }
        let Some(&target) = self.res.instance_module.get(&inst_def) else {
            // Struct literal veya çözülmemiş hedef — bağlama ifadeleri
            // yine de yayılıma girer.
            for b in &inst.bindings {
                if let Some(e) = b.value {
                    self.expr_domain(e);
                }
            }
            return;
        };
        let Some(&item_idx) = self.res.item_of_def.get(&target) else {
            return;
        };
        let target_ports: &[Port] = match &self.ast.items_arena[item_idx].kind {
            ItemKind::Module(m) => &m.ports,
            ItemKind::Extern(x) => &x.ports,
            _ => return,
        };

        // Hedef modülün saat portları (K8 adım 1 hazırlığı).
        let target_clocks: Vec<&Port> = target_ports
            .iter()
            .filter(|p| {
                self.decl_def(&p.name)
                    .is_some_and(|def| self.is_clock_def(def))
            })
            .collect();

        // 1. Saat bağlantılarından modülün domain haritasını çıkar.
        let mut mapping: HashMap<DefId, DomainId> = HashMap::new();
        for b in &inst.bindings {
            let Some(port) = target_ports
                .iter()
                .find(|p| p.name.text == b.port_name.text)
            else {
                continue;
            };
            let Some(key) = self.port_domain_key(port, &target_clocks) else {
                continue;
            };
            let is_clock = self
                .decl_def(&port.name)
                .is_some_and(|def| self.is_clock_def(def));
            if is_clock {
                let actual = self.binding_domain(b);
                mapping.insert(key, actual);
            }
        }

        // 2. Diğer portları bu haritaya göre kontrol et.
        let mut port_domains: HashMap<String, DomainId> = HashMap::new();
        for port in target_ports {
            let expected = self.expected_port_domain(port, &target_clocks, &mapping);
            port_domains.insert(port.name.text.clone(), expected);
        }
        for b in &inst.bindings {
            let Some(port) = target_ports
                .iter()
                .find(|p| p.name.text == b.port_name.text)
            else {
                if let Some(e) = b.value {
                    self.expr_domain(e);
                }
                continue;
            };
            let is_clock = self
                .decl_def(&port.name)
                .is_some_and(|def| self.is_clock_def(def));
            if is_clock {
                continue;
            }
            let expected = port_domains
                .get(&port.name.text)
                .copied()
                .unwrap_or(DomainId::Timeless);
            let actual = self.binding_domain(b);
            let value_span = match b.value {
                Some(e) => self.ast.exprs[e].span,
                None => b.span,
            };
            self.check_compat(expected, actual, b.span, value_span);
        }

        self.instance_ports.insert(inst_def, port_domains);
    }

    /// Yerleşik CDC primitifi (ADR-0027, K8'in yerleşik eşleniği):
    /// yazma/kaynak domain'i `wr_clk`/`src_clk` bağlamasından,
    /// okuma/hedef domain'i `rd_clk`/`dst_clk` bağlamasından gelir.
    /// Port→domain haritası doldurulur ki `f.rd_data` gibi alan
    /// okumaları hedef alanda görünsün ve mevcut CDC denetimleri
    /// (K5-K7) doğal olarak çalışsın. Kaynak taraf girişleri kaynak
    /// alanda olmalı (check_compat). PulseSync her örneklemede W3005
    /// kullanım kısıtını hatırlatır.
    fn check_builtin_instance(
        &mut self,
        inst: &volt_ast::InstanceDecl,
        inst_def: DefId,
        prim: crate::builtin::BuiltinPrim,
    ) {
        use crate::builtin::{DomainRole, PortKind};

        // 1. Saat bağlamalarından src/dst domain'leri.
        let mut src_dom = DomainId::Error;
        let mut dst_dom = DomainId::Error;
        for b in &inst.bindings {
            let Some(port) = prim.port(&b.port_name.text) else {
                continue;
            };
            if port.kind == PortKind::Clock {
                let dom = self.binding_domain(b);
                match port.role {
                    DomainRole::Src => src_dom = dom,
                    DomainRole::Dst => dst_dom = dom,
                }
            }
        }

        // 2. Port→domain haritası (alan okumaları için).
        let mut port_domains: HashMap<String, DomainId> = HashMap::new();
        for port in prim.ports() {
            let dom = match port.role {
                DomainRole::Src => src_dom,
                DomainRole::Dst => dst_dom,
            };
            port_domains.insert(port.name.to_string(), dom);
        }

        // 3. Saat dışı giriş bağlamaları kendi tarafının alanında olmalı.
        for b in &inst.bindings {
            let Some(port) = prim.port(&b.port_name.text) else {
                if let Some(e) = b.value {
                    self.expr_domain(e);
                }
                continue;
            };
            if port.kind == PortKind::Clock {
                continue;
            }
            let expected = port_domains
                .get(port.name)
                .copied()
                .unwrap_or(DomainId::Error);
            let actual = self.binding_domain(b);
            let value_span = match b.value {
                Some(e) => self.ast.exprs[e].span,
                None => b.span,
            };
            self.check_compat(expected, actual, b.span, value_span);
        }

        self.instance_ports.insert(inst_def, port_domains);

        // 4a. DualPortRam kullanım kısıtı (ADR-0029 W3006): iki port
        //     aynı adrese aynı çevrimde yazarsa B portu kazanır; adres
        //     çakışması statik olarak bilinemediğinden her örneklemede
        //     hatırlatılır (W3005 kalıbı).
        if prim == crate::builtin::BuiltinPrim::DualPortRam {
            self.diagnostics.push(Diagnostic::warning(
                ErrorCode::W3006,
                lstr!(en: "DualPortRam write-write collisions resolve in favor of port B";
                      tr: "DualPortRam yazma-yazma çakışmalarında B portu kazanır"),
                LabeledSpan::primary(
                    inst.name.span,
                    lstr!(en: "simultaneous writes to the same address are not detected";
                          tr: "aynı adrese eş zamanlı yazma algılanmaz"),
                ),
                lstr!(en: "ensure the two ports never write the same address in the same cycle, \
                           or arbitrate writes before the RAM";
                      tr: "iki portun aynı çevrimde aynı adrese yazmadığından emin olun ya da \
                           yazmaları RAM'den önce arbitre edin"),
            ));
        }

        // 4b. PulseSync kullanım kısıtı (ADR-0027 W3005): toggle
        //     protokolü sık darbeleri yutar; saat oranı statik olarak
        //     bilinemediğinden her örneklemede hatırlatılır.
        if prim == crate::builtin::BuiltinPrim::PulseSync {
            self.diagnostics.push(Diagnostic::warning(
                ErrorCode::W3005,
                lstr!(en: "PulseSync requires spacing between source pulses";
                      tr: "PulseSync kaynak darbeleri arasında aralık gerektirir"),
                LabeledSpan::primary(
                    inst.name.span,
                    lstr!(en: "toggle protocol drops closely spaced pulses";
                          tr: "toggle protokolü sık darbeleri düşürür"),
                ),
                lstr!(en: "guarantee at least 3 destination clock cycles between consecutive \
                           pulses, or use HandshakeSync/AsyncFifo";
                      tr: "ardışık darbeler arasında en az 3 hedef saat çevrimi bırakın ya da \
                           HandshakeSync/AsyncFifo kullanın"),
            ));
        }
    }

    /// Hedef portun domain anahtarı: açık @Domain tanımı ya da hedefin
    /// clock portu (tek saat kuralı hedef modülde de geçerli).
    fn port_domain_key(&self, port: &Port, target_clocks: &[&Port]) -> Option<DefId> {
        if let Some(ann) = &port.domain {
            return self.use_def(ann.span);
        }
        let def = self.decl_def(&port.name)?;
        if self.is_clock_def(def) {
            return Some(def);
        }
        match target_clocks {
            [single] => {
                if let Some(ann) = &single.domain {
                    self.use_def(ann.span)
                } else {
                    self.decl_def(&single.name)
                }
            }
            _ => None,
        }
    }

    /// K8 adım 2: haritada eşleşme yoksa global domain tanımı geçerli;
    /// o da yoksa Timeless (saatsiz hedef modül).
    fn expected_port_domain(
        &mut self,
        port: &Port,
        target_clocks: &[&Port],
        mapping: &HashMap<DefId, DomainId>,
    ) -> DomainId {
        let Some(key) = self.port_domain_key(port, target_clocks) else {
            return DomainId::Timeless;
        };
        if let Some(&dom) = mapping.get(&key) {
            return dom;
        }
        match self.by_decl.get(&key) {
            Some(&id) => DomainId::Explicit(id),
            None => DomainId::Timeless,
        }
    }

    fn binding_domain(&mut self, b: &volt_ast::PortBinding) -> DomainId {
        match b.value {
            Some(e) => self.expr_domain(e),
            // `clk:` kısayolu — yerel isim port adıyla aynı.
            None => self
                .use_def(b.port_name.span)
                .map(|def| self.def_domain(def))
                .unwrap_or(DomainId::Timeless),
        }
    }
}
