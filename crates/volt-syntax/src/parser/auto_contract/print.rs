//! Kontrat metni (ADR-0066 §4): otomatik kontrat kaynakta yazılı
//! olmadığından raporlar metni buradan alır. Yalnız üretilen biçimler
//! (ad, tam sayı, ikili/tekli işlem, `prev(x)`) yazılır; başkası None.
//! Öncelik tablosu docs/spec/operator-precedence.md ile aynıdır.

use volt_ast::{BinOp, Expr, ExprKind, Idx, IntSuffix, NumBase, SourceFile};

/// Tekli işlem önceliği (spec §11).
const PREC_UNARY: u8 = 11;
/// Atom / çağrı önceliği (spec §12).
const PREC_ATOM: u8 = 12;

pub(super) fn expr(ast: &SourceFile, e: Idx<Expr>) -> Option<String> {
    Some(printed(ast, e)?.0)
}

fn prec(op: BinOp) -> u8 {
    match op {
        BinOp::Imp => 0,
        BinOp::Or => 1,
        BinOp::And => 2,
        BinOp::Eq | BinOp::Ne => 3,
        BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => 4,
        BinOp::BitOr => 5,
        BinOp::BitXor => 6,
        BinOp::BitAnd => 7,
        BinOp::Shl | BinOp::Shr => 8,
        BinOp::Add | BinOp::Sub => 9,
        BinOp::Mul | BinOp::Div | BinOp::Rem => 10,
    }
}

/// (metin, önceliği).
fn printed(ast: &SourceFile, e: Idx<Expr>) -> Option<(String, u8)> {
    Some(match &ast.exprs[e].kind {
        ExprKind::IntLit {
            value,
            suffix,
            base,
        } => (int_text(*value, *suffix, *base), PREC_ATOM),
        ExprKind::BoolLit(b) => (b.to_string(), PREC_ATOM),
        ExprKind::Path(p) => (
            p.segments
                .iter()
                .map(|s| s.text.as_str())
                .collect::<Vec<_>>()
                .join("::"),
            PREC_ATOM,
        ),
        ExprKind::Call { callee, args } => {
            let (name, _) = printed(ast, *callee)?;
            let args = args
                .iter()
                .map(|&a| expr(ast, a))
                .collect::<Option<Vec<_>>>()?;
            (format!("{name}({})", args.join(", ")), PREC_ATOM)
        }
        ExprKind::Unary { op, operand } => {
            let (inner, p) = printed(ast, *operand)?;
            let inner = if p < PREC_UNARY {
                format!("({inner})")
            } else {
                inner
            };
            (format!("{}{inner}", op.symbol()), PREC_UNARY)
        }
        ExprKind::Binary { op, lhs, rhs } => {
            let me = prec(*op);
            let (l, lp) = printed(ast, *lhs)?;
            let (r, rp) = printed(ast, *rhs)?;
            // `->` sağdan, diğerleri soldan birleşir; eşitlik/karşılaştırma
            // birleşmez (E0010) — eşit öncelikte iki yan da paranteze girer.
            let right_assoc = *op == BinOp::Imp;
            let non_assoc = op.is_comparison();
            let wrap_l = lp < me || (lp == me && (right_assoc || non_assoc));
            let wrap_r = rp < me || (rp == me && !right_assoc);
            let l = if wrap_l { format!("({l})") } else { l };
            let r = if wrap_r { format!("({r})") } else { r };
            (format!("{l} {} {r}", op.symbol()), me)
        }
        _ => return None,
    })
}

fn int_text(value: u128, suffix: Option<IntSuffix>, base: NumBase) -> String {
    let digits = match base {
        NumBase::Dec => value.to_string(),
        NumBase::Hex => format!("0x{value:x}"),
        NumBase::Bin => format!("0b{value:b}"),
        NumBase::Oct => format!("0o{value:o}"),
    };
    let suffix = match suffix {
        None => "",
        Some(IntSuffix::U8) => "u8",
        Some(IntSuffix::U16) => "u16",
        Some(IntSuffix::U32) => "u32",
        Some(IntSuffix::U64) => "u64",
        Some(IntSuffix::I8) => "i8",
        Some(IntSuffix::I16) => "i16",
        Some(IntSuffix::I32) => "i32",
        Some(IntSuffix::I64) => "i64",
    };
    format!("{digits}{suffix}")
}
