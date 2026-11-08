//! Modül seviyesi `for` açılımı (ADR-0056 — düzenli yapılar).
//!
//! ```volt
//! for i in 0..4 {
//!     let pe = ProcElem { clk: clk, data_in: bus[i] }
//!     results[i] = pe.out
//! }
//! ```
//!
//! Pipeline desugar'ı (ADR-0038) ve monomorfizasyon (ADR-0041) gibi
//! parser katmanında çalışır: gövde her yineleme için modül gövdesine
//! KLONLANIR, döngü değişkeni literale ikame edilir, gövdede bildirilen
//! isimler yineleme soneki alır (`pe` → `pe_0`; iç içe `pe_0_1`) ve
//! `let ad = Modül { ... }` yapı literali örneklemeye dönüşür (stmt.rs
//! [N3] kuralı). İsim çözümleme, tip denetimi, alan çıkarımı, sürücü
//! analizi ve SV üretimi `for`u HİÇ görmez; elle yazılmış N örnekle
//! aynı yoldan geçer. SV çıktısı düz açılımdır (`generate` bloğu yok —
//! araç bağımsız, ADR-0041 ilkesi).
//!
//! Her yineleme benzersiz `Span.ctx` taşır (resolve'un span anahtarlı
//! tabloları çakışmaz); ctx → (değişken, değer) kaydı
//! `SourceFile::generate` yan tablosuna yazılır, tanılar "for i = 2
//! yinelemesinde" notunu buradan üretir.
//!
//! Monomorfizasyon turunun içinde koşar: klonlanan generic gövdede
//! `0..N` sınırı ikame sonrası literaldir; açılan gövdedeki generic
//! örneklemeler (`Pe<8> { }`) aynı turda istek olarak toplanır.
//!
//! Adlar kapsam zincirinde çözülür ([`Scopes`], #130): döngü değişkeni
//! döngünün kapsamında, gövdenin `let`'i gövde bloğunda bildirilir; iç
//! kapsamda yeniden bildirilen ad ikame edilmez. Çözücü (HIR) döngü
//! değişkenini ve yeniden adlanan `let`'i görmez, bu yüzden onların
//! gölgelemesinde W1002'yi ve döngü değişkeni deseninde E1015'i açılım
//! verir (name-resolution.md §5.1, §6).

use std::collections::HashMap;

use volt_ast::visibility::RootFilter;
use volt_ast::{
    AssignStmt, BlockStmt, ExprKind, ForStmt, GenerateIter, Idx, InstanceDecl, Item, ItemKind,
    LetDecl, PortBinding, SourceFile, Stmt, StmtKind,
};
use volt_diagnostics::{
    fold_duplicates, lstr, Applicability, Diagnostic, ErrorCode, LabeledSpan, Suggestion,
};
use volt_span::{FileId, Span};

use super::budget::{ExpansionBudget, MAX_EXPANSION_NODES};
use super::clone::{BlockRole, Cloner};
use super::scope::{
    invisible_name, invisible_name_error, shadowing_warning, Binding, Entry, Scopes,
};

/// Açılabilecek en fazla yineleme (const-eval.md §8, E2027).
pub(crate) const MAX_UNROLL: i128 = 4096;

/// Const ifade değerlendirmesinde derinlik sınırı (döngüsel const).
const MAX_CONST_DEPTH: u32 = 64;

pub(super) struct Unroller<'a> {
    ast: &'a mut SourceFile,
    next_ctx: &'a mut u16,
    /// Açılım düğüm bütçesi (ADR-0068 §4, §6): gövde büyüklüğü × yineleme
    /// çarpımı. `MAX_UNROLL` yineleme sayısını, u16 ctx toplam yinelemeyi
    /// sınırlar; çarpımı yalnız bu sınırlar. Aşılınca kalan döngüler
    /// açılmaz (tek E2027, kaskad yok).
    budget: &'a mut ExpansionBudget,
    diagnostics: &'a mut Vec<Diagnostic>,
    /// Üst düzey `const AD = ...` değerleri (sınır ifadeleri için).
    consts: Consts,
}

/// Bir modülün gövdesindeki tüm modül seviyesi `for`ları açar.
pub(super) fn unroll_module(
    ast: &mut SourceFile,
    item: Idx<Item>,
    next_ctx: &mut u16,
    budget: &mut ExpansionBudget,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let has_for = match &ast.items_arena[item].kind {
        ItemKind::Module(m) => m
            .body
            .iter()
            .any(|&s| matches!(ast.stmts[s].kind, StmtKind::For(_))),
        _ => false,
    };
    if !has_for {
        return;
    }
    let consts = collect_consts(ast);
    let mut chain = Scopes::module(ast, item);
    let ItemKind::Module(m) = &mut ast.items_arena[item].kind else {
        return;
    };
    let body = std::mem::take(&mut m.body);
    let mut u = Unroller {
        ast,
        next_ctx,
        budget,
        diagnostics,
        consts,
    };
    let mut out = Vec::with_capacity(body.len());
    for stmt in body {
        if !matches!(u.ast.stmts[stmt].kind, StmtKind::For(_)) {
            // Modül içi sıralı (§4): sonraki `for` bu bildirimi görür.
            declare_outer(&mut chain, decl_name(&u.ast.stmts[stmt].kind));
            out.push(stmt);
            continue;
        }
        let span = u.ast.stmts[stmt].span;
        let StmtKind::For(f) = std::mem::replace(&mut u.ast.stmts[stmt].kind, StmtKind::Error)
        else {
            unreachable!()
        };
        let lifted = u.expand_for(f, span, 0, "", &chain);
        for &s in &lifted {
            declare_outer(&mut chain, decl_name(&u.ast.stmts[s].kind));
        }
        out.extend(lifted);
    }
    if let ItemKind::Module(m) = &mut u.ast.items_arena[item].kind {
        m.body = out;
    }
}

/// Çözücünün de gördüğü bir bildirim (öğe, port, modül sinyali).
fn declare_outer(chain: &mut Scopes, name: Option<&volt_ast::Name>) {
    if let Some(name) = name {
        chain.declare_outer(name);
    }
}

/// Modül gövdesinde ad bildiren deyimin adı.
pub(crate) fn decl_name(kind: &StmtKind) -> Option<&volt_ast::Name> {
    match kind {
        StmtKind::Reg(r) => Some(&r.name),
        StmtKind::Let(l) => Some(&l.name),
        StmtKind::Wire(w) => Some(&w.name),
        StmtKind::Instance(i) => Some(&i.name),
        _ => None,
    }
}

/// Birimin üst düzey const öğeleri: kök ad → (tanım dosyası, değer
/// ifadesi). Aynı ad iki dosyada E1003'tür (ADR-0042 §4, çözücü verir);
/// tablo ikisini de tutar, arama görüneni seçer ([`visible_const`]).
pub(crate) type Consts = HashMap<String, Vec<(FileId, Idx<volt_ast::Expr>)>>;

/// Birimdeki üst düzey const öğeleri ([`Consts`]).
pub(crate) fn collect_consts(ast: &SourceFile) -> Consts {
    let mut consts = Consts::new();
    for &i in &ast.items {
        let item = &ast.items_arena[i];
        if let ItemKind::Const(c) = &item.kind {
            consts
                .entry(c.name.text.clone())
                .or_default()
                .push((item.span.file, c.value));
        }
    }
    consts
}

/// `at` ifadesindeki yalın `name`in bağlandığı kök const'un değer
/// ifadesi. Ad yazıldığı dosyanın kapsamında çözülür (#139): kendi
/// dosyasının const'u ya da o dosyaya `use` ile getirilen (takma adla da)
/// const; başka dosyanın getirilmemiş const'u görünmez (name-resolution.md
/// §3.3, E1001). Kural çözücününkidir (`RootFilter`). Bir const'un değeri
/// kendi dosyasında yazıldığından iç içe değerlendirme o dosyada sürer.
pub(crate) fn visible_const(
    ast: &SourceFile,
    consts: &Consts,
    at: Idx<volt_ast::Expr>,
    name: &str,
) -> Option<Idx<volt_ast::Expr>> {
    let file = ast.exprs[at].span.file;
    let filter = RootFilter::new(ast.file_scopes.as_ref());
    filter.pick(file, name, consts.get(filter.root_name(file, name))?)
}

impl Unroller<'_> {
    /// Bir `for`u açar; üretilen modül seviyesi deyimler sırayla döner.
    /// `parent_ctx` iç içe döngüde dış yinelemenin bağlamı (0: en dış
    /// döngü), `suffix` dış yinelemelerin soneki, `chain` `for`un
    /// bulunduğu modülün kapsam zinciri (kök + modül kapsamı). İç içe
    /// döngünün gövdesi dış yinelemede klonlandı: dış adlar orada çözüldü,
    /// gölgeleme tanıları da orada verildi.
    fn expand_for(
        &mut self,
        f: ForStmt,
        span: Span,
        parent_ctx: u16,
        suffix: &str,
        chain: &Scopes,
    ) -> Vec<Idx<Stmt>> {
        // Bütçe başka bir açılımda (mono klonu dahil) dolmuş olabilir:
        // döngü açılmadan atlanıyorsa tanı mutlaka verilmiş olmalı.
        if self.over_budget(&f.var.text, span) {
            return Vec::new();
        }
        // En dış döngünün değişkeni: dış tanımı gölgeliyorsa W1002 (§6);
        // çözücü döngüyü görmez. İç içe döngününkini dış yineleme verdi.
        let outermost = parent_ctx == 0;
        if outermost {
            let mut scope = chain.clone();
            scope.push();
            let entry = Entry {
                binding: Binding::LoopVar(0),
                span: f.var.span,
                resolver_sees: false,
            };
            if let Some(outer) = scope.declare(&f.var.text, entry) {
                self.diagnostics
                    .push(shadowing_warning(&f.var.text, f.var.span, outer.span));
            }
        }
        let Some((start, end)) = self.bounds(&f, span, chain) else {
            return Vec::new();
        };
        let declared = declared_names(self.ast, f.body);
        // Kök: dış yineleme; en dışta `for` deyiminin kendi ctx'i (monomorf
        // klonda klonun ctx'i, elle yazılmış modülde 0) — ADR-0068 notu.
        let parent = if parent_ctx != 0 {
            parent_ctx
        } else {
            span.ctx
        };
        // Katlama işareti: bu döngünün ürettiği tanılar buradan başlar.
        let mark = self.diagnostics.len();
        let mut out = Vec::new();
        for v in start..end {
            let Some(ctx) = self.alloc_ctx(span) else {
                return out;
            };
            self.budget.charge(1);
            self.budget.charge_text(f.var.text.len());
            self.ast.generate.iterations.insert(
                ctx,
                GenerateIter {
                    var: f.var.text.clone(),
                    value: v,
                    for_span: span,
                    parent,
                },
            );
            let suffix_v = format!("{suffix}_{v}");
            let rename_v: HashMap<String, String> = declared
                .iter()
                .map(|name| (name.clone(), format!("{name}{suffix_v}")))
                .collect();
            let mut cloner = Cloner::new(self.ast, self.budget, ctx).for_iteration(
                chain.clone(),
                &f.var,
                v,
                rename_v,
            );
            // Gölgeleme ve desen tanıları kaynak başına bir kez: en dış
            // döngünün ilk yinelemesi (iç içe döngüler de onun klonunda).
            if outermost && v == start {
                cloner = cloner.with_lint(span.ctx);
            }
            let block = cloner.clone_block(f.body, BlockRole::Iteration);
            let recovered = cloner.saw_recovery();
            let lints = cloner.take_diagnostics();
            self.diagnostics.extend(lints);
            let stmts = std::mem::take(&mut self.ast.blocks[block].stmts);
            for bs in stmts {
                self.lift(bs, ctx, &suffix_v, chain, &mut out);
            }
            // Her yinelemeden sonra: bu döngünün özdeş tanıları katlanır
            // (ADR-0068). Sonradan katlamak yetmez — 283² kopya önce
            // belleğe yığılırdı; bellek O(farklı tanı) kalır.
            fold_duplicates(self.diagnostics, mark);
            // Düğüm bütçesi (ADR-0068 §4, §6): gövde × yineleme çarpımı da
            // AÇILIM SIRASINDA denetlenir (E4010 ilkesi, ADR-0067 §2).
            if self.over_budget(&f.var.text, span) {
                return out;
            }
            // Hata kurtarma düğümü taşıyan gövde bir kez açılır (ADR-0068
            // §6): tanısı zaten verildi, kopyaları katlanırdı; parse hatası
            // anlamsal aşamaları durdurduğundan ek kopyanın bilgi değeri
            // yok. Tek kopya modülde kalır (LSP çözümlemesi için).
            if recovered {
                return out;
            }
        }
        out
    }

    /// Bütçe dolu mu? İlk kez görülüyorsa (birimde tek) E2027 basar;
    /// sonraki açılımlar sessizce durur — tanı zaten verildi.
    fn over_budget(&mut self, var: &str, span: Span) -> bool {
        if !self.budget.exhausted() {
            return false;
        }
        if self.budget.take_report() {
            self.err_node_budget(var, span);
        }
        true
    }

    /// Düğüm bütçesi aşımı: tek E2027, sonraki döngüler açılmaz.
    fn err_node_budget(&mut self, var: &str, span: Span) {
        self.diagnostics.push(Diagnostic::error(
            ErrorCode::E2027,
            lstr!(
                en: "unrolling 'for {var}' exceeded the AST node budget ({MAX_EXPANSION_NODES} nodes per compilation unit)";
                tr: "'for {var}' açılımı AST düğüm bütçesini aştı (derleme birimi başına {MAX_EXPANSION_NODES} düğüm)"
            ),
            LabeledSpan::primary(
                span,
                lstr!(en: "the unrolled body is too large"; tr: "açılan gövde çok büyük"),
            ),
            lstr!(
                en: "narrow the range or move the loop body into a submodule instantiated per iteration";
                tr: "aralığı daraltın ya da döngü gövdesini yineleme başına örneklenen bir alt modüle taşıyın"
            ),
        ));
    }

    /// Klonlanmış gövde deyimini modül seviyesi deyime kaldırır.
    fn lift(
        &mut self,
        bs: BlockStmt,
        ctx: u16,
        suffix: &str,
        chain: &Scopes,
        out: &mut Vec<Idx<Stmt>>,
    ) {
        match bs {
            BlockStmt::BlockAssign { lhs, rhs, span } => {
                out.push(self.alloc_stmt(span, StmtKind::Assign(AssignStmt { lhs, rhs })));
            }
            BlockStmt::Let(l) => {
                let span = self.let_span(&l, ctx);
                let kind = self.reclassify_let(l);
                out.push(self.alloc_stmt(span, kind));
            }
            BlockStmt::For(inner) => {
                let span = self.ast.blocks[inner.body].span;
                out.extend(self.expand_for(inner, span, ctx, suffix, chain));
            }
            BlockStmt::NonBlockAssign { span, .. } => self.err_unsupported(
                span,
                &lstr!(en: "non-blocking assignment ('<=')"; tr: "ardışık atama ('<=')"),
                &lstr!(
                    en: "module-level 'for' bodies are combinational: use '=' or move the assignment into an 'on' block";
                    tr: "modül seviyesi 'for' gövdesi kombinasyoneldir: '=' kullanın ya da atamayı bir 'on' bloğuna taşıyın"
                ),
            ),
            BlockStmt::If(i) => self.err_unsupported(
                i.span,
                &lstr!(en: "'if'"; tr: "'if'"),
                &self.comb_help(),
            ),
            BlockStmt::Match(m) => self.err_unsupported(
                m.span,
                &lstr!(en: "'match'"; tr: "'match'"),
                &self.comb_help(),
            ),
            BlockStmt::Error => {}
        }
    }

    fn comb_help(&self) -> String {
        lstr!(
            en: "move the conditional into a 'comb' block, or write it as an 'if' expression";
            tr: "koşulu bir 'comb' bloğuna taşıyın ya da 'if' ifadesi olarak yazın"
        )
    }

    /// `let u = Mod { ... }` (tip anotasyonsuz yapı literali) →
    /// örnekleme — stmt.rs [N3] ile aynı kural.
    fn reclassify_let(&mut self, decl: LetDecl) -> StmtKind {
        if decl.ty.is_none()
            && matches!(self.ast.exprs[decl.value].kind, ExprKind::StructLit { .. })
        {
            let kind = std::mem::replace(&mut self.ast.exprs[decl.value].kind, ExprKind::Error);
            let ExprKind::StructLit { path, fields } = kind else {
                unreachable!()
            };
            let bindings = fields
                .into_iter()
                .map(|f| PortBinding {
                    span: f.span,
                    port_name: f.name,
                    value: f.value,
                })
                .collect();
            // `let w = W<8> { }` — generic argümanlar yan tablodan (ADR-0056).
            let generic_args = self
                .ast
                .generate
                .block_generic_args
                .remove(&decl.value)
                .unwrap_or_default();
            return StmtKind::Instance(InstanceDecl {
                name: decl.name,
                module_path: path,
                generic_args,
                bindings,
            });
        }
        StmtKind::Let(decl)
    }

    /// `let` deyiminin span'i: ad başından değer sonuna.
    fn let_span(&self, l: &LetDecl, ctx: u16) -> Span {
        let name = l.name.span;
        let value = self.ast.exprs[l.value].span;
        Span::new(
            name.file,
            name.start.min(value.start),
            value.end.max(name.end),
        )
        .with_ctx(ctx)
    }

    fn alloc_stmt(&mut self, span: Span, kind: StmtKind) -> Idx<Stmt> {
        self.budget.charge(1);
        self.ast.stmts.alloc(Stmt {
            span,
            attrs: Vec::new(),
            kind,
        })
    }

    /// Yeni yineleme bağlamı; u16 bütçesi biterse E2027.
    fn alloc_ctx(&mut self, span: Span) -> Option<u16> {
        if *self.next_ctx == u16::MAX {
            self.diagnostics.push(
                Diagnostic::error(
                    ErrorCode::E2027,
                    lstr!(
                        en: "loop unrolling exhausted the span context budget ({} iterations per compilation unit)", u16::MAX;
                        tr: "döngü açılımı span bağlam bütçesini tüketti (derleme birimi başına {} yineleme)", u16::MAX
                    ),
                    LabeledSpan::primary(
                        span,
                        lstr!(en: "this loop cannot be unrolled"; tr: "bu döngü açılamıyor"),
                    ),
                    lstr!(
                        en: "split the design into smaller modules or narrow the ranges";
                        tr: "tasarımı daha küçük modüllere bölün ya da aralıkları daraltın"
                    ),
                ),
            );
            return None;
        }
        let ctx = *self.next_ctx;
        *self.next_ctx += 1;
        Some(ctx)
    }

    /// `[start, end)` — sabit değilse E2021, ters aralık E2028, sınır
    /// üstü E2027. Sınır `for`un bulunduğu kapsamda çözülür: kök const'u
    /// gölgeleyen port ya da modül sinyali sabit değildir (#130).
    fn bounds(&mut self, f: &ForStmt, span: Span, chain: &Scopes) -> Option<(i128, i128)> {
        let var = f.var.text.clone();
        // Hata kurtarmanın bıraktığı sınır (ör. E0018 ile kesilen ifade,
        // ADR-0080) zaten raporlandı: E2021 kaskadı değil.
        let recovered = |e: Idx<volt_ast::Expr>| matches!(self.ast.exprs[e].kind, ExprKind::Error);
        if recovered(f.start) || recovered(f.end) {
            return None;
        }
        let (Some(s), Some(e)) = (self.eval(f.start, chain), self.eval(f.end, chain)) else {
            // Başka dosyanın görünmeyen öğesi sabit değil, tanımsızdır (#139).
            let invisible = invisible_name(self.ast, chain, f.start)
                .or_else(|| invisible_name(self.ast, chain, f.end));
            if let Some(name) = invisible {
                self.diagnostics.push(invisible_name_error(self.ast, name));
                return None;
            }
            self.diagnostics.push(Diagnostic::error(
                ErrorCode::E2021,
                lstr!(
                    en: "the bounds of 'for {var}' must be compile-time constants";
                    tr: "'for {var}' sınırları derleme zamanı sabiti olmalı"
                ),
                LabeledSpan::primary(
                    span,
                    lstr!(en: "bounds are not constant"; tr: "sınırlar sabit değil"),
                ),
                lstr!(
                    en: "use literals, const items or generic parameters: for i in 0..N";
                    tr: "literal, const ya da generic parametre kullanın: for i in 0..N"
                ),
            ));
            return None;
        };
        if e < s {
            self.diagnostics.push(
                Diagnostic::error(
                    ErrorCode::E2028,
                    lstr!(
                        en: "'for {var}' range is reversed: {s}..{e}";
                        tr: "'for {var}' aralığı ters: {s}..{e}"
                    ),
                    LabeledSpan::primary(
                        span,
                        lstr!(en: "end is smaller than start"; tr: "bitiş başlangıçtan küçük"),
                    ),
                    lstr!(
                        en: "write the smaller bound first: for {var} in {e}..{s}";
                        tr: "küçük sınırı önce yazın: for {var} in {e}..{s}"
                    ),
                )
                // suggestion: e2028_module_for
                .with_suggestion(
                    Suggestion::replace(
                        self.ast.exprs[f.start].span,
                        e.to_string(),
                        Applicability::MaybeIncorrect,
                    )
                    .and_replace(self.ast.exprs[f.end].span, s.to_string()),
                ),
            );
            return None;
        }
        if e - s > MAX_UNROLL {
            self.diagnostics.push(Diagnostic::error(
                ErrorCode::E2027,
                lstr!(
                    en: "'for {var}' unrolls {} iterations, the limit is {MAX_UNROLL}", e - s;
                    tr: "'for {var}' {} yinelemeye açılıyor, sınır {MAX_UNROLL}", e - s
                ),
                LabeledSpan::primary(
                    span,
                    lstr!(en: "too many iterations"; tr: "çok fazla yineleme"),
                ),
                lstr!(en: "narrow the range"; tr: "aralığı daraltın"),
            ));
            return None;
        }
        Some((s, e))
    }

    fn eval(&self, e: Idx<volt_ast::Expr>, chain: &Scopes) -> Option<i128> {
        let root_const = |name: &str| !chain.bound_above_root(name);
        eval_const_scoped(self.ast, &self.consts, e, 0, &root_const)
    }

    fn err_unsupported(&mut self, span: Span, what: &str, help: &str) {
        let d = Diagnostic::error(
            ErrorCode::E0003,
            lstr!(
                en: "{what} inside a module-level 'for' is not supported";
                tr: "modül seviyesi 'for' içinde {what} desteklenmiyor"
            ),
            LabeledSpan::primary(
                span,
                lstr!(en: "not allowed here"; tr: "burada kullanılamaz"),
            ),
            help,
        );
        self.diagnostics.push(d);
    }
}

/// Gövdenin doğrudan çocuklarında bildirilen isimler (`let`); iç içe
/// `for` gövdesi kendi turunda ele alınır.
fn declared_names(ast: &SourceFile, body: Idx<volt_ast::Block>) -> Vec<String> {
    ast.blocks[body]
        .stmts
        .iter()
        .filter_map(|s| match s {
            BlockStmt::Let(l) => Some(l.name.text.clone()),
            _ => None,
        })
        .collect()
}

/// Parser katmanı sabit değerlendirme: literal, üst düzey const,
/// aritmetik. Sinyal ya da bilinmeyen isim → None (tanı çağıranda).
/// HIR'ın `ConstEvaluator`ından dar (fonksiyon çağrısı, cast, dizi
/// yok): parser katmanı desugar'larının (for sınırı, bundle dizisi
/// uzunluğu/indeksi) ihtiyacı budur.
pub(crate) fn eval_const(
    ast: &SourceFile,
    consts: &Consts,
    e: Idx<volt_ast::Expr>,
    depth: u32,
) -> Option<i128> {
    eval_const_scoped(ast, consts, e, depth, &|_| true)
}

/// [`eval_const`]; `root_const(ad)` yanlışsa ad kök const'a bağlanmaz
/// (kullanıldığı yerde bir iç tanım onu gölgeler). Const'un kendi değeri
/// kök kapsamda çözülür (name-resolution.md §4).
pub(crate) fn eval_const_scoped(
    ast: &SourceFile,
    consts: &Consts,
    e: Idx<volt_ast::Expr>,
    depth: u32,
    root_const: &dyn Fn(&str) -> bool,
) -> Option<i128> {
    use volt_ast::{BinOp, UnOp};
    if depth > MAX_CONST_DEPTH {
        return None;
    }
    match &ast.exprs[e].kind {
        ExprKind::IntLit { value, .. } => i128::try_from(*value).ok(),
        ExprKind::Unary {
            op: UnOp::Neg,
            operand,
        } => eval_const_scoped(ast, consts, *operand, depth + 1, root_const)?.checked_neg(),
        ExprKind::Binary { op, lhs, rhs } => {
            let l = eval_const_scoped(ast, consts, *lhs, depth + 1, root_const)?;
            let r = eval_const_scoped(ast, consts, *rhs, depth + 1, root_const)?;
            match op {
                BinOp::Add => l.checked_add(r),
                BinOp::Sub => l.checked_sub(r),
                BinOp::Mul => l.checked_mul(r),
                BinOp::Div => l.checked_div(r),
                BinOp::Rem => l.checked_rem(r),
                BinOp::Shl => u32::try_from(r).ok().and_then(|r| l.checked_shl(r)),
                BinOp::Shr => u32::try_from(r).ok().and_then(|r| l.checked_shr(r)),
                _ => None,
            }
        }
        ExprKind::Path(p) if p.segments.len() == 1 && root_const(&p.segments[0].text) => {
            let value = visible_const(ast, consts, e, &p.segments[0].text)?;
            eval_const(ast, consts, value, depth + 1)
        }
        _ => None,
    }
}
