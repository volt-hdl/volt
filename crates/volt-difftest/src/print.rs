//! İfade ağacından Volt kaynak metni. İki biçim: en az parantez
//! (`operator-precedence.md` §1'e göre) ve tam parantez (her alt ifade,
//! atomlar dahil, parantez içinde). Metamorfik denetim ikisinin aynı
//! anlamı (ya da aynı tanıyı) vermesini ister.

use crate::ast::{BinOp, Expr, LitFmt};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Min,
    Full,
}

pub fn port_name(k: usize) -> String {
    format!("p{k}")
}

const PREC_UNARY: u8 = 11;
const PREC_POSTFIX: u8 = 12;
const PREC_ATOM: u8 = 13;

fn prec(e: &Expr) -> u8 {
    match e {
        Expr::Bin(op, ..) => op.prec(),
        Expr::Un(..) => PREC_UNARY,
        Expr::Lit(v, _) if *v < 0 => PREC_UNARY,
        Expr::Index(..) | Expr::Range(..) | Expr::Part(..) | Expr::Cast(..) => PREC_POSTFIX,
        Expr::Port(_) | Expr::Name(..) | Expr::Lit(..) | Expr::WideLit(..) | Expr::Concat(_) => {
            PREC_ATOM
        }
    }
}

fn neg_lit(e: &Expr) -> bool {
    matches!(e, Expr::Lit(v, _) if *v < 0)
}

fn is_cmp(e: &Expr) -> bool {
    matches!(e, Expr::Bin(op, ..) if op.is_cmp())
}

fn lit_text(v: i128, fmt: LitFmt) -> String {
    if v < 0 {
        return format!("-{}", v.unsigned_abs());
    }
    match fmt {
        LitFmt::Dec => format!("{v}"),
        LitFmt::Hex => format!("0x{v:X}"),
        LitFmt::Bin => format!("0b{v:b}"),
    }
}

fn wide_lit_text(v: u128, fmt: LitFmt) -> String {
    match fmt {
        LitFmt::Dec => format!("{v}"),
        LitFmt::Hex => format!("0x{v:X}"),
        LitFmt::Bin => format!("0b{v:b}"),
    }
}

pub fn print(e: &Expr, style: Style) -> String {
    let inner = match e {
        Expr::Port(k) => port_name(*k),
        Expr::Name(n, _) => n.clone(),
        Expr::Lit(v, f) => lit_text(*v, *f),
        Expr::WideLit(v, f) => wide_lit_text(*v, *f),
        Expr::Bin(op, a, b) => {
            let p = op.prec();
            let l = wrap(
                a,
                style,
                prec(a) < p || (op.is_cmp() && is_cmp(a)) || neg_lit(a),
            );
            let r = wrap(
                b,
                style,
                prec(b) <= p || (op.is_cmp() && is_cmp(b)) || neg_lit(b),
            );
            format!("{l} {} {r}", op.sym())
        }
        Expr::Un(op, a) => {
            let need = prec(a) < PREC_UNARY
                || matches!(&**a, Expr::Un(..))
                || matches!(&**a, Expr::Lit(v, _) if *v < 0);
            format!("{}{}", op.sym(), wrap(a, style, need))
        }
        Expr::Index(b, i) => format!("{}[{}]", base(b, style), print(i, style)),
        Expr::Range(b, h, l) => format!("{}[{h}:{l}]", base(b, style)),
        Expr::Part(b, s, w, up) => format!(
            "{}[{} {} {w}]",
            base(b, style),
            print(s, style),
            if *up { "+:" } else { "-:" }
        ),
        Expr::Cast(a, t) => {
            let need = prec(a) < PREC_POSTFIX;
            format!("{} as {}", wrap(a, style, need), t.volt())
        }
        Expr::Concat(xs) => {
            let args: Vec<String> = xs.iter().map(|x| print(x, style)).collect();
            format!("concat({})", args.join(", "))
        }
    };
    match style {
        Style::Full => format!("({inner})"),
        Style::Min => inner,
    }
}

fn wrap(e: &Expr, style: Style, need: bool) -> String {
    let s = print(e, style);
    if need && style == Style::Min {
        format!("({s})")
    } else {
        s
    }
}

/// Seçim tabanı: ad değilse parantez; dönüşüm tabanı da (`(x as u8)[3]`).
fn base(b: &Expr, style: Style) -> String {
    let need = prec(b) < PREC_POSTFIX || matches!(b, Expr::Cast(..));
    wrap(b, style, need)
}

/// Bir ifadenin yazımı, üst işlemin önceliğine göre gerekirse parantezle
/// (karşılaştırma bağlamı için).
pub fn print_operand(e: &Expr, parent: BinOp, style: Style) -> String {
    let need = prec(e) <= parent.prec() || (parent.is_cmp() && is_cmp(e)) || neg_lit(e);
    wrap(e, style, need)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{PortTy, UnOp};

    fn p(k: usize) -> Expr {
        Expr::Port(k)
    }

    #[test]
    fn minimal_parentheses_follow_precedence() {
        let e = Expr::bin(
            BinOp::Mul,
            Expr::bin(BinOp::Add, p(0), p(1)),
            Expr::bin(BinOp::Sub, p(2), p(3)),
        );
        assert_eq!(print(&e, Style::Min), "(p0 + p1) * (p2 - p3)");
        let e = Expr::bin(BinOp::Sub, p(0), Expr::bin(BinOp::Sub, p(1), p(2)));
        assert_eq!(print(&e, Style::Min), "p0 - (p1 - p2)");
        let e = Expr::bin(
            BinOp::Eq,
            Expr::bin(BinOp::Lt, p(0), p(1)),
            Expr::bin(BinOp::And, p(2), p(3)),
        );
        assert_eq!(print(&e, Style::Min), "(p0 < p1) == p2 & p3");
    }

    #[test]
    fn selection_and_cast_bases() {
        let sum = Expr::bin(BinOp::Add, p(0), p(1));
        let r = Expr::Range(Box::new(sum.clone()), 3, 0);
        assert_eq!(print(&r, Style::Min), "(p0 + p1)[3:0]");
        let c = Expr::cast(r, PortTy::UInt(4));
        assert_eq!(print(&c, Style::Min), "(p0 + p1)[3:0] as u4");
        let ci = Expr::Index(
            Box::new(Expr::cast(p(0), PortTy::UInt(8))),
            Box::new(Expr::Lit(2, LitFmt::Dec)),
        );
        assert_eq!(print(&ci, Style::Min), "(p0 as u8)[2]");
        let neg = Expr::cast(Expr::un(UnOp::Neg, p(0)), PortTy::SInt(4));
        assert_eq!(print(&neg, Style::Min), "(-p0) as i4");
        assert_eq!(print(&sum, Style::Full), "((p0) + (p1))");
    }

    #[test]
    fn literals() {
        assert_eq!(print(&Expr::Lit(255, LitFmt::Hex), Style::Min), "0xFF");
        assert_eq!(print(&Expr::Lit(5, LitFmt::Bin), Style::Min), "0b101");
        let e = Expr::bin(BinOp::Add, p(0), Expr::Lit(-3, LitFmt::Dec));
        assert_eq!(print(&e, Style::Min), "p0 + (-3)");
    }
}
