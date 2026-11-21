//! Reset veri olarak okunamaz (ADR-0065 R3, #356): `match`'in sınananı ve
//! karşılaştırma işleçlerinin (`==`, `!=`, `<`, `>`, `<=`, `>=`) operandı
//! reset tipli olamaz, E2003. Öteki yollar (`if`, `!`, `&&`, `as`,
//! birleştirme, bit seçme, fn argümanı, struct alanı) zaten reddediliyor;
//! burada yeniden sınanır.
//!
//! Beklenen kodlar ve metinler ADR-0065 R3'ten ve ADR-0125'ten (ne oldu,
//! neden, ne yapılabilir; makine önerisi yok) elle yazıldı.

use volt_diagnostics::Diagnostic;
use volt_hir::analyze;
use volt_syntax::{parse, FileId};

fn diags(src: &str) -> Vec<Diagnostic> {
    let parsed = parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse hataları: {:?}",
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

/// Reset'i veri olarak okuyan E2003 tanıları (iletisi "a reset cannot").
fn reset_as_data(src: &str) -> Vec<Diagnostic> {
    diags(src)
        .into_iter()
        .filter(|d| d.code.as_str() == "E2003" && d.message.starts_with("a reset cannot"))
        .collect()
}

/// `t`'yi `expr` olarak tanımlayan, `sync(t, clk)`'i çıkışa veren modül
/// (#356'nın biçimi).
fn through_sync(expr: &str) -> String {
    format!(
        "module M {{
    in  clk : clock
    in  rst : reset(sync, active_high)
    out y   : bool
    let t = {expr}
    y = sync(t, clk)
}}
"
    )
}

/// `y`'yi doğrudan `expr` ile süren modül; `clk` bir register'ı sürer.
fn direct(expr: &str) -> String {
    format!(
        "module M {{
    in  clk : clock
    in  rst : reset(sync, active_high)
    out y   : bool
    out q   : u8
    reg r : u8 = 0
    on clk {{ r <= r + 1 }}
    q = r
    y = {expr}
}}
"
    )
}

#[test]
fn issue_356_design_is_e2003() {
    let src = through_sync("match rst { true => true, _ => false }");
    assert_eq!(codes(&src), ["E2003"], "{:?}", diags(&src));
    assert_eq!(reset_as_data(&src).len(), 1);
}

#[test]
fn a_match_on_a_reset_is_e2003_with_and_without_sync() {
    for src in [
        through_sync("match rst { true => true, _ => false }"),
        direct("match rst { true => true, _ => false }"),
        direct("match rst { false => false, _ => true }"),
    ] {
        let d = reset_as_data(&src);
        assert_eq!(d.len(), 1, "{src}\n{:?}", diags(&src));
        assert!(d[0].message.contains("match"), "{}", d[0].message);
    }
}

#[test]
fn a_match_statement_on_a_reset_is_e2003() {
    let src = "module M {
    in  clk : clock
    in  rst : reset(sync, active_high)
    out q   : u8
    reg r : u8 = 0
    on clk {
        match rst {
            true => { r <= 0 }
            _ => { r <= r + 1 }
        }
    }
    q = r
}
";
    assert_eq!(reset_as_data(src).len(), 1, "{:?}", diags(src));
}

#[test]
fn comparing_a_reset_with_itself_is_e2003_once() {
    for op in ["==", "!=", "<", ">", "<=", ">="] {
        let e = format!("rst {op} rst");
        for src in [through_sync(&e), direct(&e)] {
            let d = reset_as_data(&src);
            assert_eq!(d.len(), 1, "{src}\n{:?}", diags(&src));
            assert!(d[0].message.contains("compared"), "{}", d[0].message);
        }
    }
}

#[test]
fn comparing_two_reset_ports_is_e2003() {
    // ADR-0065'in reset denetimleri (E3010, E3003) bu tasarıma başka
    // nedenlerle de itiraz edebilir; burada istenen, karşılaştırmanın
    // kendisinin E2003 almasıdır.
    for e in ["rst != rst2", "rst2 == rst"] {
        let src = format!(
            "module M {{
    in  clk  : clock
    in  rst  : reset(sync, active_high)
    in  rst2 : reset(sync, active_high)
    out y    : bool
    let t = {e}
    y = sync(t, clk)
}}
"
        );
        assert_eq!(reset_as_data(&src).len(), 1, "{src}\n{:?}", diags(&src));
    }
}

#[test]
fn comparing_arrays_of_resets_is_e2003() {
    // Dizi her elemanını okur; içindeki reset de veri olarak okunur.
    let src = through_sync("[rst, rst] == [rst, rst]");
    let d = reset_as_data(&src);
    assert_eq!(d.len(), 1, "{:?}", diags(&src));
    assert!(d[0].message.contains("holds a reset"), "{}", d[0].message);
}

#[test]
fn a_named_array_of_resets_is_not_called_a_reset() {
    // `a` bir ad ama reset değil, reset taşıyan bir dizi: ileti onu "a
    // reset" diye adlandırmaz (mutasyon kapısı, #356: muhafız `is_reset`).
    let src = "module M {
    in  clk : clock
    in  rst : reset(sync, active_high)
    out y   : bool
    let a = [rst, rst]
    let t = a == a
    y = sync(t, clk)
}
";
    let d = reset_as_data(src);
    assert_eq!(d.len(), 1, "{:?}", diags(src));
    assert_eq!(
        d[0].message, "a reset cannot be compared: this value holds a reset",
        "{}",
        d[0].message
    );
}

#[test]
fn a_reset_compared_with_a_value_gets_the_reset_diagnostic() {
    // Önceden "comparison operands must have the same type" idi; kod aynı
    // (E2003), metin reset'in neden okunamadığını söyler.
    for e in ["rst == true", "rst != false", "true == rst"] {
        let src = through_sync(e);
        assert_eq!(codes(&src), ["E2003"], "{e}: {:?}", diags(&src));
        assert_eq!(reset_as_data(&src).len(), 1, "{e}");
    }
}

#[test]
fn the_diagnostic_says_what_why_and_what_to_do_without_a_fix() {
    let src = through_sync("match rst { true => true, _ => false }");
    let d = &reset_as_data(&src)[0];
    assert!(d.message.contains("'rst' is a reset"), "{}", d.message);
    let reason = d
        .notes
        .iter()
        .map(|n| n.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    for part in [
        "a reset is not a value",
        "ordinary signal",
        "synchronizer",
        "reset check",
    ] {
        assert!(
            reason.contains(part),
            "neden satırında {part:?} yok:\n{reason}"
        );
    }
    let help = d.help.clone().expect("help");
    for part in ["reset value", "bool input"] {
        assert!(help.contains(part), "help'te {part:?} yok:\n{help}");
    }
    assert!(d.suggestions.is_empty(), "makine önerisi olmamalı");
    let all = format!("{}\n{reason}\n{help}", d.message);
    assert!(!all.contains("ADR"), "{all}");
    assert!(!all.contains(".md"), "{all}");
}

#[test]
fn the_helps_bool_input_compiles_clean() {
    // Help'in ikinci yolu: yazılımla tetiklenen yumuşak reset ayrı bir bool
    // giriş portudur; aynı tasarım onunla hatasız derlenir.
    let src = "module M {
    in  clk  : clock
    in  rst  : reset(sync, active_high)
    in  soft_rst : bool
    out y    : bool
    let t = match soft_rst { true => true, _ => false }
    y = sync(t, clk)
}
";
    assert!(
        diags(src)
            .iter()
            .all(|d| d.severity != volt_diagnostics::Severity::Error),
        "{:?}",
        codes(src)
    );
}

#[test]
fn a_reset_where_a_value_is_expected_gets_the_same_diagnostic() {
    // `if rst` ve beklenen tipi olan her konum: reset'i veri olarak okuyan
    // her yer aynı denetimden geçer, aynı tanıyı alır (#356).
    for e in [
        "if rst { true } else { false }",
        "!rst",
        "rst && true",
        "true || rst",
    ] {
        let src = through_sync(e);
        let d = reset_as_data(&src);
        assert_eq!(d.len(), 1, "{e}: {:?}", diags(&src));
        assert_eq!(codes(&src), ["E2003"], "{e}");
        assert!(
            d[0].message
                .contains("a reset cannot be used as a value of type 'bool'"),
            "{e}: {}",
            d[0].message
        );
    }
    let typed_let = "module M {
    in  clk : clock
    in  rst : reset(sync, active_high)
    out y   : bool
    let b : bool = rst
    y = sync(b, clk)
}
";
    assert_eq!(reset_as_data(typed_let).len(), 1, "{:?}", diags(typed_let));
    let fn_arg = "fn id(x: bool) -> bool { x }
module M {
    in  clk : clock
    in  rst : reset(sync, active_high)
    out y   : bool
    let t = id(rst)
    y = sync(t, clk)
}
";
    assert_eq!(reset_as_data(fn_arg).len(), 1, "{:?}", diags(fn_arg));
}

#[test]
fn the_other_ways_to_read_a_reset_are_still_rejected() {
    // #356'nın kontrol tablosu: her biri bugün de reddedilmeli.
    // `concat(…)` ve `zext(…)` sürücünün E0003'üdür (ifade içinde çağrı);
    // `volt-driver/tests/reset_as_data_tests.rs` sınar.
    let cases: &[(&str, &str)] = &[
        ("rst as bool", "E2009"),
        ("rst as u1", "E2009"),
        ("rst[0]", "E2003"),
        ("if rst { true } else { false }", "E2003"),
        ("!rst", "E2003"),
        ("rst && true", "E2003"),
    ];
    for (e, code) in cases {
        let c = codes(&through_sync(e));
        assert!(c.iter().any(|x| x == code), "{e}: {code} bekleniyor, {c:?}");
    }
    let fn_arg = "fn id(x: bool) -> bool { x }
module M {
    in  clk : clock
    in  rst : reset(sync, active_high)
    out y   : bool
    let t = id(rst)
    y = sync(t, clk)
}
";
    assert!(
        codes(fn_arg).iter().any(|x| x == "E2003"),
        "{:?}",
        codes(fn_arg)
    );
    let field = "struct S { r: bool }
module M {
    in  clk : clock
    in  rst : reset(sync, active_high)
    out y   : bool
    let s = S { r: rst }
    y = sync(s.r, clk)
}
";
    assert!(
        codes(field).iter().any(|x| x == "E2003"),
        "{:?}",
        codes(field)
    );
    let sync_rst = "module M {
    in  clk : clock
    in  rst : reset(sync, active_high)
    out y   : bool
    y = sync(rst, clk)
}
";
    assert!(
        codes(sync_rst).iter().any(|x| x == "E2003"),
        "{:?}",
        codes(sync_rst)
    );
}
