//! Referans modelin Trit kuralları (#458), spesifikasyondan elle:
//! type-inference.md §3.3, §3.4, §3.6, §4, §10; ADR-0003; ADR-0062 Karar
//! 1–2 (kod +1 = `01`, 0 = `00`, -1 = `11`; geniş hedefte çarpım hedef
//! genişliğinde); ADR-0142 (`~`, bit işlemleri, kaydırma, bit seçme E2003).
//! Değerler kodun bit örüntüsüdür; A15'e (çarpımın taşması) ve A16'ya
//! (sıralama) bağlı değerler elle sabitlenmez, yalnız etiketleri sınanır.

use volt_difftest::ast::{BinOp, Expr, LitFmt, PortTy, UnOp};
use volt_difftest::model::{tag, Model, Reject, Ty};

const PORTS: &[PortTy] = &[
    PortTy::Trit,    // 0: t
    PortTy::Trit,    // 1: u
    PortTy::SInt(8), // 2: x
    PortTy::UInt(8), // 3: y
    PortTy::Bits(2), // 4: b
    PortTy::Bool,    // 5: f
];

const PLUS: u128 = 0b01;
const ZERO: u128 = 0b00;
const MINUS: u128 = 0b11;

fn m() -> Model<'static> {
    Model { ports: PORTS }
}

fn p(k: usize) -> Expr {
    Expr::Port(k)
}

fn lit(v: i128) -> Expr {
    Expr::Lit(v, LitFmt::Dec)
}

fn ty(e: &Expr) -> Result<Ty, Reject> {
    m().synth(e).map(|t| t.ty)
}

fn inp(t: u128, u: u128, x: i128) -> Vec<u128> {
    vec![t, u, (x as u128) & 0xFF, 0, 0, 0]
}

/// type-inference.md §10: `t * t` Trit, `t + t` i3, `t * x` (x : i8) i8,
/// `x as Trit` E2009, `~t` E2003.
#[test]
fn section_10_vectors_of_trit() {
    let tt = Expr::bin(BinOp::Mul, p(0), p(1));
    assert_eq!(ty(&tt), Ok(Ty::Trit));
    assert_eq!(ty(&Expr::bin(BinOp::Add, p(0), p(1))), Ok(Ty::int(true, 3)));
    assert_eq!(ty(&Expr::bin(BinOp::Sub, p(0), p(1))), Ok(Ty::int(true, 3)));
    assert_eq!(ty(&Expr::bin(BinOp::Mul, p(0), p(2))), Ok(Ty::int(true, 8)));
    assert_eq!(ty(&Expr::bin(BinOp::Mul, p(2), p(0))), Ok(Ty::int(true, 8)));
    assert_eq!(
        ty(&Expr::cast(p(2), PortTy::Trit)),
        Err(Reject::Invalid("E2009"))
    );
    assert_eq!(
        ty(&Expr::un(UnOp::BitNot, p(0))),
        Err(Reject::Invalid("E2003"))
    );
}

/// §3.4, ADR-0142: `-t` Trit; -(+1) = -1, -0 = 0, -(-1) = +1.
#[test]
fn negation_maps_plus_to_minus() {
    let neg = Expr::un(UnOp::Neg, p(0));
    assert_eq!(ty(&neg), Ok(Ty::Trit));
    assert_eq!(m().eval(&neg, &inp(PLUS, 0, 0)), Some(MINUS));
    assert_eq!(m().eval(&neg, &inp(ZERO, 0, 0)), Some(ZERO));
    assert_eq!(m().eval(&neg, &inp(MINUS, 0, 0)), Some(PLUS));
}

/// §3.3: `Trit * Trit` kapalı: (-1)(-1) = +1, (+1)(-1) = -1, 0 · x = 0.
/// `Trit ± Trit` i3: +1 + +1 = +2 (`010`), -1 - +1 = -2 (`110`).
#[test]
fn trit_products_and_sums() {
    let mul = Expr::bin(BinOp::Mul, p(0), p(1));
    assert_eq!(m().eval(&mul, &inp(MINUS, MINUS, 0)), Some(PLUS));
    assert_eq!(m().eval(&mul, &inp(PLUS, MINUS, 0)), Some(MINUS));
    assert_eq!(m().eval(&mul, &inp(ZERO, MINUS, 0)), Some(ZERO));
    let add = Expr::bin(BinOp::Add, p(0), p(1));
    assert_eq!(m().eval(&add, &inp(PLUS, PLUS, 0)), Some(0b010));
    let sub = Expr::bin(BinOp::Sub, p(0), p(1));
    assert_eq!(m().eval(&sub, &inp(MINUS, PLUS, 0)), Some(0b110));
    // Trit literaliyle: t * (-1) = -t, t + 1 i3.
    let by_lit = Expr::bin(BinOp::Mul, p(0), lit(-1));
    assert_eq!(ty(&by_lit), Ok(Ty::Trit));
    assert_eq!(m().eval(&by_lit, &inp(PLUS, 0, 0)), Some(MINUS));
    let plus_one = Expr::bin(BinOp::Add, lit(1), p(0));
    assert_eq!(m().eval(&plus_one, &inp(PLUS, 0, 0)), Some(0b010));
}

/// §3.3 ternary MAC: `t * x` iN; t = +1 → x, t = -1 → -x, t = 0 → 0.
/// Etiket A15 her `Trit * iN`'de.
#[test]
fn ternary_mac_selects_negates_or_zeroes() {
    let mac = Expr::bin(BinOp::Mul, p(0), p(2));
    assert_eq!(m().eval(&mac, &inp(PLUS, 0, 5)), Some(5));
    assert_eq!(
        m().eval(&mac, &inp(MINUS, 0, 5)),
        Some((-5i128 as u128) & 0xFF)
    );
    assert_eq!(m().eval(&mac, &inp(ZERO, 0, 5)), Some(0));
    assert_ne!(m().synth(&mac).unwrap().tags & tag::TRIT_MUL_WRAP, 0);
    assert_eq!(
        m().check(&mac, PortTy::SInt(8))
            .map(|t| t & tag::TRIT_MUL_WRAP != 0),
        Ok(true)
    );
}

/// ADR-0062 Karar 2 + ADR-0041: geniş hedefte (i12) en üstteki `t * x`
/// hedef genişliğinde yapılır; x = -128, t = -1 → +128 (12 bitte 0x080).
/// Doğal i8'de aynı çarpım A15'in okumasıdır (sarar).
#[test]
fn the_top_trit_product_is_computed_at_a_wider_target() {
    let mac = Expr::bin(BinOp::Mul, p(0), p(2));
    let v = inp(MINUS, 0, -128);
    assert_eq!(m().check(&mac, PortTy::SInt(12)).map(|_| ()), Ok(()));
    assert_eq!(m().eval_check(&mac, PortTy::SInt(12), &v), Some(0x080));
    // Trit * Trit ve Trit ± Trit itilmez: tam sayı hedefe Trit atanmaz.
    let tt = Expr::bin(BinOp::Mul, p(0), p(1));
    assert_eq!(
        m().check(&tt, PortTy::SInt(8)),
        Err(Reject::Invalid("E2003"))
    );
}

/// §3.6: `t as iN` (N >= 2) işaretle genişler; Trit'ten öteki her tipe
/// ve her tipten (literal dahil) Trit'e E2009.
#[test]
fn casts_of_trit() {
    let c = Expr::cast(p(0), PortTy::SInt(4));
    assert_eq!(ty(&c), Ok(Ty::int(true, 4)));
    assert_eq!(m().eval(&c, &inp(MINUS, 0, 0)), Some(0xF));
    assert_eq!(m().eval(&c, &inp(PLUS, 0, 0)), Some(0x1));
    for t in [
        PortTy::SInt(1),
        PortTy::UInt(2),
        PortTy::Bits(2),
        PortTy::Bool,
    ] {
        assert_eq!(
            ty(&Expr::cast(p(0), t)),
            Err(Reject::Invalid("E2009")),
            "{t:?}"
        );
    }
    for src in [p(3), p(4), p(5), lit(1)] {
        assert_eq!(
            ty(&Expr::cast(src.clone(), PortTy::Trit)),
            Err(Reject::Invalid("E2009")),
            "{src:?}"
        );
    }
}

/// ADR-0142 §4: `& | ^`, `<< >>` ve bit seçme Trit'te E2003.
#[test]
fn bit_operations_on_trit_are_e2003() {
    for op in [BinOp::And, BinOp::Or, BinOp::Xor] {
        assert_eq!(
            ty(&Expr::bin(op, p(0), p(1))),
            Err(Reject::Invalid("E2003"))
        );
    }
    for op in [BinOp::Shl, BinOp::Shr] {
        assert_eq!(
            ty(&Expr::bin(op, p(0), lit(1))),
            Err(Reject::Invalid("E2003"))
        );
    }
    assert_eq!(
        ty(&Expr::Index(Box::new(p(0)), Box::new(lit(0)))),
        Err(Reject::Invalid("E2003"))
    );
}

/// §4, ADR-0003: Trit bağlamında literal {-1, 0, +1}; dışı E2011.
#[test]
fn trit_literals() {
    for v in [-1, 0, 1] {
        assert_eq!(m().check(&lit(v), PortTy::Trit), Ok(0), "{v}");
        assert_eq!(ty(&Expr::bin(BinOp::Mul, p(0), lit(v))), Ok(Ty::Trit));
    }
    assert_eq!(
        m().check(&lit(2), PortTy::Trit),
        Err(Reject::Invalid("E2011"))
    );
    assert_eq!(
        ty(&Expr::bin(BinOp::Mul, p(0), lit(2))),
        Err(Reject::Invalid("E2011"))
    );
    assert_eq!(
        ty(&Expr::bin(BinOp::Eq, p(0), lit(-2))),
        Err(Reject::Invalid("E2011"))
    );
}

/// §3.3 karşılaştırma: aynı tip, sonuç bool; değerle eşitlik. Sıralama
/// A16'nın etiketini taşır.
#[test]
fn comparisons_of_trit() {
    let eq = Expr::bin(BinOp::Eq, p(0), lit(-1));
    assert_eq!(ty(&eq), Ok(Ty::Bool));
    assert_eq!(m().eval(&eq, &inp(MINUS, 0, 0)), Some(1));
    assert_eq!(m().eval(&eq, &inp(PLUS, 0, 0)), Some(0));
    assert_eq!(m().synth(&eq).unwrap().tags & tag::TRIT_ORDER, 0);
    let lt = Expr::bin(BinOp::Lt, p(0), p(1));
    assert_ne!(m().synth(&lt).unwrap().tags & tag::TRIT_ORDER, 0);
    assert_eq!(
        ty(&Expr::bin(BinOp::Eq, p(0), p(2))),
        Err(Reject::Invalid("?"))
    );
}

/// Şartnamenin söylemediği biçimler karşılaştırılmaz: esnek işlenenle
/// Trit çarpımı, Trit ile indeksleme, `t as Trit` (§3.6'da özdeşlik kolu
/// yok; derleyici kabul eder).
#[test]
fn what_the_spec_does_not_say_is_not_compared() {
    let flex = Expr::bin(BinOp::Mul, p(2), p(2)); // i8 * i8: [8, 16]
    assert!(matches!(
        ty(&Expr::bin(BinOp::Mul, p(0), flex)),
        Err(Reject::Unspecified(_))
    ));
    assert!(matches!(
        ty(&Expr::Index(Box::new(p(3)), Box::new(p(0)))),
        Err(Reject::Unspecified(_))
    ));
    assert!(matches!(
        ty(&Expr::cast(p(0), PortTy::Trit)),
        Err(Reject::Unspecified(_))
    ));
}
