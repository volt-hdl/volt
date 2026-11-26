//! `~` bir Trit üzerinde tanımlı değildir (#434, ADR-0142). Trit'in değer
//! kümesi {-1, 0, +1}'dir (ADR-0003); `~` ise iki bitlik kodunu (+1 = 01,
//! 0 = 00, -1 = 11, ADR-0062) bit bit tersler ve +1 için `10` kodunu, yani
//! kümenin dışındaki -2'yi verir. Önceden `~t` Trit tipliydi (type-inference.md
//! §3.4 `UnOp::BitNot => ot`) ve tanısız derleniyordu. Bakımcının kararı:
//! struct ve enum operandlarının aldığı tanı (E2003, "operator '~' is not
//! defined for ..."), yardımı Trit'in tersinin `-t` olduğunu söyler.
//!
//! `~` Trit'e her yoldan ulaşır: ifade içinde, port, register, struct alanı,
//! Trit dizisinin elemanı ve dizinin kendisi, fn gövdesi, bağlamdan Trit
//! tipini alan literal (`y = ~1`, `-(~1)`, `t * ~1`), `const` ve generic
//! `const S : Trit` parametresi (ayrıştırıcı onu literale çevirir).
//! Beklenen kodlar ve metinler ADR-0142'den ve ADR-0125'ten (ne oldu, neden,
//! ne yapılabilir) elle yazıldı.

use volt_diagnostics::{Diagnostic, NoteKind};
use volt_hir::analyze;
use volt_syntax::{parse, FileId};

fn diags(src: &str) -> Vec<Diagnostic> {
    let parsed = parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse hataları: {:?}\n{src}",
        parsed.error_codes()
    );
    analyze(&parsed.ast).diagnostics
}

fn codes(src: &str) -> Vec<String> {
    let mut c: Vec<String> = diags(src)
        .iter()
        .map(|d| d.code.as_str().to_string())
        .collect();
    c.sort();
    c
}

/// `ports` ve `body` ile tek modül.
fn module(ports: &str, body: &str) -> String {
    format!("module M {{\n{ports}\n{body}\n}}\n")
}

/// Tasarımın tek tanısı `~`'nin E2003'üdür; o tanı döner.
fn the_bit_not_error(src: &str) -> Diagnostic {
    let d = diags(src);
    let got: Vec<&str> = d.iter().map(|d| d.code.as_str()).collect();
    assert_eq!(got, ["E2003"], "tek E2003 beklenir:\n{src}\n{d:#?}");
    d.into_iter().next().expect("bir tanı")
}

fn help(d: &Diagnostic) -> &str {
    d.help.as_deref().unwrap_or("")
}

const TRIT_MESSAGE: &str = "operator '~' is not defined for 'Trit'";

/// Her yazılışta tek E2003, ileti Trit'i adlandırır.
fn assert_rejected_as_trit(src: &str) {
    let d = the_bit_not_error(src);
    assert_eq!(d.message, TRIT_MESSAGE, "{src}");
}

#[test]
fn bit_not_of_a_trit_port_is_e2003() {
    assert_rejected_as_trit(&module("    in  a : Trit\n    out y : Trit", "    y = ~a"));
}

#[test]
fn bit_not_of_a_trit_inside_an_expression_is_e2003() {
    // `(~a) * x`: Trit × i8 ternary MAC'in işleneni (type-inference.md §3.3).
    assert_rejected_as_trit(&module(
        "    in  a : Trit\n    in  x : i8\n    out y : i8",
        "    y = (~a) * x",
    ));
    // Karşılaştırmanın işleneni.
    assert_rejected_as_trit(&module(
        "    in  a : Trit\n    in  b : Trit\n    out y : bool",
        "    y = ~a == b",
    ));
}

#[test]
fn bit_not_of_a_trit_register_is_e2003() {
    assert_rejected_as_trit(&module(
        "    in  clk : clock\n    in  d : Trit\n    out y : Trit",
        "    reg r : Trit = 0\n    on clk {\n        r <= d\n    }\n    y = ~r",
    ));
    // `Delayed<Trit, 1>` register'ı (ADR-0037): değeri Trit'tir.
    assert_rejected_as_trit(&module(
        "    in  clk : clock\n    in  d : Trit\n    out y : Trit",
        "    reg r : Delayed<Trit, 1> = 0\n    on clk {\n        r <= d\n    }\n    y = ~r",
    ));
}

#[test]
fn bit_not_of_a_trit_struct_field_is_e2003() {
    let src = "struct S {
    t  : Trit
    ok : bool
}

module M {
    in  s : S
    out y : Trit
    y = ~s.t
}
";
    assert_rejected_as_trit(src);
}

#[test]
fn bit_not_of_an_element_of_a_trit_array_is_e2003() {
    assert_rejected_as_trit(&module(
        "    in  a : [Trit; 2]\n    out y : Trit",
        "    y = ~a[1]",
    ));
}

/// Paketlenmiş dizinin `~`'si her elemanın kodunu tersler (ADR-0056): her
/// eleman Trit'in `~`'sidir.
#[test]
fn bit_not_of_a_whole_trit_array_is_e2003_and_names_the_array() {
    let d = the_bit_not_error(&module(
        "    in  a : [Trit; 2]\n    out y : [Trit; 2]",
        "    y = ~a",
    ));
    assert_eq!(d.message, "operator '~' is not defined for '[Trit; 2]'");
    // `-` dizinin kendisinde tanımlı değil; yardım elemanı gösterir.
    assert!(help(&d).contains("-a[i]"), "{}", help(&d));
    // İç içe dizi de Trit tutar.
    let d = the_bit_not_error(&module(
        "    in  a : [[Trit; 2]; 2]\n    out y : [[Trit; 2]; 2]",
        "    y = ~a",
    ));
    assert_eq!(
        d.message,
        "operator '~' is not defined for '[[Trit; 2]; 2]'"
    );
}

#[test]
fn bit_not_in_a_fn_body_on_a_trit_parameter_is_e2003() {
    let src = "fn flip(x: Trit) -> Trit {
    ~x
}

module M {
    in  t : Trit
    out y : Trit
    y = flip(t)
}
";
    assert_rejected_as_trit(src);
}

/// Bağlamdan Trit tipini alan literal (type-inference.md §4): üretici `~1`'i
/// Trit genişliğinde yazar (`~2'sd1` = `10`). `~0`'ın kodu `11` (-1) bir
/// Trit değeridir, ama işlem Trit için tanımlı değil: kural değere değil
/// tipe bakar.
#[test]
fn bit_not_of_a_literal_adapted_to_trit_is_e2003() {
    for body in [
        "    y = ~1",
        "    y = ~0",
        "    y = -(~1)",
        "    y = ~1 | 0",
        "    y = 0 + ~1",
        "    y = ~1 + 0",
        "    y = ~~1",
        "    y = t * ~1",
        "    y = if t == ~1 { 1 } else { 0 }",
        "    y = if t == 0 { ~1 } else { t }",
    ] {
        // `t` yalnız onu okuyan gövdede port (okunmayan giriş W1001 olurdu).
        let ports = if body.contains(" t") {
            "    in  t : Trit\n    out y : Trit"
        } else {
            "    out y : Trit"
        };
        assert_rejected_as_trit(&module(ports, body));
    }
    assert_rejected_as_trit(&module(
        "    in  clk : clock\n    out y : Trit",
        "    reg r : Trit = ~0\n    on clk {\n        r <= r\n    }\n    y = r",
    ));
    assert_rejected_as_trit(&module("    out y : [Trit; 2]", "    y = [~1, 0]"));
    // Literal bir diziye uyarlanamaz: tek tanı atanabilirliğin E2003'ü, `~`
    // için ikincisi yok.
    let d = the_bit_not_error(&module("    out y : [Trit; 2]", "    y = ~1"));
    assert!(d.message.starts_with("type mismatch"), "{}", d.message);
}

#[test]
fn bit_not_in_a_trit_const_is_e2003() {
    let src = "const C : Trit = ~1

module M {
    out y : Trit
    y = C
}
";
    assert!(
        diags(src)
            .iter()
            .any(|d| d.code.as_str() == "E2003" && d.message == TRIT_MESSAGE),
        "{:#?}",
        diags(src)
    );
}

/// Generic `const S : Trit` parametresi ayrıştırıcıda literale dönüşür
/// (ADR-0041): `~S` örnekte `~1`'dir.
#[test]
fn bit_not_of_a_const_generic_trit_parameter_is_e2003() {
    let src = "module P<const S: Trit> {
    out y : Trit
    y = ~S
}

module M {
    out y : Trit
    let p = P<1> {}
    y = p.y
}
";
    assert!(
        diags(src)
            .iter()
            .any(|d| d.code.as_str() == "E2003" && d.message == TRIT_MESSAGE),
        "{:#?}",
        diags(src)
    );
}

/// ADR-0125: ana ileti ne oldu, etiket neyin tersleneceği, `reason` notu
/// neden, yardım ne yapılabilir (`-t`, örnekle).
#[test]
fn the_diagnostic_says_what_why_and_shows_minus_t() {
    let d = the_bit_not_error(&module("    in  t : Trit\n    out y : Trit", "    y = ~t"));
    let label = &d.primary_span().expect("birincil konum").label;
    assert_eq!(
        label,
        "'~' inverts the bits that encode a Trit, not its value"
    );
    let reason = d
        .notes
        .iter()
        .find(|n| n.kind == NoteKind::Reason)
        .expect("reason notu");
    // Neden satırı değer kümesini söyler; kodlamanın bit desenini yazmaz
    // (ADR-0139 §2: kodlama bilgisi yeni bir yerde yazılmaz).
    assert!(
        reason.text.contains("-1, 0 or +1") && !reason.text.contains("01"),
        "{}",
        reason.text
    );
    assert!(help(&d).contains("-t"), "{}", help(&d));
    assert!(help(&d).contains("+1 to -1"), "{}", help(&d));
    // Konum `~t`'nin kendisidir: `y = ~t` satırının 4. sütunu.
    let span = d.primary_span().expect("birincil konum").span;
    let src = module("    in  t : Trit\n    out y : Trit", "    y = ~t");
    assert_eq!(&src[span.start as usize..span.end as usize], "~t");
}

/// Yardımın kodu (ADR-0125 §4): `~t` yerine `-t` yazılınca tanı gider ve
/// yenisi çıkmaz; dizi için elemanların `-`'si.
#[test]
fn the_help_code_compiles_clean() {
    for (ports, body) in [
        ("    in  t : Trit\n    out y : Trit", "    y = -t"),
        (
            "    in  t : Trit\n    in  x : i8\n    out y : i8",
            "    y = (-t) * x",
        ),
        (
            "    in  a : [Trit; 2]\n    out y : [Trit; 2]",
            "    comb {\n        for i in 0..2 {\n            y[i] = -a[i]\n        }\n    }",
        ),
    ] {
        let c = codes(&module(ports, body));
        assert!(c.is_empty(), "{body}: {c:?}");
    }
}

/// `~` tanımlı olduğu tiplerde değişmez: bool, uN, iN, bits<N>, bunların
/// dizileri ve onlara uyarlanan literal.
#[test]
fn bit_not_of_other_types_is_unchanged() {
    for (ty, value) in [
        ("bool", "~a"),
        ("u8", "~a"),
        ("i8", "~a"),
        ("bits<4>", "~a"),
        ("[u8; 2]", "~a"),
        ("[bool; 3]", "~a"),
    ] {
        let c = codes(&module(
            &format!("    in  a : {ty}\n    out y : {ty}"),
            &format!("    y = {value}"),
        ));
        assert!(c.is_empty(), "{ty}: {c:?}");
    }
    for (ty, body) in [
        ("u8", "    y = ~1"),
        ("i8", "    y = -(~1)"),
        ("u4", "    y = ~1 | 0"),
    ] {
        let c = codes(&module(&format!("    out y : {ty}"), body));
        assert!(c.is_empty(), "{ty} {body}: {c:?}");
    }
}

/// Öteki bit işlemleri Trit'te bugün de reddedilir (E2003); bu PR onları
/// değiştirmez, tabloyu sınar. Volt'ta indirgeme işleci yoktur (`&a`
/// ayrıştırılmaz: E0001); bit seçme de Trit'te E2003'tür.
#[test]
fn the_other_bit_operations_on_a_trit_are_e2003() {
    for (body, message) in [
        (
            "    y = a & b",
            "bitwise operator is not defined for 'Trit' and 'Trit'",
        ),
        (
            "    y = a | b",
            "bitwise operator is not defined for 'Trit' and 'Trit'",
        ),
        (
            "    y = a ^ b",
            "bitwise operator is not defined for 'Trit' and 'Trit'",
        ),
        (
            "    y = a & 1",
            "bitwise operator is not defined for 'Trit' and 'integer literal'",
        ),
        ("    y = a << 1", "type 'Trit' cannot be shifted"),
        ("    y = a >> 1", "type 'Trit' cannot be shifted"),
    ] {
        // `b` yalnız ikili işlemlerde okunur; öbürlerinde portu yoktur
        // (okunmayan giriş W1001 olurdu).
        let ports = if body.contains('b') {
            "    in  a : Trit\n    in  b : Trit\n    out y : Trit"
        } else {
            "    in  a : Trit\n    out y : Trit"
        };
        let d = diags(&module(ports, body));
        let got: Vec<&str> = d.iter().map(|d| d.code.as_str()).collect();
        assert_eq!(got, ["E2003"], "{body}: {d:#?}");
        assert_eq!(d[0].message, message, "{body}");
    }
    let d = diags(&module(
        "    in  a : Trit\n    out y : bool",
        "    y = a[1]",
    ));
    assert_eq!(
        d.iter().map(|d| d.code.as_str()).collect::<Vec<_>>(),
        ["E2003"],
        "{d:#?}"
    );
    let parsed = parse(
        FileId(0),
        &module("    in  a : Trit\n    out y : bool", "    y = &a"),
    );
    assert_eq!(parsed.error_codes(), ["E0001"]);
}
