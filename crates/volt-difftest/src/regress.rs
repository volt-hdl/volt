//! Sabit regresyon kümesi: fark testinin bulduğu her kök nedenin en küçük
//! örneği. Her koşunun ilk toplu koşusudur (CI'daki küçük koşu dahil);
//! `VOLT_DIFFTEST_COUNT=0` yalnız bunu koşturur. Her örnek bulunduğu
//! issue'yu taşır.

use crate::ast::{BinOp, Expr, LitFmt, PortTy, UnOp};

fn p(k: usize) -> Expr {
    Expr::Port(k)
}

fn lit(v: i128) -> Expr {
    Expr::Lit(v, LitFmt::Dec)
}

fn hex(v: i128) -> Expr {
    Expr::Lit(v, LitFmt::Hex)
}

fn bin(op: BinOp, a: Expr, b: Expr) -> Expr {
    Expr::bin(op, a, b)
}

fn cast(a: Expr, t: PortTy) -> Expr {
    Expr::cast(a, t)
}

fn range(a: Expr, h: u32, l: u32) -> Expr {
    Expr::Range(Box::new(a), h, l)
}

fn index(a: Expr, i: Expr) -> Expr {
    Expr::Index(Box::new(a), Box::new(i))
}

/// Regresyon örneği: issue, açıklama, port tipleri, ifade.
pub struct Regression {
    pub issue: &'static str,
    pub what: &'static str,
    pub ports: Vec<PortTy>,
    pub expr: Expr,
}

fn r(issue: &'static str, what: &'static str, ports: Vec<PortTy>, expr: Expr) -> Regression {
    Regression {
        issue,
        what,
        ports,
        expr,
    }
}

pub fn all() -> Vec<Regression> {
    use PortTy::*;
    vec![
        // ── `as`: SV ifadesinin işareti hedefin işareti değil ──
        r(
            "#115",
            "sign-extending cast to a signed type compares unsigned",
            vec![SInt(10)],
            bin(BinOp::Gt, cast(p(0), SInt(11)), lit(880)),
        ),
        r(
            "#115",
            "narrowing cast to a signed type compares unsigned",
            vec![SInt(36)],
            bin(BinOp::Gt, cast(p(0), SInt(1)), lit(0)),
        ),
        r(
            "#115",
            "zero-extending cast to a signed type shifts logically",
            vec![UInt(22), UInt(1)],
            bin(
                BinOp::Gt,
                bin(BinOp::Shr, cast(p(0), SInt(38)), p(1)),
                lit(-92_102_428_598),
            ),
        ),
        r(
            "#115",
            "sign-extending cast then right shift is logical",
            vec![SInt(7)],
            bin(BinOp::Shr, cast(p(0), SInt(50)), lit(8)),
        ),
        r(
            "#115",
            "narrowing a signed sum to an unsigned type keeps the sign",
            vec![SInt(14)],
            cast(bin(BinOp::Add, p(0), p(0)), UInt(7)),
        ),
        r(
            "#115",
            "negating a sign-extended value",
            vec![SInt(2)],
            Expr::un(UnOp::Neg, cast(p(0), SInt(4))),
        ),
        // ── literaller ──
        r(
            "#116",
            "hex literal in a signed comparison",
            vec![SInt(50)],
            bin(BinOp::Gt, p(0), hex(0)),
        ),
        r(
            "#116",
            "hex literal in a signed bitwise operation",
            vec![SInt(18)],
            bin(BinOp::Gt, bin(BinOp::Or, p(0), hex(0x1FFFF)), lit(95146)),
        ),
        r(
            "#116",
            "most negative literal of the width",
            vec![SInt(1)],
            cast(bin(BinOp::Mul, p(0), lit(-1)), UInt(35)),
        ),
        // ── değeri bağlama göre değişen alt ifade (#106 sınıfı) ──
        r(
            "#117",
            "unsigned subtraction under a wider product",
            vec![UInt(12), UInt(12)],
            bin(BinOp::Mul, bin(BinOp::Sub, p(0), lit(4095)), p(1)),
        ),
        r(
            "#117",
            "unsigned subtraction under a wider sum",
            vec![UInt(8), UInt(8), UInt(9)],
            bin(BinOp::Add, bin(BinOp::Sub, p(0), p(1)), p(2)),
        ),
        r(
            "#117",
            "unsigned subtraction in a widening cast",
            vec![UInt(8), UInt(8)],
            cast(bin(BinOp::Sub, p(0), p(1)), UInt(16)),
        ),
        r(
            "#117",
            "carry of an operand dropped by a bitwise operation, in a wider sum",
            vec![UInt(8), UInt(8), UInt(8)],
            bin(
                BinOp::Add,
                bin(BinOp::Or, bin(BinOp::Add, p(0), p(1)), p(2)),
                p(2),
            ),
        ),
        r(
            "#117",
            "right shift of a sum reads the carry",
            vec![UInt(8), UInt(8), UInt(3)],
            bin(BinOp::Shr, bin(BinOp::Add, p(0), p(1)), p(2)),
        ),
        r(
            "#117",
            "negation inside a shift",
            vec![SInt(20), UInt(2)],
            bin(BinOp::Shr, Expr::un(UnOp::Neg, p(0)), p(1)),
        ),
        r(
            "#117",
            "~ under a same-width cast",
            vec![Bits(10)],
            bin(
                BinOp::Add,
                cast(Expr::un(UnOp::BitNot, p(0)), UInt(10)),
                lit(0),
            ),
        ),
        // ── geçersiz SV ──
        r(
            "#118",
            "-(-x) is written as the SV decrement operator",
            vec![SInt(56)],
            Expr::un(UnOp::Neg, Expr::un(UnOp::Neg, p(0))),
        ),
        r(
            "#119",
            "range of a range of a computed value",
            vec![Bits(70), UInt(6)],
            range(range(bin(BinOp::Shr, p(0), p(1)), 69, 0), 69, 0),
        ),
        r(
            "#119",
            "bit of a range of a cast",
            vec![UInt(34)],
            index(range(cast(p(0), Bits(34)), 28, 0), lit(27)),
        ),
        r(
            "#120",
            "bit 0 of a one-bit value",
            vec![UInt(1)],
            index(p(0), lit(0)),
        ),
        r(
            "#120",
            "range [0:0] of bits<1>",
            vec![Bits(1)],
            range(p(0), 0, 0),
        ),
        r(
            "#120",
            "bit 0 of a signed one-bit value is unsigned (part-select start)",
            vec![Bits(69), SInt(1)],
            Expr::Part(
                Box::new(p(0)),
                Box::new(cast(range(p(1), 0, 0), UInt(1))),
                68,
                true,
            ),
        ),
        // ── Verilator WIDTH (ölümcül): indeks adres genişliğinde değil ──
        r(
            "#123",
            "variable bit index narrower than the address width",
            vec![UInt(15), UInt(3)],
            index(p(0), p(1)),
        ),
        r(
            "#123",
            "variable part-select start wider than the address width",
            vec![UInt(51), UInt(8)],
            Expr::Part(Box::new(p(0)), Box::new(p(1)), 35, true),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Model;

    #[test]
    fn every_regression_is_valid_in_the_model() {
        for r in all() {
            let m = Model { ports: &r.ports };
            assert!(
                m.synth(&r.expr).is_ok(),
                "{}: {:?}",
                r.what,
                m.synth(&r.expr)
            );
        }
    }
}
