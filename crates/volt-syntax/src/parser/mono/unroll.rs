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

use std::collections::HashMap;

use volt_ast::{
    AssignStmt, BlockStmt, ExprKind, ForStmt, GenerateIter, Idx, InstanceDecl, Item, ItemKind,
    LetDecl, PortBinding, SourceFile, Stmt, StmtKind,
};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan};
use volt_span::Span;

use super::clone::Cloner;

/// Açılabilecek en fazla yineleme (const-eval.md §8, E2027).
pub(crate) const MAX_UNROLL: i128 = 4096;

/// Const ifade değerlendirmesinde derinlik sınırı (döngüsel const).
const MAX_CONST_DEPTH: u32 = 64;

pub(super) struct Unroller<'a> {
    ast: &'a mut SourceFile,
    next_ctx: &'a mut u16,
    diagnostics: &'a mut Vec<Diagnostic>,
    /// Üst düzey `const AD = ...` değerleri (sınır ifadeleri için).
    consts: HashMap<String, Idx<volt_ast::Expr>>,
}

/// Bir modülün gövdesindeki tüm modül seviyesi `for`ları açar.
pub(super) fn unroll_module(
    ast: &mut SourceFile,
    item: Idx<Item>,
    next_ctx: &mut u16,
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
    let ItemKind::Module(m) = &mut ast.items_arena[item].kind else {
        return;
    };
    let body = std::mem::take(&mut m.body);
    let mut u = Unroller {
        ast,
        next_ctx,
        diagnostics,
        consts,
    };
    let mut out = Vec::with_capacity(body.len());
    for stmt in body {
        if !matches!(u.ast.stmts[stmt].kind, StmtKind::For(_)) {
            out.push(stmt);
            continue;
        }
        let span = u.ast.stmts[stmt].span;
        let StmtKind::For(f) = std::mem::replace(&mut u.ast.stmts[stmt].kind, StmtKind::Error)
        else {
            unreachable!()
        };
        out.extend(u.expand_for(f, span, 0, &HashMap::new(), ""));
    }
    if let ItemKind::Module(m) = &mut u.ast.items_arena[item].kind {
        m.body = out;
    }
}

/// Dosyadaki üst düzey const öğeleri (isim → değer ifadesi).
pub(crate) fn collect_consts(ast: &SourceFile) -> HashMap<String, Idx<volt_ast::Expr>> {
    ast.items
        .iter()
        .filter_map(|&i| match &ast.items_arena[i].kind {
            ItemKind::Const(c) => Some((c.name.text.clone(), c.value)),
            _ => None,
        })
        .collect()
}

impl Unroller<'_> {
    /// Bir `for`u açar; üretilen modül seviyesi deyimler sırayla döner.
    /// `parent_ctx` iç içe döngüde dış yinelemenin bağlamı, `rename`
    /// dış gövdenin isim haritası, `suffix` dış yinelemelerin soneki.
    fn expand_for(
        &mut self,
        f: ForStmt,
        span: Span,
        parent_ctx: u16,
        rename: &HashMap<String, String>,
        suffix: &str,
    ) -> Vec<Idx<Stmt>> {
        let Some((start, end)) = self.bounds(&f, span) else {
            return Vec::new();
        };
        let declared = declared_names(self.ast, f.body);
        let mut out = Vec::new();
        for v in start..end {
            let Some(ctx) = self.alloc_ctx(span) else {
                return out;
            };
            self.ast.generate.iterations.insert(
                ctx,
                GenerateIter {
                    var: f.var.text.clone(),
                    value: v,
                    for_span: span,
                    parent: parent_ctx,
                },
            );
            let suffix_v = format!("{suffix}_{v}");
            let mut rename_v = rename.clone();
            for name in &declared {
                rename_v.insert(name.clone(), format!("{name}{suffix_v}"));
            }
            let subst = HashMap::from([(f.var.text.clone(), v)]);
            let block = Cloner::new(self.ast, subst, ctx)
                .with_rename(rename_v.clone())
                .clone_block(f.body);
            let stmts = std::mem::take(&mut self.ast.blocks[block].stmts);
            for bs in stmts {
                self.lift(bs, ctx, &rename_v, &suffix_v, &mut out);
            }
        }
        out
    }

    /// Klonlanmış gövde deyimini modül seviyesi deyime kaldırır.
    fn lift(
        &mut self,
        bs: BlockStmt,
        ctx: u16,
        rename: &HashMap<String, String>,
        suffix: &str,
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
                out.extend(self.expand_for(inner, span, ctx, rename, suffix));
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

    /// `[start, end)` — sabit değilse E2005, ters aralık E2028, sınır
    /// üstü E2027.
    fn bounds(&mut self, f: &ForStmt, span: Span) -> Option<(i128, i128)> {
        let var = f.var.text.clone();
        let (Some(s), Some(e)) = (self.eval(f.start, 0), self.eval(f.end, 0)) else {
            self.diagnostics.push(Diagnostic::error(
                ErrorCode::E2005,
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
            self.diagnostics.push(Diagnostic::error(
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
            ));
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

    fn eval(&self, e: Idx<volt_ast::Expr>, depth: u32) -> Option<i128> {
        eval_const(self.ast, &self.consts, e, depth)
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
    consts: &HashMap<String, Idx<volt_ast::Expr>>,
    e: Idx<volt_ast::Expr>,
    depth: u32,
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
        } => eval_const(ast, consts, *operand, depth + 1)?.checked_neg(),
        ExprKind::Binary { op, lhs, rhs } => {
            let l = eval_const(ast, consts, *lhs, depth + 1)?;
            let r = eval_const(ast, consts, *rhs, depth + 1)?;
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
        ExprKind::Path(p) if p.segments.len() == 1 => {
            let value = *consts.get(&p.segments[0].text)?;
            eval_const(ast, consts, value, depth + 1)
        }
        _ => None,
    }
}
