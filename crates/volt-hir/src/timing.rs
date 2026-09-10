//! L1 zamanlama denetimi (ADR-0037): `@strict_timing` modüllerinde
//! `Delayed<T, N>` gecikme çıkarımı ve E5010.
//!
//! Model: her değer, boru hattına girişten belirli sayıda çevrim sonra
//! geçerlidir — bu onun *gecikmesidir*. Giriş portu 0, register kaynak
//! gecikmesi + 1, kombinasyonel `let` operandlarının birleşimidir.
//! Sabitler ve literaller zamanlama taşımaz (her gecikmeyle uyumlu).
//! Geri beslemeli register'lar (sayaç, pc) sabit bir gecikmeye
//! oturmadığından çözümsüz kalır ve serbest sayılır; kesin denetim
//! açık `Delayed` anotasyonlarıyla kurulur.
//!
//! Kaçış noktaları (bilinçli yeniden zamanlama):
//!   * `delay<K>(x)` — gecikmeye K ekler;
//!   * açık anotasyonlu `let` (`let f : Delayed<u32, 2> = ...`) —
//!     sonucun gecikmesini BİLDİRİR, başlatıcısının içindeki karışım
//!     denetimi bastırılır (forwarding/bypass bu kapıdan yazılır).
//!
//! `@strict_timing` olmayan modüllerde bu geçit hiç çalışmaz — mevcut
//! davranış bire bir korunur (geriye uyumluluk, ADR-0037 §3).

use std::collections::{HashMap, HashSet};

use volt_ast::{
    Block, BlockStmt, ElseBranch, Expr, ExprKind, Idx, IfStmt, ItemKind, MatchArmBody, ModuleDecl,
    PortDir, SourceFile, StmtKind, TypeRef, TypeRefKind,
};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use crate::resolve::{DefId, DefKind, ResolveResult};

/// Bir tanımın boru hattı gecikmesi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Delay {
    /// Sabit noktada henüz bilinmiyor; yakınsamazsa (döngü) `Any` olur.
    Todo,
    /// Zamanlama taşımıyor — sabit, literal veya çözümsüz döngü üyesi.
    Any,
    /// Girişten tam `n` çevrim sonra geçerli.
    Exact(u32),
}

/// Tanıma akan bir kaynak: `reg r <= rhs` veya `let x = rhs` sağ tarafı.
#[derive(Debug, Clone, Copy)]
struct Source {
    rhs: Idx<Expr>,
    span: Span,
}

/// `@strict_timing` modüllerini denetler; diğerlerine hiç dokunmaz.
pub fn check_timing(ast: &SourceFile, res: &ResolveResult) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for &item_idx in &ast.items {
        let item = &ast.items_arena[item_idx];
        if !item.attrs.iter().any(|a| a.name.text == "strict_timing") {
            continue;
        }
        if let ItemKind::Module(m) = &item.kind {
            let mut mt = ModuleTiming {
                ast,
                res,
                diags: &mut diags,
                declared: HashMap::new(),
                delays: HashMap::new(),
                sources: HashMap::new(),
                regs: HashSet::new(),
                order: Vec::new(),
            };
            mt.run(m);
        }
    }
    diags
}

struct ModuleTiming<'a> {
    ast: &'a SourceFile,
    res: &'a ResolveResult,
    diags: &'a mut Vec<Diagnostic>,
    /// Açık `Delayed<T, N>` anotasyonları: tanım → (N, anotasyon span'i).
    declared: HashMap<DefId, (u32, Span)>,
    delays: HashMap<DefId, Delay>,
    sources: HashMap<DefId, Vec<Source>>,
    regs: HashSet<DefId>,
    order: Vec<DefId>,
}

impl<'a> ModuleTiming<'a> {
    fn run(&mut self, m: &ModuleDecl) {
        for port in &m.ports {
            let Some(&def) = self.res.decl_spans.get(&port.name.span) else {
                continue;
            };
            self.order.push(def);
            if let Some((n, _)) = self.declared_of(port.ty) {
                self.delays.insert(def, Delay::Exact(n));
                continue;
            }
            let base = match self.ast.types[port.ty].kind {
                // Saat/sıfırlama sinyalleri zamanlama taşımaz.
                TypeRefKind::Clock | TypeRefKind::Reset(_) => Delay::Any,
                // Giriş portu tanım gereği 0 çevrim gecikmelidir (ADR-0037).
                _ if port.direction == PortDir::In => Delay::Exact(0),
                // Çıkış portu sürücüsünden hesaplanır.
                _ => Delay::Todo,
            };
            self.delays.insert(def, base);
        }
        for &stmt_idx in &m.body {
            self.collect_stmt(stmt_idx);
        }
        self.fixpoint();
        self.check_all();
    }

    // ═══ Toplama: tanımlar, anotasyonlar, kaynak ifadeler ═════════

    fn collect_stmt(&mut self, stmt_idx: Idx<volt_ast::Stmt>) {
        match &self.ast.stmts[stmt_idx].kind {
            StmtKind::Reg(r) => {
                let Some(&def) = self.res.decl_spans.get(&r.name.span) else {
                    return;
                };
                self.order.push(def);
                self.regs.insert(def);
                if let Some(ty) = r.ty {
                    if let Some((n, span)) = self.declared_of(ty) {
                        self.declared.insert(def, (n, span));
                        self.delays.insert(def, Delay::Exact(n));
                        return;
                    }
                }
                // Pipeline desugar'ının ürettiği aşama register'ları
                // (ADR-0038): sabitlenmiş gecikme açık anotasyon gibidir.
                if let Some(&(n, span)) = self.ast.timing.pinned.get(&r.name.span) {
                    self.declared.insert(def, (n, span));
                    self.delays.insert(def, Delay::Exact(n));
                    return;
                }
                self.delays.insert(def, Delay::Todo);
            }
            StmtKind::Let(l) => self.collect_let(l),
            StmtKind::Wire(w) => {
                let Some(&def) = self.res.decl_spans.get(&w.name.span) else {
                    return;
                };
                self.order.push(def);
                if let Some((n, span)) = self.declared_of(w.ty) {
                    self.declared.insert(def, (n, span));
                    self.delays.insert(def, Delay::Exact(n));
                } else {
                    self.delays.insert(def, Delay::Todo);
                }
            }
            StmtKind::Assign(a) => {
                let rhs = a.rhs;
                let span = self.ast.exprs[rhs].span;
                if let Some(&def) = self.res.use_spans.get(&a.lhs.base.span) {
                    self.sources
                        .entry(def)
                        .or_default()
                        .push(Source { rhs, span });
                }
            }
            StmtKind::On(on) => self.collect_block(on.body),
            StmtKind::Comb(b) => self.collect_block(*b),
            StmtKind::For(f) => self.collect_block(f.body),
            StmtKind::Instance(_) | StmtKind::Expr(_) | StmtKind::Error => {}
        }
    }

    fn collect_let(&mut self, l: &volt_ast::LetDecl) {
        let Some(&def) = self.res.decl_spans.get(&l.name.span) else {
            return;
        };
        self.order.push(def);
        let span = self.ast.exprs[l.value].span;
        self.sources
            .entry(def)
            .or_default()
            .push(Source { rhs: l.value, span });
        if let Some(ty) = l.ty {
            if let Some((n, aspan)) = self.declared_of(ty) {
                self.declared.insert(def, (n, aspan));
                self.delays.insert(def, Delay::Exact(n));
                return;
            }
        }
        // `stage(...)` içeren aşama-yerel let'ler desugar'da kendi
        // aşamalarının gecikmesine sabitlenir (ADR-0038 §3): otomatik
        // yeniden zamanlama iddiası — karışım denetimi bastırılır.
        if let Some(&(n, aspan)) = self.ast.timing.pinned.get(&l.name.span) {
            self.declared.insert(def, (n, aspan));
            self.delays.insert(def, Delay::Exact(n));
            return;
        }
        self.delays.insert(def, Delay::Todo);
    }

    fn collect_block(&mut self, block_idx: Idx<Block>) {
        // Koşullar (if/match başlıkları) kontrol sinyalidir ve gecikme
        // denetiminin dışındadır (ADR-0037 §2) — yalnız atanan veri izlenir.
        let block = &self.ast.blocks[block_idx];
        for stmt in &block.stmts {
            match stmt {
                BlockStmt::NonBlockAssign { lhs, rhs, span }
                | BlockStmt::BlockAssign { lhs, rhs, span } => {
                    if let Some(&def) = self.res.use_spans.get(&lhs.base.span) {
                        self.sources.entry(def).or_default().push(Source {
                            rhs: *rhs,
                            span: *span,
                        });
                    }
                }
                BlockStmt::If(ifstmt) => self.collect_if(ifstmt),
                BlockStmt::Match(m) => {
                    for arm in &m.arms {
                        if let MatchArmBody::Block(b) = &arm.body {
                            self.collect_block(*b);
                        }
                    }
                }
                BlockStmt::Let(l) => self.collect_let(l),
                BlockStmt::For(f) => self.collect_block(f.body),
                BlockStmt::Error => {}
            }
        }
    }

    fn collect_if(&mut self, ifstmt: &IfStmt) {
        self.collect_block(ifstmt.then_block);
        match &ifstmt.else_branch {
            Some(ElseBranch::Block(b)) => self.collect_block(*b),
            Some(ElseBranch::If(inner)) => self.collect_if(inner),
            None => {}
        }
    }

    /// `Delayed<T, N>` anotasyonu: parser'ın yan tablosundan okur ve
    /// N'i doğrular. V0'da N tamsayı literali olmalıdır.
    fn declared_of(&mut self, ty: Idx<TypeRef>) -> Option<(u32, Span)> {
        let &(cycles_expr, span) = self.ast.timing.delayed_types.get(&ty)?;
        match &self.ast.exprs[cycles_expr].kind {
            ExprKind::IntLit { value, .. } if *value <= u128::from(u16::MAX) => {
                Some((*value as u32, span))
            }
            _ => {
                self.diags.push(Diagnostic::error(
                    ErrorCode::E5010,
                    lstr!(en: "invalid Delayed cycle count"; tr: "geçersiz Delayed çevrim sayısı"),
                    LabeledSpan::primary(
                        span,
                        lstr!(en: "cycle count must be an integer literal (0..=65535)"; tr: "çevrim sayısı tamsayı literali olmalı (0..=65535)"),
                    ),
                    lstr!(en: "write it as Delayed<u32, 3>"; tr: "Delayed<u32, 3> biçiminde yazın"),
                ));
                None
            }
        }
    }

    // ═══ Sabit nokta: gecikme çıkarımı ════════════════════════════

    fn fixpoint(&mut self) {
        let rounds = self.order.len() + 2;
        for _ in 0..rounds {
            for i in 0..self.order.len() {
                let def = self.order[i];
                if self.declared.contains_key(&def) {
                    continue; // açık anotasyon sabittir
                }
                let new = self.compute_def(def);
                if new != Delay::Todo {
                    self.delays.insert(def, new);
                }
            }
        }
        // Yakınsamayanlar (geri besleme döngüleri: pc, sayaç, regfile)
        // sabit bir gecikmeye oturmaz — serbest bırakılır.
        for d in self.delays.values_mut() {
            if *d == Delay::Todo {
                *d = Delay::Any;
            }
        }
    }

    fn compute_def(&mut self, def: DefId) -> Delay {
        let Some(srcs) = self.sources.get(&def).cloned() else {
            return *self.delays.get(&def).unwrap_or(&Delay::Any);
        };
        let is_reg = self.regs.contains(&def);
        let mut acc = Delay::Any;
        for s in &srcs {
            // Kendini tutma (`r <= r`) gecikme denklemine katılmaz.
            if is_reg && self.is_path_to(s.rhs, def) {
                continue;
            }
            let d = self.delay_of(s.rhs, false).0;
            acc = match (acc, d) {
                (_, Delay::Todo) | (Delay::Todo, _) => return Delay::Todo,
                (Delay::Any, x) | (x, Delay::Any) => x,
                (Delay::Exact(a), Delay::Exact(b)) if a == b => Delay::Exact(a),
                // Uyumsuz kaynaklar: hata check_all'da verilir; çıkarım
                // kaskadı bastırmak için serbest kalır.
                _ => return Delay::Any,
            };
        }
        if is_reg {
            match acc {
                Delay::Exact(n) => Delay::Exact(n.saturating_add(1)),
                other => other,
            }
        } else {
            acc
        }
    }

    // ═══ Denetim: E5010 ═══════════════════════════════════════════

    fn check_all(&mut self) {
        for i in 0..self.order.len() {
            let def = self.order[i];
            let annotated = self.declared.get(&def).copied();
            let Some(srcs) = self.sources.get(&def).cloned() else {
                continue;
            };
            let is_reg = self.regs.contains(&def);
            if !is_reg {
                // Açık anotasyonlu `let` bir yeniden zamanlama İDDİASIDIR:
                // başlatıcısının içindeki karışım denetimi bastırılır.
                if annotated.is_none() {
                    for s in &srcs {
                        self.delay_of(s.rhs, true);
                    }
                }
                continue;
            }
            for s in &srcs {
                if self.is_path_to(s.rhs, def) {
                    continue; // tutma (hold) yazımı serbest
                }
                let (d, _) = self.delay_of(s.rhs, true);
                let Some((want, aspan)) = annotated else {
                    continue;
                };
                if let Delay::Exact(n) = d {
                    if n.saturating_add(1) != want {
                        let diff = n + 1;
                        self.diags.push(
                            Diagnostic::error(
                                ErrorCode::E5010,
                                lstr!(en: "timing misalignment in register write"; tr: "register yazımında zamanlama hizasızlığı"),
                                LabeledSpan::primary(
                                    s.span,
                                    lstr!(en: "source is {n} cycles — register would hold a {diff}-cycle value"; tr: "kaynak {n} çevrim — register {diff} çevrimlik değer tutar"),
                                ),
                                lstr!(en: "align the source with delay<K>(...) or fix the declared cycle count"; tr: "kaynağı delay<K>(...) ile hizalayın ya da bildirilen çevrim sayısını düzeltin"),
                            )
                            .with_secondary(
                                aspan,
                                lstr!(en: "register declared as {want} cycles here"; tr: "register burada {want} çevrim olarak bildirildi"),
                            )
                            .with_note(
                                NoteKind::Reason,
                                lstr!(en: "a register adds one cycle: a Delayed<_, {want}> register must be written from a value {} cycles old", want.saturating_sub(1); tr: "register bir çevrim ekler: Delayed<_, {want}> register'ı {} çevrim yaşında bir değerden yazılmalı", want.saturating_sub(1)),
                            ),
                        );
                    }
                }
            }
        }
    }

    /// İfadenin gecikmesi. `emit` açıksa Exact/Exact uyumsuzluklarında
    /// E5010 üretir ve kaskadı bastırmak için `Any` döner.
    /// Dönüş: (gecikme, temsilci span — Exact değerin kaynağı).
    fn delay_of(&mut self, e: Idx<Expr>, emit: bool) -> (Delay, Span) {
        let span = self.ast.exprs[e].span;
        let base = match &self.ast.exprs[e].kind {
            ExprKind::IntLit { .. }
            | ExprKind::BoolLit(_)
            | ExprKind::StringLit(_)
            | ExprKind::Todo { .. }
            | ExprKind::Error => (Delay::Any, span),
            ExprKind::Path(_) => (self.path_delay(e), span),
            ExprKind::Unary { operand, .. } => {
                let d = self.delay_of(*operand, emit);
                (d.0, span)
            }
            ExprKind::Binary { lhs, rhs, .. } => {
                let (l, r) = (*lhs, *rhs);
                let a = self.delay_of(l, emit);
                let b = self.delay_of(r, emit);
                self.combine(a, b, emit)
            }
            ExprKind::Index { base, index } => {
                let a = self.delay_of(*base, emit);
                let b = self.delay_of(*index, emit);
                self.combine(a, b, emit)
            }
            ExprKind::Range { base, hi, lo } => {
                let a = self.delay_of(*base, emit);
                let b = self.delay_of(*hi, emit);
                let c = self.delay_of(*lo, emit);
                let ab = self.combine(a, b, emit);
                self.combine(ab, c, emit)
            }
            ExprKind::PartSelect {
                base, start, width, ..
            } => {
                let a = self.delay_of(*base, emit);
                let b = self.delay_of(*start, emit);
                let c = self.delay_of(*width, emit);
                let ab = self.combine(a, b, emit);
                self.combine(ab, c, emit)
            }
            ExprKind::Field { base, .. } => {
                let d = self.delay_of(*base, emit);
                (d.0, span)
            }
            ExprKind::Cast { expr, .. } => {
                let d = self.delay_of(*expr, emit);
                (d.0, span)
            }
            // Koşul kontrol sinyalidir — yalnız dallar birleştirilir.
            ExprKind::If {
                then_expr,
                else_expr,
                ..
            } => {
                let a = self.delay_of(*then_expr, emit);
                let b = self.delay_of(*else_expr, emit);
                self.combine(a, b, emit)
            }
            ExprKind::Match { arms, .. } => {
                let bodies: Vec<Idx<Expr>> = arms
                    .iter()
                    .filter_map(|arm| match &arm.body {
                        MatchArmBody::Expr(e) => Some(*e),
                        MatchArmBody::Block(_) => None,
                    })
                    .collect();
                let mut acc = (Delay::Any, span);
                for b in bodies {
                    let d = self.delay_of(b, emit);
                    acc = self.combine(acc, d, emit);
                }
                acc
            }
            ExprKind::Call { args, .. } => {
                let args = args.clone();
                let mut acc = (Delay::Any, span);
                for a in args {
                    let d = self.delay_of(a, emit);
                    acc = self.combine(acc, d, emit);
                }
                acc
            }
            ExprKind::StructLit { fields, .. } => {
                let vals: Vec<Idx<Expr>> = fields.iter().filter_map(|f| f.value).collect();
                let mut acc = (Delay::Any, span);
                for v in vals {
                    let d = self.delay_of(v, emit);
                    acc = self.combine(acc, d, emit);
                }
                acc
            }
            ExprKind::ArrayLit(kind) => {
                let vals: Vec<Idx<Expr>> = match kind {
                    volt_ast::ArrayLitKind::List(items) => items.clone(),
                    volt_ast::ArrayLitKind::Repeat { value, .. } => vec![*value],
                };
                let mut acc = (Delay::Any, span);
                for v in vals {
                    let d = self.delay_of(v, emit);
                    acc = self.combine(acc, d, emit);
                }
                acc
            }
            ExprKind::TupleLit(items) => {
                let items = items.clone();
                let mut acc = (Delay::Any, span);
                for v in items {
                    let d = self.delay_of(v, emit);
                    acc = self.combine(acc, d, emit);
                }
                acc
            }
        };
        // `delay<K>(x)` sarmalayıcıları: gecikmeye K ekler (ADR-0037).
        let mut result = base;
        if let Some(wraps) = self.ast.timing.delay_exprs.get(&e).cloned() {
            for (cycles_expr, wrap_span) in wraps {
                let k = match &self.ast.exprs[cycles_expr].kind {
                    ExprKind::IntLit { value, .. } if *value <= u128::from(u16::MAX) => {
                        *value as u32
                    }
                    _ => {
                        if emit {
                            self.diags.push(Diagnostic::error(
                                ErrorCode::E5010,
                                lstr!(en: "invalid delay<K> cycle count"; tr: "geçersiz delay<K> çevrim sayısı"),
                                LabeledSpan::primary(
                                    wrap_span,
                                    lstr!(en: "K must be an integer literal (0..=65535)"; tr: "K tamsayı literali olmalı (0..=65535)"),
                                ),
                                lstr!(en: "write it as delay<1>(x)"; tr: "delay<1>(x) biçiminde yazın"),
                            ));
                        }
                        // Kaskad bastırma: geçersiz K sonucu serbest bırakır.
                        result = (Delay::Any, wrap_span);
                        continue;
                    }
                };
                result = match result.0 {
                    Delay::Exact(n) => (Delay::Exact(n.saturating_add(k)), wrap_span),
                    // Zamanlamasız değere delay eklemek onu sabitler.
                    Delay::Any => (Delay::Exact(k), wrap_span),
                    Delay::Todo => (Delay::Todo, wrap_span),
                };
            }
        }
        result
    }

    fn path_delay(&self, e: Idx<Expr>) -> Delay {
        let Some(&def) = self.res.resolutions.get(&e) else {
            return Delay::Any;
        };
        match self.res.def_kind(def) {
            DefKind::Port { .. } | DefKind::Register | DefKind::Wire | DefKind::LocalBinding => {
                *self.delays.get(&def).unwrap_or(&Delay::Any)
            }
            // Sabitler, enum varyantları, döngü değişkenleri, generic'ler:
            // zamanlama taşımaz.
            _ => Delay::Any,
        }
    }

    /// İki alt sonucu birleştirir; `emit` açıkken Exact/Exact
    /// uyuşmazlığında E5010 üretir ve kaskadı bastırmak için Any döner.
    fn combine(&mut self, a: (Delay, Span), b: (Delay, Span), emit: bool) -> (Delay, Span) {
        match (a.0, b.0) {
            (Delay::Todo, _) | (_, Delay::Todo) => (Delay::Todo, a.1),
            (Delay::Any, _) => (b.0, b.1),
            (_, Delay::Any) => (a.0, a.1),
            (Delay::Exact(x), Delay::Exact(y)) if x == y => (Delay::Exact(x), a.1),
            (Delay::Exact(x), Delay::Exact(y)) => {
                if emit {
                    let (old, young) = if x > y { (x, y) } else { (y, x) };
                    let diff = old - young;
                    self.diags.push(
                        Diagnostic::error(
                            ErrorCode::E5010,
                            lstr!(en: "timing misalignment"; tr: "zamanlama hizasızlığı"),
                            LabeledSpan::primary(
                                a.1,
                                lstr!(en: "{x} cycles"; tr: "{x} çevrim"),
                            ),
                            lstr!(en: "align the younger value with delay<{diff}>(...) or declare the result's delay explicitly (let x : Delayed<T, N> = ...)"; tr: "genç değeri delay<{diff}>(...) ile hizalayın ya da sonucun gecikmesini açıkça bildirin (let x : Delayed<T, N> = ...)"),
                        )
                        .with_secondary(b.1, lstr!(en: "{y} cycles"; tr: "{y} çevrim"))
                        .with_note(
                            NoteKind::Reason,
                            lstr!(en: "values from different pipeline stages cannot be combined directly"; tr: "farklı boru hattı aşamalarının değerleri doğrudan birleştirilemez"),
                        ),
                    );
                }
                (Delay::Any, a.1)
            }
        }
    }

    fn is_path_to(&self, e: Idx<Expr>, def: DefId) -> bool {
        matches!(&self.ast.exprs[e].kind, ExprKind::Path(_))
            && self.res.resolutions.get(&e) == Some(&def)
    }
}
