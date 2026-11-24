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
//!   * `delay<K>(x)` — gecikmeye K ekler; register EKLEMEZ, denetlenmeyen
//!     bir iddiadır (ADR-0127, #232). E5010'un yolu gerçek aşama
//!     register'ıdır (`stage_register_help`);
//!   * açık anotasyonlu `let` (`let f : Delayed<u32, 2> = ...`) —
//!     sonucun gecikmesini BİLDİRİR, başlatıcısının içindeki karışım
//!     denetimi bastırılır (forwarding/bypass bu kapıdan yazılır).
//!
//! Bildirim denetimi (ADR-0126, #354): register, `let`, `wire` ve çıkış
//! portundaki `Delayed<T, N>`, değerin gecikmesi tam biliniyorsa N ile
//! karşılaştırılır (`check_declared`); uyuşmazlık E5010. Değer aşamaları
//! karıştırıyorsa gecikmesi tam bilinmez ve bildirim yukarıdaki iddiadır.
//!
//! `@strict_timing` olmayan modüllerde bu geçit hiç çalışmaz — mevcut
//! davranış bire bir korunur (geriye uyumluluk, ADR-0037 §3).

use std::collections::{HashMap, HashSet};

use volt_ast::{
    Block, BlockStmt, ElseBranch, Expr, ExprKind, Idx, IfStmt, ItemKind, MatchArmBody, ModuleDecl,
    PortDir, SourceFile, StmtKind, TypeRef,
};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use crate::port_class::{DefTypes, SignalClass};
use crate::resolve::{DefId, DefKind, ResolveResult};
use crate::ty::Ty;
use crate::typeck::{DeadArms, TypeckResult};

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

/// Zamanlama geçidinin ürünleri: tanılar ve çıkarılan kesin gecikmeler.
#[derive(Debug, Default)]
pub struct TimingResult {
    pub diagnostics: Vec<Diagnostic>,
    /// `@strict_timing` modüllerinde sabit bir gecikmeye oturan tanımlar
    /// (port, register, let, wire) → çevrim. Serbest (`Any`) olanlar
    /// yoktur. Editör gecikme ipucu (ADR-0091) buradan okur — ikinci bir
    /// çıkarım yazılmaz.
    pub delays: HashMap<DefId, u32>,
}

/// `@strict_timing` modüllerini denetler; diğerlerine hiç dokunmaz.
pub fn check_timing(
    ast: &SourceFile,
    res: &ResolveResult,
    typeck: &TypeckResult,
) -> Vec<Diagnostic> {
    analyze_timing(ast, res, typeck).diagnostics
}

/// `check_timing` ile aynı geçit; tanılara ek olarak kesin gecikmeleri
/// de döndürür. `typeck`: tasarıma yazılmayan `match` kolları (onların
/// kaynakları donanımda yoktur, #331) ve E5010'un önerdiği aşama
/// register'ının tipi ile genişliği (ADR-0127).
pub fn analyze_timing(
    ast: &SourceFile,
    res: &ResolveResult,
    typeck: &TypeckResult,
) -> TimingResult {
    let dead = &typeck.dead_arms;
    let mut diags = Vec::new();
    let mut delays = HashMap::new();
    for &item_idx in &ast.items {
        let item = &ast.items_arena[item_idx];
        if !item.attrs.iter().any(|a| a.name.text == "strict_timing") {
            continue;
        }
        if let ItemKind::Module(m) = &item.kind {
            let mut mt = ModuleTiming {
                ast,
                res,
                dead,
                typeck,
                clocks: Vec::new(),
                diags: &mut diags,
                declared: HashMap::new(),
                delays: HashMap::new(),
                sources: HashMap::new(),
                regs: HashSet::new(),
                pinned: HashSet::new(),
                mixed: false,
                order: Vec::new(),
            };
            mt.run(m);
            delays.extend(mt.delays.into_iter().filter_map(|(def, d)| match d {
                Delay::Exact(n) => Some((def, n)),
                Delay::Todo | Delay::Any => None,
            }));
        }
    }
    TimingResult {
        diagnostics: diags,
        delays,
    }
}

struct ModuleTiming<'a> {
    ast: &'a SourceFile,
    res: &'a ResolveResult,
    /// Tasarıma yazılmayan `match` kolları: kaynak ve gecikme taşımaz.
    dead: &'a DeadArms,
    /// Tipler: E5010'un önerdiği aşama register'ının tipi ve genişliği.
    typeck: &'a TypeckResult,
    /// Modüldeki `on` bloklarının ayrı saat adları (öneri metni için).
    clocks: Vec<String>,
    diags: &'a mut Vec<Diagnostic>,
    /// Açık `Delayed<T, N>` anotasyonları: tanım → (N, anotasyon span'i).
    declared: HashMap<DefId, (u32, Span)>,
    delays: HashMap<DefId, Delay>,
    sources: HashMap<DefId, Vec<Source>>,
    regs: HashSet<DefId>,
    /// Pipeline desugar'ının aşamasına sabitlediği `let`'ler (ADR-0038
    /// §3): `declared`'dadır ama kullanıcının yazdığı bir bildirim değil,
    /// otomatik yeniden zamanlama iddiasıdır; bildirim denetimine girmez.
    pinned: HashSet<DefId>,
    /// Son `compute_def`/`delay_of` çağrısında farklı gecikmeli iki değer
    /// birleşti mi: sonucun `Any`'si zamanlamasızlıktan değil karışımdan.
    mixed: bool,
    order: Vec<DefId>,
}

impl<'a> ModuleTiming<'a> {
    fn run(&mut self, m: &ModuleDecl) {
        for port in &m.ports {
            let Some(&def) = self.res.decl_spans.get(&port.name.span) else {
                continue;
            };
            self.order.push(def);
            if let Some((n, span)) = self.declared_of(port.ty) {
                // Sınırdaki beyan: giriş portunda varsayım (sürücüsü
                // modülün dışında), çıkış portunda sürücüsüyle denetlenen
                // söz. Çıkarım onu ezmez (#354).
                self.declared.insert(def, (n, span));
                self.delays.insert(def, Delay::Exact(n));
                continue;
            }
            let base = match DefTypes::of(self.typeck).class(def) {
                // Saat/sıfırlama sinyalleri zamanlama taşımaz (takma adla
                // yazılanlar da, #234).
                SignalClass::Clock | SignalClass::Reset(_) => Delay::Any,
                // Giriş portu tanım gereği 0 çevrim gecikmelidir (ADR-0037).
                SignalClass::Data if port.direction == PortDir::In => Delay::Exact(0),
                // Çıkış portu sürücüsünden hesaplanır.
                SignalClass::Data => Delay::Todo,
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
            StmtKind::On(on) => {
                if let volt_ast::OnTrigger::Clock(clk) = &on.trigger {
                    if !self.clocks.contains(&clk.text) {
                        self.clocks.push(clk.text.clone());
                    }
                }
                self.collect_block(on.body);
            }
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
            self.pinned.insert(def);
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
                    let dead = self.dead;
                    for arm in dead.live(&m.arms) {
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
        if let ExprKind::IntLit { value, .. } = &self.ast.exprs[cycles_expr].kind {
            if let Ok(n) = u16::try_from(*value) {
                return Some((u32::from(n), span));
            }
        }
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
                _ => {
                    self.mixed = true;
                    return Delay::Any;
                }
            };
        }
        if is_reg {
            match acc {
                Delay::Exact(n) => Delay::Exact(n.saturating_add(1)),
                other @ (Delay::Todo | Delay::Any) => other,
            }
        } else {
            acc
        }
    }

    // ═══ Denetim: E5010 ═══════════════════════════════════════════

    fn check_all(&mut self) {
        for i in 0..self.order.len() {
            let def = self.order[i];
            let Some(srcs) = self.sources.get(&def).cloned() else {
                continue;
            };
            let is_reg = self.regs.contains(&def);
            match self.declared.get(&def).copied() {
                // Aşamasına sabitlenen let: otomatik yeniden zamanlama
                // iddiası, başlatıcısı denetlenmez (ADR-0038 §3).
                Some(_) if !is_reg && self.pinned.contains(&def) => {}
                Some(decl) => self.check_declared(def, &srcs, decl, is_reg),
                None => {
                    for s in &srcs {
                        if is_reg && self.is_path_to(s.rhs, def) {
                            continue; // tutma (hold) yazımı serbest
                        }
                        self.delay_of(s.rhs, true);
                    }
                }
            }
        }
    }

    /// Bildirilen `Delayed<_, N>` ile değerin gerçek gecikmesi: kuralın
    /// tek yeri (ADR-0126). Register her yazımıyla ayrı denetlenir
    /// (kaynak + 1 = N). `let`, `wire` ve çıkış portunun gecikmesi çıkarımın
    /// kendisinden (`compute_def`) gelir; tam biliniyorsa (`Exact`) N ile
    /// karşılaştırılır. Değer aşamaları karıştırıyorsa ya da zamanlama
    /// taşımıyorsa bildirim denetlenmez: karışımda yeniden zamanlama
    /// iddiasıdır (ADR-0037) ve başlatıcısının içindeki karışım da
    /// bastırılır.
    fn check_declared(
        &mut self,
        def: DefId,
        srcs: &[Source],
        (want, aspan): (u32, Span),
        is_reg: bool,
    ) {
        if !is_reg {
            // Çıkış portunun bildirimi sınırdaki sözdür, yeniden zamanlama
            // iddiası değil: sürücüsündeki karışım her zamanki gibi E5010.
            if matches!(self.res.def_kind(def), DefKind::Port { .. }) {
                for s in srcs {
                    self.delay_of(s.rhs, true);
                }
            }
            self.mixed = false;
            let got = self.compute_def(def);
            if self.mixed {
                return;
            }
            let Delay::Exact(got) = got else {
                return;
            };
            if got != want {
                let at = srcs.first().map_or(aspan, |s| s.span);
                self.diags.push(declared_mismatch(aspan, at, want, got));
            }
            return;
        }
        for s in srcs {
            if self.is_path_to(s.rhs, def) {
                continue; // tutma (hold) yazımı serbest
            }
            let (d, _) = self.delay_of(s.rhs, true);
            if let Delay::Exact(n) = d {
                if n.saturating_add(1) != want {
                    let diff = n + 1;
                    let art = volt_diagnostics::a_an(u64::from(diff));
                    let help = self.register_write_help(s.rhs, n, want);
                    self.diags.push(
                        Diagnostic::error(
                            ErrorCode::E5010,
                            lstr!(en: "timing misalignment in register write"; tr: "register yazımında zamanlama hizasızlığı"),
                            LabeledSpan::primary(
                                s.span,
                                lstr!(en: "source is {n} cycles — register would hold {art} {diff}-cycle value"; tr: "kaynak {n} çevrim — register {diff} çevrimlik değer tutar"),
                            ),
                            help,
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
            | ExprKind::Concat(_)
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
                self.combine_operands(a, b, [Some(l), Some(r)], emit)
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
                let (t, f) = (*then_expr, *else_expr);
                let a = self.delay_of(t, emit);
                let b = self.delay_of(f, emit);
                self.combine_operands(a, b, [Some(t), Some(f)], emit)
            }
            ExprKind::Match { arms, .. } => {
                let bodies: Vec<Idx<Expr>> = self
                    .dead
                    .live(arms)
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
                let vals: Vec<Idx<Expr>> = fields.iter().map(|f| f.value).collect();
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
                let literal =
                    if let ExprKind::IntLit { value, .. } = &self.ast.exprs[cycles_expr].kind {
                        u16::try_from(*value).ok()
                    } else {
                        None
                    };
                let Some(k) = literal.map(u32::from) else {
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
            DefKind::Module
            | DefKind::Domain
            | DefKind::Function
            | DefKind::Struct
            | DefKind::Enum
            | DefKind::EnumVariant { .. }
            | DefKind::Const
            | DefKind::TypeAlias
            | DefKind::ExternModule
            | DefKind::Instance
            | DefKind::LoopVar
            | DefKind::PatternBinding
            | DefKind::GenericParam
            | DefKind::DomainParam
            | DefKind::Builtin(_)
            | DefKind::Import
            | DefKind::Error => Delay::Any,
        }
    }

    /// İki alt sonucu birleştirir; `emit` açıkken Exact/Exact
    /// uyuşmazlığında E5010 üretir ve kaskadı bastırmak için Any döner.
    fn combine(&mut self, a: (Delay, Span), b: (Delay, Span), emit: bool) -> (Delay, Span) {
        self.combine_operands(a, b, [None, None], emit)
    }

    /// `combine`; `ops`, iki sonucun ifadeleri (doğrudan işlenenler
    /// biliniyorsa). E5010'un yolu genç işlenenin adını ve tipini
    /// buradan alır (ADR-0127).
    fn combine_operands(
        &mut self,
        a: (Delay, Span),
        b: (Delay, Span),
        ops: [Option<Idx<Expr>>; 2],
        emit: bool,
    ) -> (Delay, Span) {
        match (a.0, b.0) {
            (Delay::Todo, _) | (_, Delay::Todo) => (Delay::Todo, a.1),
            (Delay::Any, _) => (b.0, b.1),
            (_, Delay::Any) => (a.0, a.1),
            (Delay::Exact(x), Delay::Exact(y)) if x == y => (Delay::Exact(x), a.1),
            (Delay::Exact(x), Delay::Exact(y)) => {
                self.mixed = true;
                if emit {
                    let (old, young, young_op) = if x > y {
                        (x, y, ops[1])
                    } else {
                        (y, x, ops[0])
                    };
                    let help = format!(
                        "{}; {}",
                        self.stage_register_help(young_op, young, old),
                        delay_claim_note(old - young, old),
                    );
                    self.diags.push(
                        Diagnostic::error(
                            ErrorCode::E5010,
                            lstr!(en: "timing misalignment"; tr: "zamanlama hizasızlığı"),
                            LabeledSpan::primary(a.1, cycles_old(x)),
                            help,
                        )
                        .with_secondary(b.1, cycles_old(y))
                        .with_note(
                            NoteKind::Reason,
                            lstr!(en: "the result combines values from different cycles: the {young}-cycle value belongs to an item that entered {} later than the {old}-cycle value", cycle_count(old - young); tr: "sonuç farklı çevrimlerin değerlerini birleştiriyor: {young} çevrimlik değer, {old} çevrimlik değerden {} çevrim sonra giren bir öğeye ait", old - young),
                        ),
                    );
                }
                (Delay::Any, a.1)
            }
        }
    }

    /// E5010'un yolu (ADR-0127): `young` çevrimlik değeri `old` çevrime
    /// taşıyan aşama register'ları, kodu ve bedeliyle. Değer tek adlı
    /// bir sinyalse (`a`), tipi `0`/`false` ile başlatılabiliyorsa ve
    /// önerilen adlar boşsa kod adıyla yazılır; değilse genel biçim.
    fn stage_register_help(&self, value: Option<Idx<Expr>>, young: u32, old: u32) -> String {
        let k = old - young;
        let ty = value
            .and_then(|e| self.typeck.expr_types.get(&e))
            .map(|&id| self.typeck.types.ty(id));
        let bits = ty.and_then(|t| match t {
            Ty::Bool => Some(1),
            Ty::UInt { width } | Ty::SInt { width } | Ty::Bits { width } => Some(*width),
            Ty::Trit
            | Ty::Clock
            | Ty::Reset { .. }
            | Ty::Array { .. }
            | Ty::Tuple(_)
            | Ty::Struct(_)
            | Ty::Enum(_)
            | Ty::Instance(_)
            | Ty::Builtin { .. }
            | Ty::Delayed { .. }
            | Ty::IntLit
            | Ty::UIntFlex { .. }
            | Ty::SIntFlex { .. }
            | Ty::Error => None,
        });
        let cost = register_cost(k, bits);
        let block = match self.clocks.as_slice() {
            [clk] => lstr!(en: "the `on {clk}` block"; tr: "`on {clk}` bloğu"),
            _ => lstr!(en: "the `on` block of its clock"; tr: "saatinin `on` bloğu"),
        };
        let named = value.and_then(|e| self.named_stage_registers(e, ty, young, old));
        match named {
            Some((decls, loads, last)) => lstr!(
                en: "add {} so that the {young}-cycle value is {} old too: {decls}, write `{loads}` in {block}, and use `{last}` here (cost: {cost})", stage_registers(k), cycle_count(old);
                tr: "{young} çevrimlik değer de {old} çevrim yaşında olsun diye {k} aşama register'ı ekleyin: {decls}; {block} içine `{loads}` yazın ve burada `{last}` kullanın (bedel: {cost})"
            ),
            None => lstr!(
                en: "add {} so that the {young}-cycle value is {} old too: declare each as `reg … : Delayed<T, N> = 0`, load the first from the value and each next one from the one before in {block}, and use the last one here (cost: {cost})", stage_registers(k), cycle_count(old);
                tr: "{young} çevrimlik değer de {old} çevrim yaşında olsun diye {k} aşama register'ı ekleyin: her birini `reg … : Delayed<T, N> = 0` diye bildirin; {block} içinde ilkini değerden, sonrakileri bir öncekinden yükleyin ve burada sonuncusunu kullanın (bedel: {cost})"
            ),
        }
    }

    /// Register kuralının yolu (ADR-0127): `n` çevrimlik kaynak, `want`
    /// çevrim bildirilmiş register. Kaynak gençse önce aşama register'ı,
    /// sonra bildirimin düzeltilmesi ve en son `delay<K>`'nın iddia
    /// olduğu; kaynak yaşlıysa bildirim ya da daha erken aşama.
    fn register_write_help(&self, src: Idx<Expr>, n: u32, want: u32) -> String {
        let holds = n + 1;
        if holds < want {
            let source_age = want - 1;
            let k = source_age - n;
            lstr!(
                en: "write the register from a value {} old: {}; or, if it really holds a {holds}-cycle value, declare it as `Delayed<T, {holds}>`; {}", cycle_count(source_age), self.stage_register_help(Some(src), n, source_age), delay_claim_note(k, source_age);
                tr: "register'ı {source_age} çevrim yaşında bir değerden yazın: {}; ya da gerçekten {holds} çevrimlik bir değer tutuyorsa `Delayed<T, {holds}>` diye bildirin; {}", self.stage_register_help(Some(src), n, source_age), delay_claim_note(k, source_age)
            )
        } else if want == 0 {
            lstr!(
                en: "a register is at least 1 cycle old: declare it as `Delayed<T, {holds}>`";
                tr: "register en az 1 çevrim yaşındadır: `Delayed<T, {holds}>` diye bildirin"
            )
        } else {
            lstr!(
                en: "declare the register as `Delayed<T, {holds}>`, or write it from the stage {} earlier, where the value is {} old", cycle_count(holds - want), cycle_count(want.saturating_sub(1));
                tr: "register'ı `Delayed<T, {holds}>` diye bildirin ya da değeri {} çevrim yaşında olduğu {} çevrim önceki aşamadan yazın", want.saturating_sub(1), holds - want
            )
        }
    }

    /// Adlı aşama register'ları: (`reg` bildirimleri, `on` satırı, son ad).
    fn named_stage_registers(
        &self,
        e: Idx<Expr>,
        ty: Option<&Ty>,
        young: u32,
        old: u32,
    ) -> Option<(String, String, String)> {
        if !matches!(self.ast.exprs[e].kind, ExprKind::Path(_)) {
            return None;
        }
        let def = *self.res.resolutions.get(&e)?;
        let name = &self.res.defs[def.0 as usize].name;
        let (ty_text, init) = match ty? {
            Ty::Bool => ("bool".to_string(), "false"),
            Ty::UInt { width } => (format!("u{width}"), "0"),
            Ty::SInt { width } => (format!("i{width}"), "0"),
            Ty::Bits { width } => (format!("bits<{width}>"), "0"),
            Ty::Trit
            | Ty::Clock
            | Ty::Reset { .. }
            | Ty::Array { .. }
            | Ty::Tuple(_)
            | Ty::Struct(_)
            | Ty::Enum(_)
            | Ty::Instance(_)
            | Ty::Builtin { .. }
            | Ty::Delayed { .. }
            | Ty::IntLit
            | Ty::UIntFlex { .. }
            | Ty::SIntFlex { .. }
            | Ty::Error => return None,
        };
        let names: Vec<String> = (young + 1..=old).map(|n| format!("{name}_d{n}")).collect();
        if names
            .iter()
            .any(|n| self.res.defs.iter().any(|d| &d.name == n))
        {
            return None;
        }
        let decl = |i: usize| {
            format!(
                "`reg {} : Delayed<{ty_text}, {}> = {init}`",
                names[i],
                young + 1 + i as u32
            )
        };
        let decls = if names.len() <= 3 {
            (0..names.len()).map(decl).collect::<Vec<_>>().join(", ")
        } else {
            format!("{}, …, {}", decl(0), decl(names.len() - 1))
        };
        let mut loads = vec![format!("{} <= {name}", names[0])];
        for i in 1..names.len() {
            loads.push(format!("{} <= {}", names[i], names[i - 1]));
        }
        let loads = if loads.len() <= 3 {
            loads.join("  ")
        } else {
            format!("{}  …  {}", loads[0], loads[loads.len() - 1])
        };
        Some((decls, loads, names[names.len() - 1].clone()))
    }

    fn is_path_to(&self, e: Idx<Expr>, def: DefId) -> bool {
        matches!(&self.ast.exprs[e].kind, ExprKind::Path(_))
            && self.res.resolutions.get(&e) == Some(&def)
    }
}

/// "N cycle(s) old" etiketi: hangi değer kaç çevrim yaşında (ADR-0125 §1).
fn cycles_old(n: u32) -> String {
    lstr!(en: "this value is {} old", cycle_count(n); tr: "bu değer {n} çevrim yaşında")
}

/// "1 cycle" / "N cycles".
fn cycle_count(n: u32) -> String {
    if n == 1 {
        "1 cycle".to_string()
    } else {
        format!("{n} cycles")
    }
}

/// "a stage register" / "N stage registers".
fn stage_registers(k: u32) -> String {
    if k == 1 {
        "a stage register".to_string()
    } else {
        format!("{k} stage registers")
    }
}

/// Aşama register'larının bedeli: "K register(s) of W bits".
fn register_cost(k: u32, bits: Option<u16>) -> String {
    let regs = if k == 1 {
        "1 register".to_string()
    } else {
        format!("{k} registers")
    };
    match bits {
        Some(w) => {
            let unit = if w == 1 { "bit" } else { "bits" };
            if k == 1 {
                lstr!(en: "{regs} of {w} {unit}"; tr: "1 register, {w} bit")
            } else {
                lstr!(en: "{regs} of {w} {unit}"; tr: "{k} register, her biri {w} bit")
            }
        }
        None => lstr!(en: "{regs} of the value's width"; tr: "{k} register, değerin genişliğinde"),
    }
}

/// `delay<K>` bir register değildir (ADR-0127, #232): önerinin sonunda,
/// kaçış gibi, bedeliyle anılır (ADR-0125 §3).
fn delay_claim_note(k: u32, old: u32) -> String {
    lstr!(
        en: "`delay<{k}>(…)` is not such a register: it is a claim that adds no hardware and that Volt does not check, right only if the value really is {} old already (it does not change during {})", cycle_count(old), if k == 1 { "that cycle".to_string() } else { format!("those {k} cycles") };
        tr: "`delay<{k}>(…)` böyle bir register değildir: donanım eklemeyen ve Volt'un denetlemediği bir iddiadır; yalnız değer gerçekten zaten {old} çevrim yaşındaysa (o {k} çevrim boyunca değişmiyorsa) doğrudur"
    )
}

/// `let`, `wire` ya da çıkış portunun bildirimi değerin gecikmesiyle
/// uyuşmuyor (ADR-0126). Makine önerisi yok: sayıyı değiştirmek, bildirimin
/// gizlediği karışımı okunduğu yerde geri getirebilir (gidiş-dönüş testi
/// geçmez). `delay<K>` önerilmez: bugün register eklemez (#232).
fn declared_mismatch(decl: Span, value: Span, want: u32, got: u32) -> Diagnostic {
    let cycles = |n: u32| {
        if n == 1 {
            "1 cycle".to_string()
        } else {
            format!("{n} cycles")
        }
    };
    let (dw, dg) = (cycles(want), cycles(got));
    let help = if want > got {
        let k = want - got;
        if k == 1 {
            lstr!(en: "write the count as {got}; if the value must be {dw} old, pass it through one more register stage (a register written in an 'on' block adds one cycle)"; tr: "sayıyı {got} yazın; değerin {want} çevrim yaşında olması gerekiyorsa onu bir register aşamasından daha geçirin (on bloğunda yazılan bir register bir çevrim ekler)")
        } else {
            lstr!(en: "write the count as {got}; if the value must be {dw} old, pass it through {k} more register stages (each register written in an 'on' block adds one cycle)"; tr: "sayıyı {got} yazın; değerin {want} çevrim yaşında olması gerekiyorsa onu {k} register aşamasından daha geçirin (on bloğunda yazılan her register bir çevrim ekler)")
        }
    } else {
        let k = got - want;
        if k == 1 {
            lstr!(en: "write the count as {got}; if the value must be {dw} old, take it from the stage one cycle earlier"; tr: "sayıyı {got} yazın; değerin {want} çevrim yaşında olması gerekiyorsa onu bir çevrim önceki aşamadan alın")
        } else {
            lstr!(en: "write the count as {got}; if the value must be {dw} old, take it from the stage {k} cycles earlier"; tr: "sayıyı {got} yazın; değerin {want} çevrim yaşında olması gerekiyorsa onu {k} çevrim önceki aşamadan alın")
        }
    };
    Diagnostic::error(
        ErrorCode::E5010,
        lstr!(en: "declared delay does not match the value"; tr: "bildirilen gecikme değerle uyuşmuyor"),
        LabeledSpan::primary(
            decl,
            lstr!(en: "declared as {dw} here"; tr: "burada {want} çevrim olarak bildirildi"),
        ),
        help,
    )
    .with_secondary(
        value,
        lstr!(en: "this value is {dg} old"; tr: "bu değer {got} çevrim yaşında"),
    )
    .with_note(
        NoteKind::Reason,
        lstr!(en: "every reader takes the declared count as the value's age, so a wrong count hides values of different cycles being combined where the value is read"; tr: "değeri okuyan her yer bildirilen sayıyı onun yaşı sayar; yanlış bir sayı, okunduğu yerde farklı çevrimlerin değerlerinin birleşmesini gizler"),
    )
}
