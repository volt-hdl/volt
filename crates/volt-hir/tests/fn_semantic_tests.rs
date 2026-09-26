//! Fonksiyonların anlamsal katmanlarla etkileşimi (ADR-0081 §4): domain
//! (Karar 9, K5 join), trust (Karar 10), L1 zamanlama (Delayed, E5010)
//! çağrı yerinde argümanları birleştirir; tip kuralları (Karar 2, 5) ve
//! tanının yeri ilkesi (§6).

use volt_hir::{analyze, AnalysisResult};
use volt_span::SourceMap;
use volt_syntax::{parse, FileId};

fn check(src: &str) -> AnalysisResult {
    let parsed = parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "kaynak ayrışmalı: {:?}",
        parsed.error_codes()
    );
    analyze(&parsed.ast)
}

/// Hata kodları + birincil satır (1 tabanlı).
fn errors_at(src: &str) -> Vec<(&'static str, u32)> {
    let result = check(src);
    let mut map = SourceMap::new();
    map.add_file("t.volt", src.to_string());
    result
        .diagnostics
        .iter()
        .filter(|d| !d.code.is_warning())
        .filter_map(|d| {
            d.primary_span()
                .map(|s| (d.code.as_str(), map.line_col_utf8(s.span).0))
        })
        .collect()
}

fn codes_of(src: &str, wanted: &[&str]) -> Vec<&'static str> {
    check(src)
        .error_codes()
        .into_iter()
        .filter(|c| wanted.contains(c))
        .collect()
}

// ═══ Domain (Karar 9) ═══════════════════════════════════════════════

const TWO_DOMAINS: &str = "domain Fast { clock = posedge, reset = sync active_high }\n\
                           domain Slow { clock = posedge, reset = sync active_high }\n";

fn cdc_module(fn_src: &str, call: &str) -> String {
    format!(
        "{TWO_DOMAINS}\n{fn_src}\n\nmodule M {{\n    in  fast_clk : clock @Fast\n    in  slow_clk : clock @Slow\n    in  f : bool @Fast\n    in  f2 : bool @Fast\n    in  s : bool @Slow\n    out y : bool @Fast\n    y = {call}\n}}\n"
    )
}

#[test]
fn arguments_from_two_domains_are_e3001_at_the_call() {
    let src = cdc_module("fn both(a: bool, b: bool) -> bool { a && b }", "both(f, s)");
    // Satır: 2 domain + boş + fn + 2 boş + module + 6 port → 13.
    assert_eq!(errors_at(&src), vec![("E3001", 13)]);
}

#[test]
fn arguments_from_one_domain_take_that_domain() {
    let src = cdc_module(
        "fn both(a: bool, b: bool) -> bool { a && b }",
        "both(f, f2)",
    );
    assert!(errors_at(&src).is_empty(), "{:?}", errors_at(&src));
}

/// Muhafazakâr yön: gövdede kullanılmayan parametre de join'e girer
/// (parametre başına bağımlılık analizi yok).
#[test]
fn an_unused_parameter_still_joins_the_call_domain() {
    let src = cdc_module("fn first(a: bool, b: bool) -> bool { a }", "first(f, s)");
    assert_eq!(codes_of(&src, &["E3001"]), vec!["E3001"]);
}

// ═══ Trust (Karar 10) ══════════════════════════════════════════════

const TRUST: &str = "domain SecureCore { clock = posedge, reset = sync active_high, trust_level = secret }\n\
                     domain Debug      { clock = posedge, reset = sync active_high, trust_level = public }\n";

fn trust_module(expr: &str) -> String {
    format!(
        "{TRUST}\nfn mask(k: u8) -> u8 {{ k ^ 0x5A }}\n\nmodule M {{\n    in  key : u8 @SecureCore\n    out dbg : u8 @Debug\n    dbg = {expr}\n}}\n"
    )
}

/// Bilgi fn'den geçer: gizli argümanın sonucu gizlidir.
#[test]
fn a_secret_argument_makes_the_call_result_secret() {
    let src = trust_module("mask(key)");
    assert_eq!(codes_of(&src, &["E3009", "E3015"]), vec!["E3009"]);
}

/// Düşürme çağıranın modülünde, çağrının sonucuna yazılır.
#[test]
fn declassify_on_the_call_result_in_the_caller_is_allowed() {
    let src = trust_module("declassify(mask(key), \"masked with a public constant\")");
    assert!(codes_of(&src, &["E3009", "E3015"]).is_empty());
    assert!(check(&src).error_codes().contains(&"W3008"));
}

#[test]
fn declassify_inside_a_function_is_e3015_once_at_the_definition() {
    let src = format!(
        "{TRUST}\nfn reveal(k: u8) -> u8 {{ declassify(k, \"debug\") }}\n\nmodule M {{\n    in  key : u8 @SecureCore\n    out a : u8 @SecureCore\n    out b : u8 @SecureCore\n    a = reveal(key)\n    b = reveal(key + 1)\n}}\n"
    );
    assert_eq!(errors_at(&src), vec![("E3015", 4)]);
}

// ═══ L1 zamanlama (Karar 10, ADR-0037) ═════════════════════════════

fn timing_module(call: &str) -> String {
    format!(
        "fn add(a: u32, b: u32) -> u32 {{ a + b }}\n\n@strict_timing\nmodule M {{\n    in  clk : clock\n    in  x   : u32\n    out y   : u32\n    reg a : Delayed<u32, 1> = 0\n    reg b : Delayed<u32, 2> = 0\n    reg c : Delayed<u32, 2> = 0\n    let sum = {call}\n    on clk {{\n        a <= x\n        b <= a\n        c <= a\n    }}\n    y = sum\n}}\n"
    )
}

/// fn gecikmeyi değiştirmez; farklı gecikmeli argümanlar çağrı yerinde E5010.
#[test]
fn arguments_with_different_latency_are_e5010_at_the_call() {
    let src = timing_module("add(a, b)");
    assert_eq!(codes_of(&src, &["E5010"]), vec!["E5010"]);
    assert!(
        errors_at(&src).contains(&("E5010", 11)),
        "{:?}",
        errors_at(&src)
    );
}

#[test]
fn arguments_with_equal_latency_pass_through() {
    let src = timing_module("add(b, c)");
    assert!(codes_of(&src, &["E5010"]).is_empty());
}

// ═══ Tip kuralları (Karar 2, 5) ════════════════════════════════════

/// Son ifade dönüş tipine `check` edilir: yazılı hedef kuralı —
/// aynı işaretli genişleme örtük (ADR-0041).
#[test]
fn the_final_expression_is_checked_against_the_return_type() {
    let ok = "fn wide(a: u8) -> u16 { a }\nmodule M { in a : u8 out y : u16 y = wide(a) }\n";
    assert!(errors_at(ok).is_empty(), "{:?}", errors_at(ok));
    let narrow = "fn narrow(a: u16) -> u8 { a }\n";
    assert_eq!(
        codes_of(narrow, &["E2001", "E2003"]),
        vec!["E2001"],
        "daraltma 'as' ister"
    );
}

/// Çağrının tipi dönüş tipidir (bool'u sayı hedefine yazmak hatadır).
#[test]
fn the_call_has_the_return_type() {
    let src = "fn is0(a: u8) -> bool { a == 0 }\nmodule M { in a : u8 out y : u8 y = is0(a) }\n";
    assert_eq!(codes_of(src, &["E2003"]), vec!["E2003"]);
}

/// Gövde imzaya karşı bir kez denetlenir: gövdedeki tip hatası çağrı
/// sayısıyla çoğalmaz (tanının yeri ilkesi).
#[test]
fn a_body_type_error_is_reported_once_at_the_definition() {
    let src = "fn bad(a: u8) -> u8 { a + true }\nmodule M { in a : u8 out y : u8 y = bad(a) + bad(a) + bad(a) }\n";
    assert_eq!(errors_at(src), vec![("E2003", 1)]);
}

/// Gövde yalnız parametreleri ve birimin öğelerini görür (modül
/// sinyali E1001, ADR-0081 Karar 1).
#[test]
fn a_module_signal_is_not_visible_in_a_function_body() {
    let src = "fn f(a: u8) -> u8 { a + x }\nmodule M { in x : u8 out y : u8 y = f(x) }\n";
    assert_eq!(codes_of(src, &["E1001"]), vec!["E1001"]);
}
