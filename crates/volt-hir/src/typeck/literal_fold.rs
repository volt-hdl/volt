//! Literal aritmetiğinin katlanması (#144).
//!
//! Yalnız soneksiz literallerden, blok içi `for` değişkenlerinden ve
//! aritmetik işleçlerden (`+ - * / %`, tekli `-`) oluşan ifade bir literal
//! ifadesidir. Tipi `IntLit`'tir (type-inference.md §3.3: iki literalin
//! aritmetiği "hâlâ literal"), değeri derleme zamanında hesaplanır. Hesap
//! const-eval.md §1'in hassasiyetindedir: tam sayı `i128`'de "geniş
//! tutulur, sığdırma sonra kontrol edilir"; ara değerler kesilmez. §4'ün
//! kuralları: taşma hatadır (E2022), sıfıra bölme ve sıfıra göre kalan
//! E2023'tür (`DivisionByZero`), `/` ve `%` sıfıra doğru keser
//! (`checked_div`, `checked_rem`).
//!
//! Literal ifadesi beklenen somut tipe uyarlanırken (§4 check kipi:
//! atama, tipli `let`, karşılaştırmanın ve aritmetiğin karşı yanı) değeri
//! o tipe sığmalıdır; sığmazsa E2010 (const-eval.md §4 gerekçesi: `const X
//! : u8 = 200 + 100; // ✗ E2010`). Üretici aynı değeri yazar
//! (volt-sv-emit `fold_literal_arith`).
//!
//! Generic parametreler ve modül düzeyi `for` değişkenleri ayrıştırıcıda
//! literale dönüşür (ADR-0041, ADR-0056); burada literaldirler. Blok içi
//! `for` değişkeninin değeri sınırlarından gelir (`loop_bounds`,
//! const-eval.md §8): ifade her yinelemenin değeriyle denetlenir.

use volt_ast::{BinOp, Expr, ExprKind, Idx, UnOp};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use super::TypeChecker;
use crate::resolve::DefId;
use crate::ty::{Ty, TypeId};

/// Değerlendirme hatası.
enum Fault {
    /// Bölen 0 (raporu `check_literal_divisor` verir).
    DivZero,
    /// `i128` taşması (const-eval.md §4 `checked_*`).
    Overflow(Span),
}

/// Blok içi döngü değişkeni ve `[start, end)` sınırı.
type LoopVar = (DefId, u32, u32);

/// Döngü değişkenlerinin bir yinelemedeki değerleri.
type Env = Vec<(DefId, i128)>;

/// Sığmayan ya da taşan ilk yineleme.
enum Finding {
    OutOfRange(i128, Env),
    Overflow(Span, Env),
}

impl TypeChecker<'_, '_> {
    /// `expr` literal ifadesiyse içindeki blok içi döngü değişkenleri.
    /// Sınırı bilinmeyen döngü değişkeni, sonekli literal ya da başka her
    /// biçim `None`.
    fn literal_vars(&self, expr: Idx<Expr>) -> Option<Vec<LoopVar>> {
        let mut vars = Vec::new();
        self.collect_literal_vars(expr, &mut vars).then_some(vars)
    }

    fn collect_literal_vars(&self, expr: Idx<Expr>, vars: &mut Vec<LoopVar>) -> bool {
        let kind = &self.ast.exprs[expr].kind;
        if let ExprKind::IntLit { suffix, .. } = kind {
            return suffix.is_none();
        }
        if let ExprKind::Unary {
            op: UnOp::Neg,
            operand,
        } = kind
        {
            return self.collect_literal_vars(*operand, vars);
        }
        if let ExprKind::Binary { op, lhs, rhs } = kind {
            return is_arith(*op)
                && self.collect_literal_vars(*lhs, vars)
                && self.collect_literal_vars(*rhs, vars);
        }
        let Some((def, start, end)) = self.bounded_loop_var(expr) else {
            return false;
        };
        if !vars.iter().any(|v| v.0 == def) {
            vars.push((def, start, end));
        }
        true
    }

    /// İfade, sınırları bilinen blok içi döngü değişkeniyse kimliği ve
    /// `[start, end)` sınırı.
    fn bounded_loop_var(&self, expr: Idx<Expr>) -> Option<LoopVar> {
        let ExprKind::Path(path) = &self.ast.exprs[expr].kind else {
            return None;
        };
        let [seg] = path.segments.as_slice() else {
            return None;
        };
        let def = *self.res.use_spans.get(&seg.span)?;
        let &(start, end) = self.loop_bounds.get(&def)?;
        Some((def, start, end))
    }

    /// Soneksiz literal ifadesinin değeri, işaret ve büyüklük olarak (#207):
    /// çıplak literal (2^128 - 1'e kadar), `-literal` ve döngü değişkeni
    /// taşımayan literal aritmetiği (i128'de). Başka biçim, döngü
    /// değişkeni ya da taşma `None`.
    pub(super) fn literal_sign_mag(&self, expr: Idx<Expr>) -> Option<(bool, u128)> {
        let lit = |e: Idx<Expr>| {
            if let ExprKind::IntLit {
                value,
                suffix: None,
                ..
            } = self.ast.exprs[e].kind
            {
                Some(value)
            } else {
                None
            }
        };
        if let Some(v) = lit(expr) {
            return Some((false, v));
        }
        if let ExprKind::Unary {
            op: UnOp::Neg,
            operand,
        } = self.ast.exprs[expr].kind
        {
            if let Some(v) = lit(operand) {
                return Some((v != 0, v));
            }
        }
        if !self.literal_vars(expr)?.is_empty() {
            return None;
        }
        let v = self.eval_literal(expr, &[]).ok()?;
        Some((v < 0, v.unsigned_abs()))
    }

    /// Literal ifadesinin değeri; döngü değişkenleri `env`'den.
    fn eval_literal(&self, expr: Idx<Expr>, env: &[(DefId, i128)]) -> Result<i128, Fault> {
        let e = &self.ast.exprs[expr];
        if let ExprKind::IntLit { value, .. } = &e.kind {
            return i128::try_from(*value).map_err(|_| Fault::Overflow(e.span));
        }
        if let ExprKind::Unary { operand, .. } = &e.kind {
            // -2^127 = i128::MIN aralıktadır; işleneni 2^127 değildir (#207).
            if matches!(self.ast.exprs[*operand].kind,
                ExprKind::IntLit { value, .. } if value == 1u128 << 127)
            {
                return Ok(i128::MIN);
            }
            return self
                .eval_literal(*operand, env)?
                .checked_neg()
                .ok_or(Fault::Overflow(e.span));
        }
        if let ExprKind::Binary { op, lhs, rhs } = &e.kind {
            let l = self.eval_literal(*lhs, env)?;
            let r = self.eval_literal(*rhs, env)?;
            if matches!(op, BinOp::Div | BinOp::Rem) && r == 0 {
                return Err(Fault::DivZero);
            }
            return literal_op(*op, l, r).ok_or(Fault::Overflow(e.span));
        }
        // `literal_vars` yalnız sınırı bilinen döngü değişkenini geçirir;
        // değeri ortamdadır.
        let (def, ..) = self
            .bounded_loop_var(expr)
            .expect("literal_vars yalnız literal biçimleri geçirir");
        Ok(env
            .iter()
            .find(|(d, _)| *d == def)
            .map(|(_, v)| *v)
            .expect("döngü değişkeni ortamda"))
    }

    /// Döngü değişkenlerinin her değer bileşimi için `f`; `f` `false`
    /// dönerse durur. Bileşim sayısına üst sınır konmaz: şartname koymaz,
    /// üretici blok içi döngünün her bileşimini zaten açar (döngü başına
    /// sınır E2027); burada sınır, aşan bileşimde sığmayan değeri sessizce
    /// geçirirdi (#152).
    fn for_each_assignment(&self, vars: &[LoopVar], mut f: impl FnMut(&[(DefId, i128)]) -> bool) {
        if vars.iter().any(|&(_, s, e)| s >= e) {
            return; // boş döngü: gövde hiç açılmaz
        }
        let mut env: Env = vars.iter().map(|&(d, s, _)| (d, i128::from(s))).collect();
        loop {
            if !f(&env) {
                return;
            }
            // Sayaç gibi ilerlet: son değişken en hızlı.
            let mut k = vars.len();
            loop {
                if k == 0 {
                    return;
                }
                k -= 1;
                env[k].1 += 1;
                if env[k].1 < i128::from(vars[k].2) {
                    break;
                }
                env[k].1 = i128::from(vars[k].1);
            }
        }
    }

    /// Check kipi (§4): `expr` literal ifadesiyse (en az bir aritmetik
    /// işlem ya da döngü değişkeni taşıyan) değeri `expected`'a sığmalı;
    /// sığmazsa E2010, `i128` taşması E2022. Çıplak literal
    /// `check_int_lit`'in yoludur; sıfıra bölmeyi sentez yolu bildirir
    /// (`check_literal_divisor`), üst düzey `const`'ın taşmasını const-eval.
    pub(super) fn check_literal_expr(&mut self, expr: Idx<Expr>, expected: TypeId) {
        let kind = &self.ast.exprs[expr].kind;
        let compound = match kind {
            ExprKind::Binary { op, .. } => is_arith(*op),
            // `-<literal>` de literal ifadesidir: değeri hedefe sığmalı
            // (#213; çıplak literalin yolu `check_int_lit` negatif değeri
            // göremez). Sonekli literalin olumsuzu literal ifadesi değildir
            // (`literal_vars` onu dışarıda bırakır).
            ExprKind::Unary { op: UnOp::Neg, .. } => true,
            ExprKind::Path(_) => true,
            ExprKind::IntLit { .. }
            | ExprKind::BoolLit(_)
            | ExprKind::StringLit(_)
            | ExprKind::Unary { .. }
            | ExprKind::Index { .. }
            | ExprKind::Range { .. }
            | ExprKind::PartSelect { .. }
            | ExprKind::Field { .. }
            | ExprKind::Call { .. }
            | ExprKind::Cast { .. }
            | ExprKind::If { .. }
            | ExprKind::Match { .. }
            | ExprKind::StructLit { .. }
            | ExprKind::ArrayLit(_)
            | ExprKind::TupleLit(_)
            | ExprKind::Concat(_)
            | ExprKind::Todo { .. }
            | ExprKind::Error => false,
        };
        if !compound {
            return;
        }
        let Some((min, max)) = self.literal_range(expected) else {
            return;
        };
        let Some(vars) = self.literal_vars(expr) else {
            return;
        };
        let in_const = self.in_const_item;
        let mut found: Option<Finding> = None;
        self.for_each_assignment(&vars, |env| {
            let value = self.eval_literal(expr, env);
            if let Ok(v) = value {
                if v >= min && (v < 0 || v.unsigned_abs() <= max) {
                    return true;
                }
                found = Some(Finding::OutOfRange(v, env.to_vec()));
            } else if let Err(Fault::Overflow(at)) = value {
                // Üst düzey `const`'ın taşmasını const-eval bildirir.
                if !in_const {
                    found = Some(Finding::Overflow(at, env.to_vec()));
                }
            }
            // Sıfıra bölme sentez yolunda raporlanır.
            false
        });
        let span = self.ast.exprs[expr].span;
        let (diag, env) = match found {
            None => return,
            Some(Finding::OutOfRange(value, env)) => (
                self.literal_out_of_range(value, expected, min, max, span),
                env,
            ),
            Some(Finding::Overflow(at, env)) => (
                Diagnostic::error(
                    ErrorCode::E2022,
                    lstr!(en: "compile-time overflow in a literal expression"; tr: "literal ifadesinde derleme zamanı taşması"),
                    LabeledSpan::primary(
                        at,
                        lstr!(en: "the value exceeds the 128-bit compile-time range"; tr: "değer 128 bitlik derleme zamanı aralığını aşıyor"),
                    ),
                    lstr!(en: "use smaller literals"; tr: "daha küçük literaller kullanın"),
                ),
                env,
            ),
        };
        let diag = self.with_loop_values(diag, &vars, &env);
        self.push_once(diag);
    }

    /// Sentez yolu: iki yanı literal olan `/` ve `%`'nin böleni bir
    /// yinelemede 0 ise E2023 (const-eval.md §4 `DivisionByZero`, §12
    /// `1 / 0`). Bölünen değerlendirilmez: sıfıra bölme bölüneni
    /// gerektirmez. Üst düzey `const`'ta tanıyı const-eval verir.
    pub(super) fn check_literal_divisor(
        &mut self,
        op: BinOp,
        lhs: Idx<Expr>,
        rhs: Idx<Expr>,
        span: Span,
    ) {
        if !matches!(op, BinOp::Div | BinOp::Rem) || self.in_const_item {
            return;
        }
        // Bölünen de literal ifadesi olmalı: `x / 0` çalışma zamanı bölmesidir.
        let (Some(_), Some(rvars)) = (self.literal_vars(lhs), self.literal_vars(rhs)) else {
            return;
        };
        let mut zero_at: Option<Env> = None;
        self.for_each_assignment(&rvars, |env| {
            if matches!(self.eval_literal(rhs, env), Ok(0)) {
                zero_at = Some(env.to_vec());
                return false;
            }
            true
        });
        let Some(env) = zero_at else {
            return;
        };
        let message = if op == BinOp::Rem {
            lstr!(en: "remainder by zero"; tr: "sıfıra göre kalan")
        } else {
            lstr!(en: "division by zero"; tr: "sıfıra bölme")
        };
        let diag = Diagnostic::error(
            ErrorCode::E2023,
            message,
            LabeledSpan::primary(
                span,
                lstr!(en: "the divisor is 0 at compile time"; tr: "bölen derleme zamanında 0"),
            ),
            lstr!(en: "make the divisor a nonzero constant"; tr: "böleni sıfırdan farklı bir sabit yapın"),
        )
        .with_note(
            NoteKind::Reason,
            lstr!(en: "literal arithmetic is evaluated at compile time; its result must be a number"; tr: "literal aritmetiği derleme zamanında hesaplanır; sonucu bir sayı olmalı"),
        );
        let diag = self.with_loop_values(diag, &rvars, &env);
        self.push_once(diag);
    }

    /// Aynı ifade hem sentezlenip hem denetlenebilir (karşılaştırmanın ve
    /// aritmetiğin literal yanı önce sentezlenir, sonra karşı tipe göre
    /// denetlenir): aynı kod ve konumdaki tanı bir kez eklenir.
    fn push_once(&mut self, diag: Diagnostic) {
        let span = diag.primary_span().map(|s| s.span);
        let seen = self
            .diagnostics
            .iter()
            .any(|d| d.code == diag.code && d.primary_span().map(|s| s.span) == span);
        if !seen {
            self.diagnostics.push(diag);
        }
    }

    /// Beklenen tipin değer aralığı: `uN`, `iN`, esnek aralığın üst ucu
    /// (`check_int_lit` gibi), Trit `-1..=1`. Başka tiplerin uyumsuzluğunu
    /// atanabilirlik denetimi bildirir.
    ///
    /// Alt sınır `i128`, üst sınır `u128`: 127 bit ve üstü genişlikte de
    /// aralık vardır (#272). Literal ifadesinin değeri `i128`'dir; `u128`
    /// bağlamda üst sınır pratikte bağlamaz ama negatif değer sığmaz,
    /// işaretli 128 bit ve üstüne her `i128` sığar.
    fn literal_range(&self, expected: TypeId) -> Option<(i128, u128)> {
        let ty = self.types.ty(expected);
        if let Ty::UInt { width } | Ty::UIntFlex { hi: width, .. } = *ty {
            let max = if width >= 128 {
                u128::MAX
            } else {
                (1u128 << width) - 1
            };
            return Some((0, max));
        }
        if let Ty::SInt { width } | Ty::SIntFlex { hi: width, .. } = *ty {
            if width == 0 {
                return None;
            }
            if width >= 128 {
                return Some((i128::MIN, i128::MAX.unsigned_abs()));
            }
            let half = 1i128 << (width - 1);
            return Some((-half, (half - 1).unsigned_abs()));
        }
        matches!(ty, Ty::Trit).then_some((-1, 1))
    }

    /// E2010 (Trit için E2011): literal ifadesinin değeri aralık dışında.
    fn literal_out_of_range(
        &self,
        value: i128,
        expected: TypeId,
        min: i128,
        max: u128,
        span: Span,
    ) -> Diagnostic {
        let shown = self.show(expected);
        let code = if matches!(self.types.ty(expected), Ty::Trit) {
            ErrorCode::E2011
        } else {
            ErrorCode::E2010
        };
        Diagnostic::error(
            code,
            lstr!(en: "literal expression evaluates to {value}, which does not fit in type {shown} ({min}..={max})"; tr: "literal ifadesinin değeri {value}, {shown} tipine sığmıyor ({min}..={max})"),
            LabeledSpan::primary(
                span,
                lstr!(en: "evaluates to {value} at compile time"; tr: "derleme zamanında {value}"),
            ),
            lstr!(en: "use a wider type or change the literals so that the result fits"; tr: "daha geniş bir tip kullanın ya da sonucun sığacağı literaller yazın"),
        )
        .with_note(
            NoteKind::Reason,
            lstr!(en: "arithmetic on literals is computed exactly at compile time; the result is not wrapped to the target width"; tr: "literal aritmetiği derleme zamanında tam değerle hesaplanır; sonuç hedef genişliğe sarılmaz"),
        )
    }

    /// Döngü değişkeni taşıyan ifadede hangi yinelemenin hatalı olduğu.
    fn with_loop_values(
        &self,
        diag: Diagnostic,
        vars: &[LoopVar],
        env: &[(DefId, i128)],
    ) -> Diagnostic {
        if vars.is_empty() {
            return diag;
        }
        let values: Vec<String> = env
            .iter()
            .filter(|(d, _)| vars.iter().any(|v| v.0 == *d))
            .map(|(d, v)| format!("{} = {v}", self.res.defs[d.0 as usize].name))
            .collect();
        let values = values.join(", ");
        diag.with_note(
            NoteKind::Note,
            lstr!(en: "in the iteration {values}"; tr: "{values} yinelemesinde"),
        )
    }
}

fn is_arith(op: BinOp) -> bool {
    matches!(
        op,
        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem
    )
}

/// İki literalin aritmetiği, `i128`'de (const-eval.md §4 `checked_*`);
/// taşma ya da bölen 0 `None`. Yalnız aritmetik işleçle çağrılır.
fn literal_op(op: BinOp, l: i128, r: i128) -> Option<i128> {
    match op {
        BinOp::Add => l.checked_add(r),
        BinOp::Sub => l.checked_sub(r),
        BinOp::Mul => l.checked_mul(r),
        BinOp::Div => l.checked_div(r),
        BinOp::Rem => l.checked_rem(r),
        BinOp::BitAnd
        | BinOp::BitOr
        | BinOp::BitXor
        | BinOp::Shl
        | BinOp::Shr
        | BinOp::Eq
        | BinOp::Ne
        | BinOp::Lt
        | BinOp::Gt
        | BinOp::Le
        | BinOp::Ge
        | BinOp::And
        | BinOp::Or
        | BinOp::Imp => unreachable!("literal_op yalnız aritmetik işleçle çağrılır"),
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{codes, diagnostics};

    fn module(body: &str) -> String {
        format!("module M {{\n{body}\n}}\n")
    }

    /// Son yinelemenin değeri (i = 3) u2'ye sığar; sınırın kendisi (4)
    /// döngü değeri değildir (`[start, end)`).
    #[test]
    fn the_end_of_the_range_is_not_an_iteration() {
        let c = codes(&module(
            "    out ys : [u2; 4]\n    comb {\n        for i in 0..4 {\n            ys[i] = i\n        }\n    }",
        ));
        assert!(!c.contains(&"E2010"), "{c:?}");
    }

    /// İç içe döngüde her bileşim denetlenir; sığmayan tek bileşim
    /// sonuncusudur (i = 1, j = 2: 2 > 1) ve not onu adlandırır.
    #[test]
    fn every_combination_of_nested_loops_is_checked() {
        let d = diagnostics(&module(
            "    out zs : [u1; 6]\n    comb {\n        for i in 0..2 {\n            for j in 0..3 {\n                zs[i * 3 + j] = i * j\n            }\n        }\n    }",
        ));
        let e = d
            .iter()
            .find(|d| d.code.as_str() == "E2010")
            .expect("E2010 beklenir");
        assert!(e.message.contains("evaluates to 2"), "{}", e.message);
        assert!(
            e.notes.iter().any(|n| n.text.contains("i = 1, j = 2")),
            "{:?}",
            e.notes
        );
        assert_eq!(d.iter().filter(|d| d.code.as_str() == "E2010").count(), 1);
    }

    /// Bileşik ifadenin `-`'si literal ifadesidir: `-(2 + 1)` = -3 u2'ye
    /// sığmaz, `-(1 - 2)` = 1 sığar.
    #[test]
    fn negation_of_literal_arithmetic_is_folded() {
        let bad = codes(&module("    out y : u2\n    y = -(2 + 1)"));
        assert!(bad.contains(&"E2010"), "{bad:?}");
        let good = codes(&module("    out y : u2\n    y = -(1 - 2)"));
        assert!(!good.contains(&"E2010"), "{good:?}");
    }

    /// Sığma denetimi işaretli aralığın iki ucunu da kullanır: i4'te -8 ve
    /// 7 sığar, -9 ve 8 sığmaz.
    #[test]
    fn signed_range_bounds() {
        for (expr, fits) in [
            ("0 - 8", true),
            ("3 + 4", true),
            ("0 - 9", false),
            ("4 + 4", false),
        ] {
            let c = codes(&module(&format!("    out y : i4\n    y = {expr}")));
            assert_eq!(!c.contains(&"E2010"), fits, "{expr}: {c:?}");
        }
    }

    /// Aynı ifade hem sentezlenip hem denetlendiğinde tanı bir kez
    /// (karşılaştırmanın literal yanı); farklı konumdaki iki tanı ikisi de.
    #[test]
    fn a_diagnostic_is_reported_once_per_position() {
        let one = codes(&module(
            "    in  x : u1\n    out c : bool\n    c = x == 7 / 0",
        ));
        assert_eq!(one.iter().filter(|c| **c == "E2023").count(), 1, "{one:?}");
        let two = codes(&module(
            "    out a : u2\n    out b : u2\n    a = 7 / 0\n    b = 5 % 0",
        ));
        assert_eq!(two.iter().filter(|c| **c == "E2023").count(), 2, "{two:?}");
    }

    /// Dört iç içe döngünün her bileşimi (16⁴ = 65 536) denetlenir: `0..16`
    /// döngülerinde `i * j * k * l`'nin en büyüğü 15⁴ = 50 625, u15'e (en
    /// çok 32 767) sığmaz.
    #[test]
    fn four_nested_loops_are_checked_in_every_combination() {
        let c = codes(&module(
            "    out y : u15\n    comb {\n        for i in 0..16 {\n            for j in 0..16 {\n                for k in 0..16 {\n                    for l in 0..16 {\n                        y = i * j * k * l\n                    }\n                }\n            }\n        }\n    }",
        ));
        assert!(c.contains(&"E2010"), "{c:?}");
    }

    /// Sıfır bölenli `%` kalanı, `/` bölmeyi adlandırır.
    #[test]
    fn the_zero_divisor_message_names_the_operator() {
        for (expr, text) in [
            ("7 % 0", "remainder by zero"),
            ("7 / 0", "division by zero"),
        ] {
            let d = diagnostics(&module(&format!("    out y : u3\n    y = {expr}")));
            let e = d
                .iter()
                .find(|d| d.code.as_str() == "E2023")
                .unwrap_or_else(|| panic!("{expr}: E2023 beklenir"));
            assert_eq!(e.message.to_string(), text, "{expr}");
        }
    }

    /// 127 bitlik ve daha geniş hedefin aralığı `i128`'de yazılmaz; literal
    /// aritmetiği orada sığma denetimine girmez ve derleyici düşmez.
    #[test]
    fn a_target_of_127_bits_or_more_is_not_range_checked() {
        for width in [127, 128] {
            let c = codes(&module(&format!("    out y : u{width}\n    y = 5 + 2")));
            assert!(!c.contains(&"E2010"), "u{width}: {c:?}");
        }
    }

    /// Trit'in aralığı -1..=1: `0 - 1`, `1 - 1` ve `2 - 1` sığar, `0 - 2`
    /// sığmaz (E2011).
    #[test]
    fn trit_range_bounds() {
        for (expr, fits) in [
            ("0 - 1", true),
            ("1 - 1", true),
            ("2 - 1", true),
            ("0 - 2", false),
        ] {
            let c = codes(&module(&format!("    out t : Trit\n    t = {expr}")));
            assert_eq!(!c.contains(&"E2011"), fits, "{expr}: {c:?}");
        }
    }

    /// Tek döngü değişkeninde not hatalı yinelemeyi adlandırır: `i + 2`
    /// i = 2'de 4, u2'ye sığmaz (i = 0, 1 sığar; 2'de durulur).
    #[test]
    fn the_note_names_the_failing_iteration_of_a_single_loop() {
        let d = diagnostics(&module(
            "    out ys : [u2; 4]\n    comb {\n        for i in 0..4 {\n            ys[i] = i + 2\n        }\n    }",
        ));
        let e = d
            .iter()
            .find(|d| d.code.as_str() == "E2010")
            .expect("E2010 beklenir");
        assert!(e.message.contains("evaluates to 4"), "{}", e.message);
        assert!(
            e.notes.iter().any(|n| n.text == "in the iteration i = 2"),
            "{:?}",
            e.notes
        );
    }

    /// Bileşim sayısının üst sınırı yoktur (şartname koymaz; üretici de
    /// her bileşimi açar): üç `0..41` döngüsü 68 921 bileşimdir, son
    /// bileşimde `i * j * k` = 40³ = 64 000 u15'e (en çok 32 767) sığmaz.
    #[test]
    fn a_loop_nest_above_65536_combinations_is_checked() {
        let d = diagnostics(&module(
            "    out y : u15\n    comb {\n        for i in 0..41 {\n            for j in 0..41 {\n                for k in 0..41 {\n                    y = i * j * k\n                }\n            }\n        }\n    }",
        ));
        let e = d
            .iter()
            .find(|d| d.code.as_str() == "E2010")
            .expect("E2010 beklenir");
        assert!(
            e.message.contains("does not fit in type u15"),
            "{}",
            e.message
        );
    }

    /// #207: -2^127 i128'in en küçük değeridir, taşma değildir; i64'e
    /// sığmayan değer E2010'dur. `2^127` literalinin kendisi i128'e
    /// sığmaz, onun bileşik ifadesi E2022'dir (değerlendiricinin aralığı).
    /// Düzeltmeden önce ilk ifade de E2022 veriyordu.
    #[test]
    fn issue_207_minus_two_pow_127_is_in_range_and_two_pow_127_is_not() {
        let d = diagnostics(&module(
            "    out y : i64
    y = -170141183460469231731687303715884105728 + 1",
        ));
        let got: Vec<&str> = d.iter().map(|d| d.code.as_str()).collect();
        assert_eq!(got, ["E2010"], "{got:?}");
        assert!(
            d[0].message
                .contains("-170141183460469231731687303715884105727"),
            "{}",
            d[0].message
        );
        let c = codes(&module(
            "    out y : i64
    y = 170141183460469231731687303715884105728 - 1",
        ));
        assert_eq!(c, ["E2022"], "{c:?}");
    }

    /// #213: `-<literal>` beklenen tipin aralığına sığmalı (E2010), çıplak
    /// literal gibi. Önceden ne katlanmış denetime ne `check_int_lit`'e
    /// giriyordu: `-129 : i8` `8'sh7F` (+127) yazılıyordu.
    #[test]
    fn issue_213_negative_literal_must_fit_the_expected_type() {
        for (decl, body) in [
            ("    out b : i8", "    b = -129"),
            (
                "    in  i : i8
    out c : bool",
                "    c = i < -500",
            ),
            (
                "    in  i : i8
    out f : i9",
                "    f = i + -300",
            ),
            ("    out a : u8", "    a = -1"),
            ("    out a : i64", "    a = -0x8000_0000_0000_0001"),
            (
                "    out a : i64",
                "    a = -0x8000_0000_0000_0000_0000_0000_0000_0000",
            ),
        ] {
            let c = codes(&module(&format!(
                "{decl}
{body}"
            )));
            assert_eq!(c, ["E2010"], "{body}: {c:?}");
        }
        for (decl, body) in [
            ("    out b : i8", "    b = -128"),
            (
                "    in  i : i8
    out c : bool",
                "    c = i < -128",
            ),
            ("    out a : i64", "    a = -0x8000_0000_0000_0000"),
            ("    out a : i8", "    a = -0"),
        ] {
            let c = codes(&module(&format!(
                "{decl}
{body}"
            )));
            assert!(c.is_empty(), "{body}: {c:?}");
        }
    }

    /// #272: 127 bit ve üstü bağlamda da literal ifadesi aralığa sığmalı.
    /// Önceden `literal_range` bu genişliklerde `None` dönüyordu:
    /// `a * b == -1` (a, b : u64, u128 bağlam) hatasız derleniyor,
    /// `-128'd1` (2^128 - 1) ile karşılaştırıyordu. İşaretli 128 bit ve
    /// üstüne her i128 değeri sığar.
    #[test]
    fn issue_272_literals_in_contexts_of_127_bits_and_more_are_checked() {
        let ports = "    in  a : u64
    in  b : u64
    in  c : i64
    in  d : i64
    out o : bool";
        for body in [
            "    o = a * b == -1",
            "    o = a * b < -5",
            "    o = a * b == -(2 - 1)",
        ] {
            let c = codes(&module(&format!(
                "{ports}
{body}"
            )));
            assert_eq!(c, ["E2010"], "{body}: {c:?}");
        }
        for body in [
            "    o = a * b == 0",
            "    o = a * b == 0x7FFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF",
            "    o = c * d == -170141183460469231731687303715884105728",
            "    o = -(c * d) == -0x7FFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF",
        ] {
            let c = codes(&module(&format!(
                "{ports}
{body}"
            )));
            assert!(c.is_empty(), "{body}: {c:?}");
        }
    }
}
