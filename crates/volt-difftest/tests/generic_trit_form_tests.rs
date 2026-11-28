//! Fark testinin t biçimi (#468, #471, ADR-0148): ifadenin okuduğu ilk Trit
//! portu bir generic modülün `const S : Trit` parametresi olur, modülün S =
//! +1, 0 ve -1 örneklerinden portun değerine göre biri seçilir. Beklenen
//! değer a biçimininkidir (bakımcının #468 kararı: S aynı yerdeki Trit
//! sinyalinin değerini alır).

use volt_difftest::ast::{BinOp, Expr, PortTy, UnOp};
use volt_difftest::harness::{build, case_of_generic_file, generic_module_name, Case, Variant};
use volt_difftest::model::Model;
use volt_difftest::rng::Rng;

fn p(k: usize) -> Expr {
    Expr::Port(k)
}

/// p1 Trit, p12 i8 (adı p1 ile başlar), p2 Trit, p3 i8; arası u4.
fn ports() -> Vec<PortTy> {
    let mut ports = vec![PortTy::UInt(4); 13];
    ports[1] = PortTy::Trit;
    ports[2] = PortTy::Trit;
    ports[3] = PortTy::SInt(8);
    ports[12] = PortTy::SInt(8);
    ports
}

fn case(id: usize, e: Expr) -> Case {
    Case::new(id, e, &ports(), &mut Rng::new(1))
}

#[test]
fn the_first_trit_port_read_becomes_the_generic_constant() {
    // `-p2 * p12 + p1 * p3`: ilk okunan Trit portu p2.
    let e = Expr::bin(
        BinOp::Add,
        Expr::bin(BinOp::Mul, Expr::un(UnOp::Neg, p(2)), p(12)),
        Expr::bin(BinOp::Mul, p(1), p(3)),
    );
    let c = case(7, e);
    assert!(c.is_valid(), "{:?}", c.kind);
    assert_eq!(c.generic_trit, Some(2));
    assert!(c.variants.contains(&Variant::T));
    // Trit okumayan ifadede t biçimi yok.
    let c = case(8, Expr::bin(BinOp::Add, p(3), p(12)));
    assert_eq!(c.generic_trit, None);
    assert!(!c.variants.contains(&Variant::T));
}

#[test]
fn the_module_has_the_generic_and_three_instances_selected_by_the_port() {
    // `p1 * p12`: S yalnız tam ad p1'in yerine geçer, p12 kalır.
    let c = case(5, Expr::bin(BinOp::Mul, p(1), p(12)));
    let built = build(std::slice::from_ref(&c), &ports());
    let text = built.text;
    for line in [
        "module T5t<const S : Trit> {",
        "    in p12 : i8",
        "    out o : i8",
        "    o = S * p12",
        "    let u5tp = T5t<1> { p12: p12 }",
        "    let u5tz = T5t<0> { p12: p12 }",
        "    let u5tn = T5t<-1> { p12: p12 }",
        "    o5t = if p1 == 1 { u5tp.o } else if p1 == -1 { u5tn.o } else { u5tz.o }",
    ] {
        assert!(text.lines().any(|l| l == line), "satır yok: {line}\n{text}");
    }
    // Generic modülde Trit portu yok.
    let generic = &text[text.find("module T5t").expect("generic modül")..];
    assert!(!generic.contains("in p1 "), "{generic}");
    // Satırlar t biçimine ait.
    let owned = built
        .line_owner
        .iter()
        .filter(|o| **o == Some((5, Variant::T)))
        .count();
    // Çıkışın bildirimi, modülün içinde dört satır (üç örnek ve seçim) ve
    // generic modülün beş satırı.
    assert_eq!(owned, 1 + 4 + 5, "{text}");
    assert!(built
        .outputs
        .iter()
        .any(|o| o.name == "o5t" && o.variant == Variant::T && o.ty == PortTy::SInt(8)));
}

#[test]
fn the_expected_value_is_the_inline_value() {
    let c = case(3, Expr::bin(BinOp::Mul, Expr::un(UnOp::Neg, p(1)), p(3)));
    let ports = ports();
    let model = Model { ports: &ports };
    // p1 = +1 (01), p3 = -128 (0x80): -p1 * p3 = +128 i8'de sarar, -128
    // (ADR-0145 Karar 8); örüntü 0x80.
    let mut inp = vec![0u128; 13];
    inp[1] = 0b01;
    inp[3] = 0x80;
    assert_eq!(c.expected(Variant::T, &model, &inp), Some(0x80));
    assert_eq!(
        c.expected(Variant::T, &model, &inp),
        c.expected(Variant::A, &model, &inp)
    );
    let text = c.variant_text(Variant::T);
    assert!(
        text.starts_with("module T3t<const S : Trit> { out o : i8  o = -S * p3 }"),
        "{text}"
    );
    assert!(text.contains("o3t = if p1 == 1"), "{text}");
}

#[test]
fn a_generic_module_file_names_its_case() {
    assert_eq!(generic_module_name(12), "T12t");
    assert_eq!(case_of_generic_file("T12t_1.sv"), Some(12));
    assert_eq!(case_of_generic_file("T12t_n1.sv"), Some(12));
    assert_eq!(case_of_generic_file("T0t_0.sv"), Some(0));
    assert_eq!(case_of_generic_file("Dt.sv"), None);
    assert_eq!(case_of_generic_file("Tt_1.sv"), None);
    assert_eq!(case_of_generic_file("T1x_0.sv"), None);
    assert_eq!(case_of_generic_file("T1t_0.v"), None);
    assert_eq!(case_of_generic_file("R12r_0_3.sv"), None);
}

#[test]
fn every_variant_has_a_distinct_letter() {
    let mut letters: Vec<char> = Variant::ALL.iter().map(|v| v.letter()).collect();
    letters.sort_unstable();
    letters.dedup();
    assert_eq!(letters.len(), Variant::ALL.len());
    assert_eq!(Variant::from_letter('t'), Some(Variant::T));
    assert_eq!(Variant::T.what(), "a Trit port as a generic const S : Trit");
}
