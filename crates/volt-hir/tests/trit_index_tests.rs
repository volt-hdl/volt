//! Bir Trit indeks olamaz (#465, ADR-0146). Trit'in değer kümesi
//! {-1, 0, +1}'dir (ADR-0003) ve -1 bir bit ya da eleman konumu değildir;
//! indeks ise Trit'i kodlayan iki biti (+1 = 01, 0 = 00, -1 = 11,
//! ADR-0062) işaretsiz bir sayı olarak okuyordu. Önceden `x[t]` tanısız
//! derleniyordu ve t = -1 iken `x[3]`'ü okuyordu (sessiz yanlış).
//! Bakımcının kararı: indeksin tipi Trit tutuyorsa (ADR-0142'nin
//! `holds_trit`'i: Trit ya da Trit tutan dizi) E2003; öteki her indeks
//! tipi (uN, iN, esnek, literal, bits<N>, bool, enum, `[u1; N]`) bugünkü
//! gibi kalır. Kural değere değil tipe bakar.
//!
//! Kural her indeks yolunda aynıdır: bit seçimi, dizi elemanı (port, const,
//! register), aralık seçiminin sınırları, parça seçiminin başlangıcı
//! (`+:`, `-:`), atamanın sol tarafı (`on` ve `comb`), fn gövdesi ve
//! kontrat. Beklenen kodlar ve metinler ADR-0146'dan ve ADR-0125'ten (ne
//! oldu, neden, ne yapılabilir) elle yazıldı.

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

/// Tasarımın tek tanısı indeksin E2003'üdür; o tanı döner.
fn the_index_error(src: &str) -> Diagnostic {
    let d = diags(src);
    let got: Vec<&str> = d.iter().map(|d| d.code.as_str()).collect();
    assert_eq!(got, ["E2003"], "tek E2003 beklenir:\n{src}\n{d:#?}");
    d.into_iter().next().expect("bir tanı")
}

fn help(d: &Diagnostic) -> &str {
    d.help.as_deref().unwrap_or("")
}

const TRIT_MESSAGE: &str = "an index must not be a Trit, found 'Trit'";

/// Tek E2003, ileti Trit'i adlandırır ve konum indeksin kendisidir.
fn assert_rejected_index(src: &str, index_text: &str) {
    let d = the_index_error(src);
    assert_eq!(d.message, TRIT_MESSAGE, "{src}");
    let span = d.primary_span().expect("birincil konum").span;
    assert_eq!(
        &src[span.start as usize..span.end as usize],
        index_text,
        "{src}"
    );
}

const T_X: &str = "    in  t : Trit\n    in  x : u8";

#[test]
fn a_trit_bit_select_index_is_e2003() {
    assert_rejected_index(
        &module(&format!("{T_X}\n    out y : bool"), "    y = x[t]"),
        "t",
    );
    // bits<N> ve iN tabanı da aynı kural.
    assert_rejected_index(
        &module(
            "    in  t : Trit\n    in  x : bits<8>\n    out y : bool",
            "    y = x[t]",
        ),
        "t",
    );
    assert_rejected_index(
        &module(
            "    in  t : Trit\n    in  x : i8\n    out y : bool",
            "    y = x[t]",
        ),
        "t",
    );
}

#[test]
fn a_trit_index_of_an_array_port_is_e2003() {
    assert_rejected_index(
        &module(
            "    in  t : Trit\n    in  arr : [u8; 4]\n    out y : u8",
            "    y = arr[t]",
        ),
        "t",
    );
}

#[test]
fn a_trit_index_of_a_const_array_is_e2003() {
    let src = "const TABLE : [u8; 4] = [10, 20, 30, 40]

module M {
    in  t : Trit
    out y : u8
    y = TABLE[t]
}
";
    assert_rejected_index(src, "t");
}

#[test]
fn a_trit_index_of_a_register_array_is_e2003() {
    assert_rejected_index(
        &module(
            "    in  clk : clock\n    in  t : Trit\n    in  d : u8\n    out y : u8",
            "    reg ra : [u8; 4] = [0, 0, 0, 0]\n    on clk {\n        ra[0] <= d\n    }\n    y = ra[t]",
        ),
        "t",
    );
}

#[test]
fn a_trit_part_select_start_is_e2003() {
    for body in ["    y = x[t +: 2]", "    y = x[t -: 2]"] {
        assert_rejected_index(&module(&format!("{T_X}\n    out y : bits<2>"), body), "t");
    }
}

/// Aralık seçiminin sınırları derleme zamanı sabitidir (E2008); Trit
/// tipli bir `const` sınır olunca tip Trit'tir: değeri doğru olsa da kural
/// tipe bakar.
#[test]
fn a_trit_range_bound_is_e2003() {
    for (body, bound) in [("    y = x[K : 0]", "K"), ("    y = x[1 : K]", "K")] {
        let src = format!(
            "const K : Trit = 1\n\nmodule M {{\n    in  x : u8\n    out y : bits<2>\n{body}\n}}\n"
        );
        assert_rejected_index(&src, bound);
    }
}

/// Reddedilen seçimin tipi hatadır: hedefin tipi seçiminkine uymasa da
/// ikinci bir tanı çıkmaz (bit seçimi bool, parça ve aralık bits<N> olurdu).
/// İki sınır da Trit'se ikisi de bildirilir.
#[test]
fn a_rejected_selection_has_the_error_type() {
    for (out, body) in [
        ("u8", "    y = x[t]"),
        ("bool", "    y = x[t +: 2]"),
        ("bool", "    y = x[t -: 2]"),
    ] {
        assert_rejected_index(&module(&format!("{T_X}\n    out y : {out}"), body), "t");
    }
    for body in ["    y = x[K : 0]", "    y = x[1 : K]"] {
        let src = format!(
            "const K : Trit = 1\n\nmodule M {{\n    in  x : u8\n    out y : bool\n{body}\n}}\n"
        );
        assert_rejected_index(&src, "K");
    }
    let src =
        "const K : Trit = 1\n\nmodule M {\n    in  x : u8\n    out y : bool\n    y = x[K : K]\n}\n";
    let d = diags(src);
    let got: Vec<(&str, &str)> = d
        .iter()
        .map(|d| (d.code.as_str(), d.message.as_str()))
        .collect();
    assert_eq!(
        got,
        [("E2003", TRIT_MESSAGE), ("E2003", TRIT_MESSAGE)],
        "{d:#?}"
    );
}

/// Atamanın sol tarafı aynı kuraldan geçer: `on` bloğunda dizi ve vektör
/// register'ı, `comb` bloğunda vektör.
#[test]
fn a_trit_index_on_the_left_of_an_assignment_is_e2003() {
    assert_rejected_index(
        &module(
            "    in  clk : clock\n    in  t : Trit\n    out y : u8",
            "    reg wa : [u8; 4] = [0, 0, 0, 0]\n    on clk {\n        wa[t] <= 99\n    }\n    y = wa[0]",
        ),
        "t",
    );
    assert_rejected_index(
        &module(
            "    in  clk : clock\n    in  t : Trit\n    out y : u8",
            "    reg r : u8 = 0\n    on clk {\n        r[t] <= true\n    }\n    y = r",
        ),
        "t",
    );
    assert_rejected_index(
        &module(
            "    in  clk : clock\n    in  t : Trit\n    out y : u8",
            "    reg r : u8 = 0\n    on clk {\n        r[t +: 2] <= 3\n    }\n    y = r",
        ),
        "t",
    );
    assert_rejected_index(
        &module(
            "    in  t : Trit\n    out y : u8",
            "    comb {\n        y = 0\n        y[t] = true\n    }",
        ),
        "t",
    );
}

#[test]
fn a_trit_index_in_a_fn_body_is_e2003() {
    let src = "fn pick(x: u8, t: Trit) -> bool {
    x[t]
}

module M {
    in  t : Trit
    in  x : u8
    out y : bool
    y = pick(x, t)
}
";
    assert_rejected_index(src, "t");
}

#[test]
fn a_trit_index_in_a_contract_is_e2003() {
    assert_rejected_index(
        &module(
            &format!("{T_X}\n    in  b : bool\n    out y : bool"),
            "    assert: x[t] == b\n    y = b",
        ),
        "t",
    );
}

/// Kural tipe bakar: tipi Trit olan her ifade indeks olamaz (`-t`, Trit
/// çarpımı, Trit dallı `if`, Trit dizisinin elemanı, Trit struct alanı).
#[test]
fn every_trit_typed_index_expression_is_e2003() {
    for (ports, body, index) in [
        (T_X, "    y = x[-t]", "-t"),
        (
            "    in  t : Trit\n    in  u : Trit\n    in  x : u8",
            "    y = x[t * u]",
            "t * u",
        ),
        (
            "    in  t : Trit\n    in  c : bool\n    in  x : u8",
            "    y = x[if c { t } else { -t }]",
            "if c { t } else { -t }",
        ),
        (
            "    in  ws : [Trit; 2]\n    in  x : u8",
            "    y = x[ws[1]]",
            "ws[1]",
        ),
    ] {
        assert_rejected_index(&module(&format!("{ports}\n    out y : bool"), body), index);
    }
    let src = "struct W {
    t  : Trit
    ok : bool
}

module M {
    in  w : W
    in  x : u8
    out y : bool
    y = x[w.t] && w.ok
}
";
    assert_rejected_index(src, "w.t");
}

/// Trit tutan dizi de (iç içe dahil) indeks olamaz; ileti tipi adlandırır.
#[test]
fn an_array_holding_trits_as_an_index_is_e2003() {
    for ty in ["[Trit; 2]", "[[Trit; 2]; 2]"] {
        let d = the_index_error(&module(
            &format!("    in  a : {ty}\n    in  x : u8\n    out y : bool"),
            "    y = x[a]",
        ));
        assert_eq!(
            d.message,
            format!("an index must not be a Trit, found '{ty}'")
        );
    }
}

/// ADR-0125: ana ileti ne oldu, etiket neyin okunacağı, `reason` notu
/// neden, yardım ne yapılabilir (Trit'i açıkça işaretsiz indekse çevirmek).
#[test]
fn the_diagnostic_says_what_why_and_shows_the_conversion() {
    let d = the_index_error(&module(&format!("{T_X}\n    out y : bool"), "    y = x[t]"));
    let label = &d.primary_span().expect("birincil konum").label;
    assert_eq!(
        label,
        "the index would read the bits that encode the Trit, not its value"
    );
    let reason = d
        .notes
        .iter()
        .find(|n| n.kind == NoteKind::Reason)
        .expect("reason notu");
    // Neden satırı değer kümesini söyler; kodlamanın bit desenini yazmaz
    // (ADR-0139 §2).
    assert!(
        reason.text.contains("-1, 0 or +1") && !reason.text.contains("11"),
        "{}",
        reason.text
    );
    assert_eq!(
        help(&d),
        "convert the Trit to an unsigned index explicitly: x[(t as i3 + 1) as u3] \
         reads x[0], x[1] and x[2] for t = -1, 0 and +1"
    );
}

/// Yardımın kodu (ADR-0125 §4): indeksin yerine konunca tanı gider ve
/// yenisi çıkmaz; bit seçimi, dizi elemanı, parça seçimi ve sol taraf.
/// Tabanların adresi 3 bittir, `u3` indeksle aynı.
#[test]
fn the_help_code_compiles_clean() {
    const CONV: &str = "(t as i3 + 1) as u3";
    for (ports, body) in [
        (format!("{T_X}\n    out y : bool"), format!("    y = x[{CONV}]")),
        (
            "    in  t : Trit\n    in  arr : [u8; 8]\n    out y : u8".to_string(),
            format!("    y = arr[{CONV}]"),
        ),
        (
            format!("{T_X}\n    out y : bits<2>"),
            format!("    y = x[{CONV} +: 2]"),
        ),
        (
            "    in  clk : clock\n    in  t : Trit\n    out y : u8".to_string(),
            format!(
                "    reg wa : [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 0]\n    on clk {{\n        wa[{CONV}] <= 99\n    }}\n    y = wa[0]"
            ),
        ),
    ] {
        let c = codes(&module(&ports, &body));
        assert!(c.is_empty(), "{body}: {c:?}");
    }
}

/// Öteki indeks tipleri değişmez (bakımcının kararı (c)): uN, esnek,
/// literal, bits<N>, bool, enum ve `[u1; N]` bugünkü gibi kabul edilir.
/// `iN` artık bu listede değildir: işaretli indeks E2003'tür (#475,
/// ADR-0147; `signed_index_tests`).
#[test]
fn other_index_types_are_unchanged() {
    for (ty, index) in [
        ("u3", "i"),
        ("u8", "i"),
        ("bits<3>", "i"),
        ("bool", "i"),
        ("[u1; 2]", "i"),
        ("u2", "i + i"),
    ] {
        let c = codes(&module(
            &format!("    in  i : {ty}\n    in  x : u8\n    out y : bool"),
            &format!("    y = x[{index}]"),
        ));
        assert!(c.is_empty(), "{ty} {index}: {c:?}");
        let c = codes(&module(
            &format!("    in  i : {ty}\n    in  x : u8\n    out y : bits<2>"),
            &format!("    y = x[{index} +: 2]"),
        ));
        assert!(c.is_empty(), "{ty} {index} +: 2: {c:?}");
    }
    let c = codes(&module("    in  x : u8\n    out y : bool", "    y = x[3]"));
    assert!(c.is_empty(), "literal: {c:?}");
    let src = "enum Sel {
    A
    B
}

module M {
    in  s : Sel
    in  x : u8
    out y : bool
    y = x[s]
}
";
    assert!(codes(src).is_empty(), "enum: {:?}", codes(src));
    // Sabit aralık sınırı ve bits<N> sol taraf indeksi.
    let c = codes(&module(
        "    in  x : u8\n    out y : bits<2>",
        "    y = x[2 : 1]",
    ));
    assert!(c.is_empty(), "range: {c:?}");
    let c = codes(&module(
        "    in  clk : clock\n    in  a : bits<2>\n    in  d : u8\n    out y : u8",
        "    reg wa : [u8; 4] = [0, 0, 0, 0]\n    on clk {\n        wa[a] <= d\n    }\n    y = wa[0]",
    ));
    assert!(c.is_empty(), "bits<2> lvalue: {c:?}");
}

/// Trit bir dizinin elemanı olarak indekslenebilir; kural tabana değil
/// indekse bakar. Bit seçme Trit tabanında bugün de E2003'tür (ADR-0142
/// §4), ileti farklıdır.
#[test]
fn a_trit_base_is_not_this_rule() {
    let c = codes(&module(
        "    in  ws : [Trit; 4]\n    in  i : u2\n    out y : Trit",
        "    y = ws[i]",
    ));
    assert!(c.is_empty(), "{c:?}");
    let d = the_index_error(&module(
        "    in  a : Trit\n    in  i : u1\n    out y : bool",
        "    y = a[i]",
    ));
    assert_ne!(d.message, TRIT_MESSAGE);
}

/// Generic `const S : Trit` parametresi tip denetiminde Trit'tir (#468,
/// bakımcı kararı): ayrıştırıcı onu literale çevirse de (ADR-0041) literal
/// parametrenin bildirilen tipini taşır, kural kendiliğinden uygulanır
/// (ADR-0146 Sınırlar). `x[S]` ve `x[S +: 2]` Trit indeks, `x[S + 1 : S]`'in
/// üst sınırı `S + 1` i3 (işaretli, ADR-0147), alt sınırı Trit: dört E2003.
/// Bu test önce bugünkü kabulü sabitliyordu
/// (`a_const_generic_trit_index_is_a_literal_and_is_accepted`).
#[test]
fn a_const_generic_trit_index_is_e2003() {
    let src = "module P<const S: Trit> {
    in  x : u8
    out a : bool
    out b : bits<2>
    out c : bits<2>
    a = x[S]
    b = x[S + 1 : S]
    c = x[S +: 2]
}

module M {
    in  x : u8
    out a : bool
    out b : bits<2>
    out c : bits<2>
    let p = P<1> { x: x }
    a = p.a
    b = p.b
    c = p.c
}
";
    assert_eq!(codes(src), ["E2003"; 4], "{:?}", codes(src));
}
