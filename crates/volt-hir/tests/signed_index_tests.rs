//! Bir indeks işaretsiz olmalı (#475, ADR-0147). SystemVerilog indeksi
//! işaretsiz bir sayı olarak okur (IEEE 1800 §11.5.1); önceden tipi
//! işaretli tamsayı olan bir indeks tanısız derleniyordu ve i : i3 = -1
//! için `x[i]` `x[7]`'yi, `TABLE[i]` ve `arr[i]` 7. elemanı okuyordu
//! (sessiz yanlış). Bakımcının kararı (A): tipi işaretli tamsayı olan
//! indeks (`iN` ve esnek `iN`) E2003; kural değere değil tipe bakar, değeri
//! hep pozitif olan bir `iN` indeks de reddedilir. Trit tutan indeksin
//! kuralı (ADR-0146) aynı yerdedir ve değişmez.
//!
//! Kural her indeks yolunda aynıdır: bit seçimi, dizi elemanı (port, const,
//! register), aralık seçiminin sınırları, parça seçiminin başlangıcı
//! (`+:`, `-:`), atamanın sol tarafı (`on` ve `comb`), fn gövdesi ve
//! kontrat. Yardım indeksi tabanın adres genişliğinde işaretsize çevirir:
//! `x : u8` için `x[i as u3]`, `[u8; 4]` için `arr[i as u2]`. Beklenen
//! kodlar, metinler ve genişlikler ADR-0147'den ve ADR-0125'ten (ne oldu,
//! neden, ne yapılabilir) elle yazıldı; adres genişliği konum sayısını
//! işaretsiz tutan en dar genişliktir, en az 1.

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

fn message(ty: &str) -> String {
    format!("an index must be unsigned, found '{ty}'")
}

/// Tek E2003, ileti indeksin tipini adlandırır ve konum indeksin kendisidir.
fn assert_rejected_index(src: &str, index_text: &str, ty: &str) {
    let d = the_index_error(src);
    assert_eq!(d.message, message(ty), "{src}");
    let span = d.primary_span().expect("birincil konum").span;
    assert_eq!(
        &src[span.start as usize..span.end as usize],
        index_text,
        "{src}"
    );
}

const I_X: &str = "    in  i : i3\n    in  x : u8";

/// Yardımın beklenen metni: `code` tabanın biçimiyle dönüşüm, `base` tabanın
/// adı ve tipi, `width` adres genişliği.
fn expected_help(code: &str, base: &str, width: u32) -> String {
    let max = (1u64 << width) - 1;
    format!(
        "convert the index explicitly to an unsigned number of the address width: \
         {code} for {base}; a negative value then becomes a large index, -1 as u{width} is {max}"
    )
}

#[test]
fn a_signed_bit_select_index_is_e2003() {
    assert_rejected_index(
        &module(&format!("{I_X}\n    out y : bool"), "    y = x[i]"),
        "i",
        "i3",
    );
    // bits<N> ve iN tabanı da aynı kural; indeksin genişliği önemsiz.
    for (base, index) in [("bits<8>", "i3"), ("i8", "i3"), ("u8", "i8"), ("u8", "i1")] {
        assert_rejected_index(
            &module(
                &format!("    in  i : {index}\n    in  x : {base}\n    out y : bool"),
                "    y = x[i]",
            ),
            "i",
            index,
        );
    }
}

#[test]
fn a_signed_index_of_an_array_port_is_e2003() {
    assert_rejected_index(
        &module(
            "    in  i : i2\n    in  arr : [u8; 4]\n    out y : u8",
            "    y = arr[i]",
        ),
        "i",
        "i2",
    );
}

#[test]
fn a_signed_index_of_a_const_array_is_e2003() {
    let src = "const TABLE : [u8; 4] = [10, 20, 30, 40]

module M {
    in  i : i2
    out y : u8
    y = TABLE[i]
}
";
    assert_rejected_index(src, "i", "i2");
}

#[test]
fn a_signed_index_of_a_register_array_is_e2003() {
    assert_rejected_index(
        &module(
            "    in  clk : clock\n    in  i : i2\n    in  d : u8\n    out y : u8",
            "    reg ra : [u8; 4] = [0, 0, 0, 0]\n    on clk {\n        ra[0] <= d\n    }\n    y = ra[i]",
        ),
        "i",
        "i2",
    );
}

#[test]
fn a_signed_part_select_start_is_e2003() {
    for body in ["    y = x[i +: 2]", "    y = x[i -: 2]"] {
        assert_rejected_index(
            &module(&format!("{I_X}\n    out y : bits<2>"), body),
            "i",
            "i3",
        );
    }
}

/// Aralık seçiminin sınırları derleme zamanı sabitidir (E2008); işaretli
/// tipte bir `const` sınır olunca tip işaretlidir: değeri doğru olsa da
/// kural tipe bakar.
#[test]
fn a_signed_range_bound_is_e2003() {
    for (body, bound) in [("    y = x[K : 0]", "K"), ("    y = x[1 : K]", "K")] {
        let src = format!(
            "const K : i8 = 1\n\nmodule M {{\n    in  x : u8\n    out y : bits<2>\n{body}\n}}\n"
        );
        assert_rejected_index(&src, bound, "i8");
    }
}

/// İşaretli bir `const` indeks de (değeri 3, konumu doğru okunurdu)
/// reddedilir; negatif değerli sabit de E2003 alır, E2006 değil: kural
/// sınır denetiminden önce tipe bakar.
#[test]
fn a_signed_constant_index_is_e2003() {
    for (k, ty) in [
        ("const K : i8 = 3", "i8"),
        ("const K : i8 = -1", "i8"),
        ("const K : i4 = 7", "i4"),
    ] {
        let src =
            format!("{k}\n\nmodule M {{\n    in  x : u8\n    out y : bool\n    y = x[K]\n}}\n");
        assert_rejected_index(&src, "K", ty);
    }
    // Sonekli literal işaretli bir tiptir; soneksiz literal değildir.
    assert_rejected_index(
        &module("    in  x : u8\n    out y : bool", "    y = x[3i8]"),
        "3i8",
        "i8",
    );
}

/// Reddedilen seçimin tipi hatadır: hedefin tipi seçiminkine uymasa da
/// ikinci bir tanı çıkmaz. İki sınır da işaretliyse ikisi de bildirilir;
/// Trit ile işaretli sınır kendi iletilerini alır.
#[test]
fn a_rejected_selection_has_the_error_type() {
    for (out, body) in [
        ("u8", "    y = x[i]"),
        ("bool", "    y = x[i +: 2]"),
        ("bool", "    y = x[i -: 2]"),
    ] {
        assert_rejected_index(
            &module(&format!("{I_X}\n    out y : {out}"), body),
            "i",
            "i3",
        );
    }
    let src =
        "const K : i8 = 1\n\nmodule M {\n    in  x : u8\n    out y : bool\n    y = x[K : K]\n}\n";
    let d = diags(src);
    let got: Vec<(&str, &str)> = d
        .iter()
        .map(|d| (d.code.as_str(), d.message.as_str()))
        .collect();
    let m = message("i8");
    assert_eq!(
        got,
        [("E2003", m.as_str()), ("E2003", m.as_str())],
        "{d:#?}"
    );
    let src = "const K : i8 = 1\nconst T : Trit = 0\n\nmodule M {\n    in  x : u8\n    out y : bits<2>\n    y = x[K : T]\n}\n";
    let d = diags(src);
    let got: Vec<(&str, &str)> = d
        .iter()
        .map(|d| (d.code.as_str(), d.message.as_str()))
        .collect();
    assert_eq!(
        got,
        [
            ("E2003", m.as_str()),
            ("E2003", "an index must not be a Trit, found 'Trit'")
        ],
        "{d:#?}"
    );
}

/// Atamanın sol tarafı aynı kuraldan geçer: `on` bloğunda dizi ve vektör
/// register'ı, `comb` bloğunda vektör.
#[test]
fn a_signed_index_on_the_left_of_an_assignment_is_e2003() {
    assert_rejected_index(
        &module(
            "    in  clk : clock\n    in  i : i2\n    out y : u8",
            "    reg wa : [u8; 4] = [0, 0, 0, 0]\n    on clk {\n        wa[i] <= 99\n    }\n    y = wa[0]",
        ),
        "i",
        "i2",
    );
    assert_rejected_index(
        &module(
            "    in  clk : clock\n    in  i : i3\n    out y : u8",
            "    reg r : u8 = 0\n    on clk {\n        r[i] <= true\n    }\n    y = r",
        ),
        "i",
        "i3",
    );
    assert_rejected_index(
        &module(
            "    in  clk : clock\n    in  i : i3\n    out y : u8",
            "    reg r : u8 = 0\n    on clk {\n        r[i +: 2] <= 3\n    }\n    y = r",
        ),
        "i",
        "i3",
    );
    assert_rejected_index(
        &module(
            "    in  i : i3\n    out y : u8",
            "    comb {\n        y = 0\n        y[i] = true\n    }",
        ),
        "i",
        "i3",
    );
}

#[test]
fn a_signed_index_in_a_fn_body_is_e2003() {
    let src = "fn pick(x: u8, i: i3) -> bool {
    x[i]
}

module M {
    in  i : i3
    in  x : u8
    out y : bool
    y = pick(x, i)
}
";
    assert_rejected_index(src, "i", "i3");
}

#[test]
fn a_signed_index_in_a_contract_is_e2003() {
    assert_rejected_index(
        &module(
            &format!("{I_X}\n    in  b : bool\n    out y : bool"),
            "    assert: x[i] == b\n    y = b",
        ),
        "i",
        "i3",
    );
}

/// Kural tipe bakar: tipi işaretli tamsayı olan her ifade indeks olamaz.
/// Esnek işaretli tipler (`a + b`, `a - b`, `a * b`, `a + 1`, `1 - a`;
/// type-inference.md §3.3) ve somut olanlar (`-a`, `a >> 1`, `a & b`,
/// `if`, `t as i3`, `t + t`, `t * a`, `a as i3`, işaretli register).
#[test]
fn every_signed_typed_index_expression_is_e2003() {
    // Her biçim yalnız kullandığı portları alır (kullanılmayan port W1001).
    const AB: &str = "    in  a : i8\n    in  b : i8\n    in  x : u8";
    const A: &str = "    in  a : i8\n    in  x : u8";
    const T: &str = "    in  t : Trit\n    in  x : u8";
    const ABC: &str = "    in  a : i8\n    in  b : i8\n    in  c : bool\n    in  x : u8";
    const AT: &str = "    in  a : i8\n    in  t : Trit\n    in  x : u8";
    for (ports, index, ty) in [
        (AB, "a + b", "i9"),
        (AB, "a - b", "i9"),
        (AB, "a * b", "i16"),
        (A, "a + 1", "i9"),
        (A, "1 - a", "i9"),
        (A, "-a", "i9"),
        (A, "a >> 1", "i8"),
        (AB, "a & b", "i8"),
        (ABC, "if c { a } else { b }", "i8"),
        (T, "t as i3", "i3"),
        (T, "t + t", "i3"),
        (AT, "t * a", "i8"),
    ] {
        assert_rejected_index(
            &module(
                &format!("{ports}\n    out y : bool"),
                &format!("    y = x[{index}]"),
            ),
            index,
            ty,
        );
    }
    // `a as i3` daraltmadır, W2010 da gelir; indeksin tanısı E2003.
    let d = diags(&module(
        &format!("{A}\n    out y : bool"),
        "    y = x[a as i3]",
    ));
    let got: Vec<(&str, &str)> = d
        .iter()
        .map(|d| (d.code.as_str(), d.message.as_str()))
        .filter(|(c, _)| *c == "E2003")
        .collect();
    assert_eq!(got, [("E2003", message("i3").as_str())], "{d:#?}");
    // İşaretli register okunuşu.
    assert_rejected_index(
        &module(
            "    in  clk : clock\n    in  a : i3\n    in  x : u8\n    out y : bool",
            "    reg d : i3 = 0\n    on clk {\n        d <= a\n    }\n    y = x[d]",
        ),
        "d",
        "i3",
    );
}

/// ADR-0125: ana ileti ne oldu, etiket negatif değerin ne seçeceği,
/// `reason` notu neden, yardım ne yapılabilir (adres genişliğinde
/// işaretsize açık dönüşüm, negatif değerin ne olacağı).
#[test]
fn the_diagnostic_says_what_why_and_shows_the_conversion() {
    let d = the_index_error(&module(&format!("{I_X}\n    out y : bool"), "    y = x[i]"));
    let label = &d.primary_span().expect("birincil konum").label;
    assert_eq!(
        label,
        "a negative value of this index would select a high bit or element"
    );
    let reason = d
        .notes
        .iter()
        .find(|n| n.kind == NoteKind::Reason)
        .expect("reason notu");
    assert_eq!(
        reason.text,
        "SystemVerilog reads an index as an unsigned number, so a negative value \
         selects a high bit or element without a diagnostic"
    );
    assert_eq!(
        help(&d),
        "convert the index explicitly to an unsigned number of the address width: \
         x[i as u3] for x : u8; a negative value then becomes a large index, -1 as u3 is 7"
    );
}

/// Yardımın genişliği tabanın adres genişliğidir: vektörde bit sayısının,
/// dizide eleman sayısının işaretsiz konumu (en az 1); indeksin kendi
/// genişliği değil (#474: daha geniş bir dönüşüm Verilator'da WIDTHTRUNC).
#[test]
fn the_help_width_is_the_address_width_of_the_base() {
    for (base, width) in [
        ("u1", 1),
        ("u2", 1),
        ("u3", 2),
        ("u4", 2),
        ("u5", 3),
        ("u8", 3),
        ("u9", 4),
        ("u16", 4),
        ("u17", 5),
        ("u64", 6),
        ("i8", 3),
        ("bits<8>", 3),
        ("bits<33>", 6),
    ] {
        let d = the_index_error(&module(
            &format!("    in  i : i8\n    in  x : {base}\n    out y : bool"),
            "    y = x[i]",
        ));
        assert_eq!(
            help(&d),
            expected_help(&format!("x[i as u{width}]"), &format!("x : {base}"), width),
            "{base}"
        );
    }
    for (base, width) in [
        ("[u8; 1]", 1),
        ("[u8; 2]", 1),
        ("[u8; 3]", 2),
        ("[u8; 4]", 2),
        ("[u8; 5]", 3),
        ("[u8; 8]", 3),
        ("[u8; 9]", 4),
        ("[u1; 1024]", 10),
        ("[u1; 1025]", 11),
        ("[[u8; 2]; 4]", 2),
    ] {
        let out = if base.starts_with("[[") {
            "[u8; 2]"
        } else if base.starts_with("[u1") {
            "u1"
        } else {
            "u8"
        };
        let d = the_index_error(&module(
            &format!("    in  i : i2\n    in  arr : {base}\n    out y : {out}"),
            "    y = arr[i]",
        ));
        assert_eq!(
            help(&d),
            expected_help(
                &format!("arr[i as u{width}]"),
                &format!("arr : {base}"),
                width
            ),
            "{base}"
        );
    }
    // İç içe dizide iç indeksin tabanı elemandır.
    let d = the_index_error(&module(
        "    in  i : i2\n    in  arr : [[u8; 2]; 4]\n    out y : u8",
        "    y = arr[0][i]",
    ));
    assert_eq!(help(&d), expected_help("arr[i as u1]", "arr : [u8; 2]", 1));
    // const dizi.
    let src = "const TABLE : [u8; 4] = [10, 20, 30, 40]

module M {
    in  i : i3
    out y : u8
    y = TABLE[i]
}
";
    assert_eq!(
        help(&the_index_error(src)),
        expected_help("arr[i as u2]", "arr : [u8; 4]", 2)
    );
}

/// Yardım seçimin biçimini gösterir: parça seçiminde `+:`/`-:`, aralıkta
/// hangi sınırın çevrileceği. Tabanın adresi vektörünkidir.
#[test]
fn the_help_follows_the_form_of_the_selection() {
    for (body, code) in [
        ("    y = x[i +: 2]", "x[i as u4 +: W]"),
        ("    y = x[i -: 2]", "x[i as u4 -: W]"),
    ] {
        let d = the_index_error(&module(
            "    in  i : i3\n    in  x : u16\n    out y : bits<2>",
            body,
        ));
        assert_eq!(help(&d), expected_help(code, "x : u16", 4), "{body}");
    }
    for (body, code) in [
        ("    y = x[K : 0]", "x[h as u3 : l]"),
        ("    y = x[1 : K]", "x[h : l as u3]"),
    ] {
        let src = format!(
            "const K : i8 = 1\n\nmodule M {{\n    in  x : u8\n    out y : bits<2>\n{body}\n}}\n"
        );
        assert_eq!(
            help(&the_index_error(&src)),
            expected_help(code, "x : u8", 3),
            "{body}"
        );
    }
}

/// Seçilemeyen ya da hatalı tabanda işaretli indeks kendi E2003'ünü
/// almaz: adres yoktur, tabanın tanısı tektir (bool tabanda bit seçimi,
/// dizide aralık ve parça seçimi).
#[test]
fn a_base_without_an_address_gives_its_own_diagnostic() {
    for (ports, body) in [
        (
            "    in  i : i3\n    in  f : bool\n    out y : bool",
            "    y = f[i]",
        ),
        (
            "    in  i : i3\n    in  arr : [u8; 4]\n    out y : bits<2>",
            "    y = arr[i +: 2]",
        ),
        (
            "const K : i8 = 1\n    in  arr : [u8; 4]\n    out y : bits<2>",
            "    y = arr[K : 0]",
        ),
    ] {
        let src = if let Some(rest) = ports.strip_prefix("const K : i8 = 1\n") {
            format!("const K : i8 = 1\n\n{}", module(rest, body))
        } else {
            module(ports, body)
        };
        let d = diags(&src);
        assert_eq!(d.len(), 1, "{src}\n{d:#?}");
        assert!(
            !d[0].message.starts_with("an index must be unsigned"),
            "{src}\n{d:#?}"
        );
    }
}

/// Yardımın kodu (ADR-0125 §4): indeksin yerine konunca tanı gider ve
/// yenisi çıkmaz; bit seçimi, dizi elemanı (port, const, register), parça
/// seçimi, aralık sınırı ve sol taraf. İndeks adresten geniş (i8 → u3),
/// eşit (i3 → u3) ya da dar (i2 → u3) olabilir; işaret değişimi daraltma
/// uyarısı vermez (type-inference.md §3.6).
#[test]
fn the_help_code_compiles_clean() {
    for index in ["i8", "i3", "i2"] {
        for (ports, body) in [
            (format!("    in  i : {index}\n    in  x : u8\n    out y : bool"), "    y = x[i as u3]".to_string()),
            (
                format!("    in  i : {index}\n    in  arr : [u8; 4]\n    out y : u8"),
                "    y = arr[i as u2]".to_string(),
            ),
            (
                format!("    in  i : {index}\n    in  x : u8\n    out y : bits<2>"),
                "    y = x[i as u3 +: 2]".to_string(),
            ),
            (
                format!("    in  i : {index}\n    in  x : u8\n    out y : bits<2>"),
                "    y = x[i as u3 -: 2]".to_string(),
            ),
            (
                format!("    in  clk : clock\n    in  i : {index}\n    out y : u8"),
                "    reg wa : [u8; 4] = [0, 0, 0, 0]\n    on clk {\n        wa[i as u2] <= 99\n    }\n    y = wa[0]".to_string(),
            ),
            (
                format!("    in  clk : clock\n    in  i : {index}\n    out y : u8"),
                "    reg r : u8 = 0\n    on clk {\n        r[i as u3] <= true\n    }\n    y = r".to_string(),
            ),
        ] {
            let c = codes(&module(&ports, &body));
            assert!(c.is_empty(), "{ports}\n{body}: {c:?}");
        }
    }
    let src = "const TABLE : [u8; 4] = [10, 20, 30, 40]
const K : i8 = 1

module M {
    in  i : i3
    in  x : u8
    out y : u8
    out z : bits<2>
    y = TABLE[i as u2]
    z = x[K as u3 : 0]
}
";
    assert!(codes(src).is_empty(), "{:?}", codes(src));
}

/// Öteki indeks tipleri değişmez (#475'in kapsamı): uN, esnek uN,
/// literal, bits<N>, bool, enum ve `[u1; N]` bugünkü gibi kabul edilir;
/// işaretli tabanın işaretsiz indeksi de.
#[test]
fn other_index_types_are_unchanged() {
    for (ty, index) in [
        ("u3", "i"),
        ("u8", "i"),
        ("bits<3>", "i"),
        ("bool", "i"),
        ("[u1; 2]", "i"),
        ("u2", "i + i"),
        ("u3", "i - i"),
        ("u2", "i * i"),
    ] {
        for (out, body) in [
            ("bool", format!("    y = x[{index}]")),
            ("bits<2>", format!("    y = x[{index} +: 2]")),
        ] {
            let c = codes(&module(
                &format!("    in  i : {ty}\n    in  x : i8\n    out y : {out}"),
                &body,
            ));
            assert!(c.is_empty(), "{ty} {body}: {c:?}");
        }
    }
    for body in ["    y = x[3]", "    y = x[7 - 4]"] {
        let c = codes(&module("    in  x : u8\n    out y : bool", body));
        assert!(c.is_empty(), "literal {body}: {c:?}");
    }
    let src = "const K : u8 = 3
const L : u32 = 2

module M {
    in  x : u8
    out y : bool
    out z : bits<2>
    y = x[K]
    z = x[L : 1]
}
";
    assert!(codes(src).is_empty(), "unsigned const: {:?}", codes(src));
    // Döngü değişkeni literaldir.
    let c = codes(&module(
        "    in  x : u8\n    out y : u8",
        "    comb {\n        y = 0\n        for k in 0..8 {\n            y[k] = x[7 - k]\n        }\n    }",
    ));
    assert!(c.is_empty(), "loop: {c:?}");
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
}

/// İşaretli bir değer dizinin elemanı ya da seçimin tabanı olabilir; kural
/// tabana değil indekse bakar.
#[test]
fn a_signed_base_or_element_is_not_this_rule() {
    let c = codes(&module(
        "    in  ws : [i8; 4]\n    in  i : u2\n    in  x : i8\n    out y : i8\n    out b : bool\n    out p : bits<2>",
        "    y = ws[i]\n    b = x[i]\n    p = x[i +: 2]",
    ));
    assert!(c.is_empty(), "{c:?}");
}

/// Generic `const S : i8` parametresi ayrıştırıcıda literale dönüşür
/// (ADR-0041, #468): `x[S]` örnekte `x[3]`'tür, tip denetimi S'nin
/// işaretli olduğunu görmez. Değer doğrudur (`P<-1>` bugün ayrıştırılmaz,
/// E0010); ADR-0147'nin Sınırlar bölümü. Bu test bugünkü davranışı
/// sabitler.
#[test]
fn a_const_generic_signed_index_is_a_literal_and_is_accepted() {
    let src = "module P<const S: i8> {
    in  x : u8
    out a : bool
    out b : bits<2>
    a = x[S]
    b = x[S +: 2]
}

module M {
    in  x : u8
    out a : bool
    out b : bits<2>
    let p = P<3> { x: x }
    a = p.a
    b = p.b
}
";
    assert!(codes(src).is_empty(), "{:?}", codes(src));
}
