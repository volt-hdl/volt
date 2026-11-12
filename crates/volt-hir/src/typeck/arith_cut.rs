//! W2016 (ADR-0116, #266): `+ - *`'nın işleneni işlemden önce kesiliyor.
//!
//! `+ - *` iki işlenenin aralıklarının kesişiminde, kendi taşmasıyla
//! yapılır (type-inference.md §3.3, ADR-0025): işlem genişliği `C`
//! kesişimin üst ucu + 1, `*` için iki katı. Doğal genişliği `C`'den geniş
//! (esnek) işlenen işlemden önce `C` bite kesilir: `a * b - a` (a, b : i32)
//! 33 bitte hesaplanır, 64 bitlik çarpım önce 33 bite iner. Kural
//! v0.2.0'da değişecek (#266): işlenen doğal genişliğinde kalacak. O
//! zamana kadar uyarı; üretilen SV değişmez.
//!
//! Uyarı, değerin bu kurala bağlı olduğu her yerde verilir; iki durumda
//! değer kurala bağlı değildir ve uyarı yoktur (gerekçe ADR-0116'da,
//! sınayan test `arith_cut_property_tests`):
//!
//! - **Bağlam yalnız alt `k` biti okuyor ve `k <= C`** (`arith_demand`):
//!   somut genişliğe atanan aritmetik zincir sonucun yalnız `k` bitini
//!   tutar; `+ - *` ve bit işlemlerinin alt `k` biti işlenenlerin alt `k`
//!   bitinden hesaplanır, `k` bitten geniş her işlem genişliği bu bitleri
//!   korur (`r <= a * b + c`, biriktirici deseni).
//! - **Sonucun değer aralığı `C` bite sığıyor**: işlem kesilmiş
//!   işlenenlerle `C` bitte yapılınca sonuç gerçek sonuçla `2^C` modülünde
//!   eşittir; gerçek sonuç `C` bite sığıyorsa ikisi aynı sayıdır ve doğal
//!   genişlikteki hesap da onu verir (`for` üretimindeki `y * (N + 1) + N`
//!   dizinleri). Yalnız işlenenin sığması yetmez: `u * 2 + c` (u8)
//!   işleneni 9 bite sığar, toplamı sığmaz.
//!
//! Bölme ve kalan W2015'tir (ADR-0112); bir işlenen tek bir işlemin
//! işlenenidir, iki uyarı aynı yerde çıkmaz.

use volt_ast::{BinOp, Expr, ExprKind, Idx, UnOp};
use volt_diagnostics::{a_an, lstr, ErrorCode};

use super::width::{clamp_width, IntMeet, MAX_INT_WIDTH};
use super::TypeChecker;
use crate::resolve::DefKind;
use crate::ty::TypeId;

/// Kapalı değer aralığı `[lo, hi]`.
type Bounds = (i128, i128);

/// Verilmiş bir W2016: tanının dizini, `arith_demand` zincirinin kökü ve
/// işlem genişliği.
pub(super) struct ArithCut {
    diag: usize,
    root: Idx<Expr>,
    width: u16,
    dropped: bool,
}

impl TypeChecker<'_, '_> {
    /// `op` (`+ - *`) işlenen tipleri `lt`, `rt` iken doğal genişliği
    /// işlem genişliğinden geniş her işlenende W2016; bağlam sonucun
    /// işlem genişliğinden fazlasını okumuyorsa ya da sonucun değeri işlem
    /// genişliğine sığıyorsa hiç.
    pub(super) fn warn_cut_arith_operands(
        &mut self,
        op: BinOp,
        lhs: Idx<Expr>,
        rhs: Idx<Expr>,
        lt: TypeId,
        rt: TypeId,
    ) {
        let Some((signed, width)) = self.arith_width(op, lt, rt) else {
            return;
        };
        let cut: Vec<(Idx<Expr>, u16)> = [(lhs, lt), (rhs, rt)]
            .into_iter()
            .filter_map(|(e, t)| {
                let (_, _, natural) = self.types.int_range(t)?;
                (natural > width).then_some((e, natural))
            })
            .collect();
        if cut.is_empty() || self.arith_demand.is_some_and(|read| read <= width) {
            return;
        }
        let result = self
            .value_bounds(lhs)
            .zip(self.value_bounds(rhs))
            .and_then(|(l, r)| combine(op, l, r));
        if result.is_some_and(|b| fits(b, signed, width)) {
            return;
        }
        let sym = op.symbol();
        let p = if signed { "i" } else { "u" };
        // Önerilen `let`in genişliği: itme (ADR-0041, check kipi) bu
        // genişlikte her işleneni kesmeden genişletir.
        let fit = [lt, rt]
            .iter()
            .filter_map(|&t| self.types.int_range(t).map(|(_, _, n)| n))
            .max()
            .unwrap_or(width);
        for (e, natural) in cut {
            let span = self.ast.exprs[e].span;
            let help = if u32::from(fit) <= MAX_INT_WIDTH {
                lstr!(en: "put the result of this '{sym}' in a 'let' with an explicit width, e.g. 'let v : {p}{fit} = …': it is computed at {fit} bits and nothing is cut";
                      tr: "bu '{sym}' işleminin sonucunu genişliği açık bir 'let'e alın, ör. 'let v : {p}{fit} = …': {fit} bitte hesaplanır, hiçbir şey kesilmez")
            } else {
                lstr!(en: "no integer type is {fit} bits wide yet (the widest is {MAX_INT_WIDTH}), so the cut cannot be avoided until the rule changes in v0.2.0; see issue #266";
                      tr: "henüz {fit} bit genişliğinde tam sayı tipi yok (en genişi {MAX_INT_WIDTH}); kesme, kural v0.2.0'da değişene kadar önlenemiyor; bkz. issue #266")
            };
            if let Some(root) = self.arith_chain {
                self.arith_cuts.push(ArithCut {
                    diag: self.diagnostics.len(),
                    root,
                    width,
                    dropped: false,
                });
            }
            self.warning(
                ErrorCode::W2016,
                span,
                lstr!(en: "this operand of '{sym}' is cut to {width} bits before the operation (its natural width is {natural}); the rule changes in v0.2.0 (#266)";
                      tr: "'{sym}' işleminin bu işleneni işlemden önce {width} bite kesiliyor (doğal genişliği {natural}); kural v0.2.0'da değişecek (#266)"),
                lstr!(en: "{} {natural}-bit value, computed at {width} bits", a_an(u64::from(natural));
                      tr: "{natural} bitlik değer, {width} bitte hesaplanıyor"),
                help,
            );
        }
    }

    /// Ebeveyn `root`'un yalnız alt `k` bitini okuyor (W2015'li bölme,
    /// daraltan `as`): `root`'un zincirinde en az `k` bitte yapılan
    /// işlemin kesmesi bu bitleri değiştirmez, W2016'sı geri alınır.
    pub(super) fn read_low_bits(&mut self, root: Idx<Expr>, k: u16) {
        for cut in &mut self.arith_cuts {
            if cut.root == root && cut.width >= k {
                cut.dropped = true;
            }
        }
    }

    /// Geri alınan W2016'ları tanılardan çıkarır (denetimin sonunda).
    pub(super) fn drop_harmless_arith_cuts(&mut self) {
        let mut dropped: Vec<usize> = self
            .arith_cuts
            .iter()
            .filter(|c| c.dropped)
            .map(|c| c.diag)
            .collect();
        dropped.sort_unstable();
        for i in dropped.into_iter().rev() {
            self.diagnostics.remove(i);
        }
        self.arith_cuts.clear();
    }

    /// `+ - *`'nın işaret ve işlem genişliği (kesişimin üst ucu + 1, `*`
    /// için iki katı); tam sayı aralıkları buluşmuyorsa `None`.
    fn arith_width(&self, op: BinOp, lt: TypeId, rt: TypeId) -> Option<(bool, u16)> {
        let IntMeet::Common { signed, hi, .. } = self.meet_int_ranges(lt, rt) else {
            return None;
        };
        let width = if op == BinOp::Mul {
            clamp_width(u32::from(hi) * 2)
        } else {
            clamp_width(u32::from(hi) + 1)
        };
        Some((signed, width))
    }

    /// İfadenin değerinin kanıtlanan aralığı; kanıt yoksa `None`.
    ///
    /// Yapraklar: literal ve `const` adı (değeri SV'ye değer olarak
    /// yazılır), blok içi `for` değişkeni (sınırları), başka her ifade
    /// doğal tipinin aralığı. `+ - *` ve tekli `-` aralık aritmetiğiyle;
    /// sonuç düğümün tip aralığını aşarsa düğüm sarar, aralığı tip
    /// aralığıdır. İşlenenlerinden birini kesen `+ - *` düğümünün değeri
    /// kuralın kendisine bağlıdır: `None` (kanıt yok, uyarı kalır).
    fn value_bounds(&mut self, e: Idx<Expr>) -> Option<Bounds> {
        let ast = self.ast;
        if let ExprKind::Binary { op, lhs, rhs } = &ast.exprs[e].kind {
            if is_arith(*op) && self.cuts_an_operand(*op, *lhs, *rhs) {
                return None;
            }
        }
        let derived = self.derived_bounds(e);
        // Literal aritmetiği tam değerle katlanır (#144): tip aralığı yok.
        if self
            .expr_types
            .get(&e)
            .is_some_and(|&t| self.types.is_int_lit(t))
        {
            return derived;
        }
        let natural = self.type_bounds(e);
        match (derived, natural) {
            (Some(d), Some(t)) if d.0 >= t.0 && d.1 <= t.1 => Some(d),
            (_, t) => t,
        }
    }

    /// Yaprağın kesin değeri ya da `+ - *`, tekli `-` düğümünün aralık
    /// aritmetiği; başka biçim `None` (çağıran tip aralığını kullanır).
    fn derived_bounds(&mut self, e: Idx<Expr>) -> Option<Bounds> {
        let ast = self.ast;
        let kind = &ast.exprs[e].kind;
        if let ExprKind::IntLit { value, .. } = kind {
            return i128::try_from(*value).ok().map(|v| (v, v));
        }
        if let ExprKind::Path(_) = kind {
            if let Some((start, end)) = self.loop_var_bounds(e) {
                return (start < end).then(|| (i128::from(start), i128::from(end) - 1));
            }
            return if self.is_const_path(e) {
                self.try_const_eval(e).map(|v| (v, v))
            } else {
                None
            };
        }
        if let ExprKind::Binary { op, lhs, rhs } = kind {
            if !is_arith(*op) {
                return None;
            }
            let l = self.value_bounds(*lhs);
            let r = self.value_bounds(*rhs);
            return l.zip(r).and_then(|(l, r)| combine(*op, l, r));
        }
        if let ExprKind::Unary {
            op: UnOp::Neg,
            operand,
        } = kind
        {
            return self
                .value_bounds(*operand)
                .and_then(|(lo, hi)| Some((hi.checked_neg()?, lo.checked_neg()?)));
        }
        None
    }

    /// İfade bir `const` öğesinin adı mı.
    fn is_const_path(&self, e: Idx<Expr>) -> bool {
        self.res
            .resolutions
            .get(&e)
            .is_some_and(|&def| matches!(self.res.def_kind(def), DefKind::Const))
    }

    /// `+ - *` düğümü kendi işleneninden birini kesiyor mu.
    fn cuts_an_operand(&self, op: BinOp, lhs: Idx<Expr>, rhs: Idx<Expr>) -> bool {
        let (Some(&lt), Some(&rt)) = (self.expr_types.get(&lhs), self.expr_types.get(&rhs)) else {
            return true;
        };
        let Some((_, width)) = self.arith_width(op, lt, rt) else {
            return false;
        };
        [lt, rt].iter().any(|&t| {
            self.types
                .int_range(t)
                .is_some_and(|(_, _, natural)| natural > width)
        })
    }

    /// Kayıtlı tipin doğal genişliğindeki değer aralığı.
    fn type_bounds(&self, e: Idx<Expr>) -> Option<Bounds> {
        let &t = self.expr_types.get(&e)?;
        let (signed, _, n) = self.types.int_range(t)?;
        if n == 0 || n > 126 {
            return None;
        }
        Some(if signed {
            (-(1i128 << (n - 1)), (1i128 << (n - 1)) - 1)
        } else {
            (0, (1i128 << n) - 1)
        })
    }
}

/// `[lo, hi]` aralıklarının `+ - *`'sı; `i128` taşmasında `None`.
fn combine(op: BinOp, (a, b): Bounds, (c, d): Bounds) -> Option<Bounds> {
    if op == BinOp::Add {
        return Some((a.checked_add(c)?, b.checked_add(d)?));
    }
    if op == BinOp::Sub {
        return Some((a.checked_sub(d)?, b.checked_sub(c)?));
    }
    let p = [
        a.checked_mul(c)?,
        a.checked_mul(d)?,
        b.checked_mul(c)?,
        b.checked_mul(d)?,
    ];
    Some((*p.iter().min()?, *p.iter().max()?))
}

/// `+ - *`: W2016'nın baktığı işleçler.
fn is_arith(op: BinOp) -> bool {
    matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul)
}

/// `[lo, hi]` işaretli ya da işaretsiz `width` bite sığıyor mu.
fn fits((lo, hi): Bounds, signed: bool, width: u16) -> bool {
    if signed {
        // `i128` her değeri 128 bitlik işaretli aralıktadır.
        width >= 128 || {
            let half = 1i128 << (width - 1);
            lo >= -half && hi < half
        }
    } else {
        lo >= 0 && (width >= 127 || hi < (1i128 << width))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combine_follows_interval_arithmetic() {
        assert_eq!(combine(BinOp::Add, (0, 255), (0, 255)), Some((0, 510)));
        assert_eq!(combine(BinOp::Sub, (0, 255), (0, 255)), Some((-255, 255)));
        assert_eq!(
            combine(BinOp::Mul, (-128, 127), (-128, 127)),
            Some((-16256, 16384))
        );
        assert_eq!(combine(BinOp::Mul, (i128::MAX, i128::MAX), (2, 2)), None);
    }

    #[test]
    fn fits_respects_sign_and_width() {
        assert!(fits((0, 511), false, 9));
        assert!(!fits((0, 512), false, 9));
        assert!(!fits((-1, 0), false, 9));
        assert!(fits((-256, 255), true, 9));
        assert!(!fits((-257, 0), true, 9));
        assert!(!fits((0, 256), true, 9));
        assert!(fits((i128::MIN, i128::MAX), true, 128));
        assert!(!fits((1 << 126, 1 << 126), true, 127));
        assert!(fits((0, i128::MAX), false, 127));
        assert!(!fits((-1, 0), false, 127));
    }
}
