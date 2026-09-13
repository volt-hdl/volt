//! `prev()` ardışık kontrat testleri — ADR-0040: E5017 (kontrat dışı /
//! geçersiz argüman), tip taşıma, domain kuralı (E3001) ve fixture'lar.

use volt_hir::analyze;
use volt_span::SourceMap;
use volt_syntax::{parse, FileId};

fn analyze_src(src: &str) -> volt_hir::AnalysisResult {
    let parsed = parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "ayrışmalı: {:?}",
        parsed.error_codes()
    );
    analyze(&parsed.ast)
}

fn analyze_file(rel: &str) -> (String, volt_hir::AnalysisResult) {
    let path = format!("{}/../../tests/ui/{rel}", env!("CARGO_MANIFEST_DIR"));
    let src = std::fs::read_to_string(&path).expect("ui dosyası okunmalı");
    let result = analyze_src(&src);
    (src, result)
}

fn assert_ui_fail(rel: &str) {
    let (src, result) = analyze_file(rel);
    let expected_code = src
        .lines()
        .next()
        .and_then(|l| l.trim().strip_prefix("//~ "))
        .map(str::trim)
        .expect("fixture ilk satırı '//~ EXXXX' olmalı")
        .to_string();
    let expected_line = src
        .lines()
        .position(|l| l.trim_start().starts_with("//~^ ERROR"))
        .map(|i| i as u32)
        .expect("fixture '//~^ ERROR' anotasyonu içermeli");
    let mut map = SourceMap::new();
    map.add_file(rel, src.clone());
    let lines: Vec<u32> = result
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == expected_code)
        .filter_map(|d| d.primary_span())
        .map(|s| map.line_col_utf8(s.span).0)
        .collect();
    assert!(
        lines.contains(&expected_line),
        "{rel}: {expected_code} satır {expected_line}'de bekleniyor, bulunan {lines:?} — kodlar {:?}",
        result.error_codes()
    );
}

fn count(result: &volt_hir::AnalysisResult, code: &str) -> usize {
    result.error_codes().iter().filter(|c| **c == code).count()
}

const HEAD: &str = "module M {\n    in  clk : clock\n    in  x : u8\n    in  go : bool\n    out y : u8\n    reg r : u8 = 0\n    on clk { r <= x }\n    y = r\n";

// ═══ Fixture'lar ══════════════════════════════════════════════════

#[test]
fn ui_pass_49_prev_contract_clean() {
    let (_, result) = analyze_file("pass/49_prev_contract.volt");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn ui_fail_37_prev_in_rtl_e5017() {
    assert_ui_fail("fail/37_prev_in_rtl.volt");
}

#[test]
fn ui_fail_38_prev_wrong_domain_e3001() {
    assert_ui_fail("fail/38_prev_wrong_domain.volt");
}

// ═══ E5017 — bağlam ═══════════════════════════════════════════════

#[test]
fn prev_in_continuous_assignment_is_e5017() {
    let result =
        analyze_src("module M {\n in clk : clock\n in x : u8\n out y : u8\n y = prev(x) }");
    assert_eq!(count(&result, "E5017"), 1, "{:?}", result.error_codes());
}

#[test]
fn prev_in_let_is_e5017() {
    let result = analyze_src(
        "module M {\n in clk : clock\n in x : u8\n out y : u8\n let p : u8 = prev(x)\n y = p }",
    );
    assert_eq!(count(&result, "E5017"), 1, "{:?}", result.error_codes());
}

#[test]
fn prev_in_on_block_is_e5017() {
    let result = analyze_src(
        "module M {\n in clk : clock\n in x : u8\n out y : u8\n reg r : u8 = 0\n on clk { r <= prev(x) }\n y = r }",
    );
    assert_eq!(count(&result, "E5017"), 1, "{:?}", result.error_codes());
}

#[test]
fn prev_in_comb_block_is_e5017() {
    let result = analyze_src(
        "module M {\n in clk : clock\n in x : u8\n out y : u8\n comb { y = prev(x) } }",
    );
    assert_eq!(count(&result, "E5017"), 1, "{:?}", result.error_codes());
}

#[test]
fn prev_in_fn_body_is_e5017_but_fn_contract_is_fine() {
    let result = analyze_src("fn f(a: u8) -> u8 requires: prev(a) == a { prev(a) }");
    assert_eq!(count(&result, "E5017"), 1, "{:?}", result.error_codes());
}

#[test]
fn e5017_message_carries_five_parts() {
    let result =
        analyze_src("module M {\n in clk : clock\n in x : u8\n out y : u8\n y = prev(x) }");
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E5017")
        .expect("E5017");
    assert!(diag.message.contains("prev()"), "{}", diag.message);
    assert!(diag.primary_span().is_some());
    assert!(
        diag.help.as_deref().unwrap_or("").contains("reg"),
        "öneri RTL'de reg kullanımını göstermeli: {:?}",
        diag.help
    );
}

// ═══ E5017 — argümanlar ═══════════════════════════════════════════

#[test]
fn prev_without_arguments_is_e5017() {
    let result = analyze_src(&format!("{HEAD}    invariant: prev() == 0\n}}"));
    assert_eq!(count(&result, "E5017"), 1, "{:?}", result.error_codes());
}

#[test]
fn prev_with_three_arguments_is_e5017() {
    let result = analyze_src(&format!("{HEAD}    invariant: prev(x, 1, 2) == 0\n}}"));
    assert_eq!(count(&result, "E5017"), 1, "{:?}", result.error_codes());
}

#[test]
fn prev_with_zero_depth_is_e5017() {
    let result = analyze_src(&format!("{HEAD}    invariant: prev(x, 0) == 0\n}}"));
    assert_eq!(count(&result, "E5017"), 1, "{:?}", result.error_codes());
}

#[test]
fn prev_with_non_literal_depth_is_e5017() {
    let result = analyze_src(&format!("{HEAD}    invariant: prev(x, x) == 0\n}}"));
    assert_eq!(count(&result, "E5017"), 1, "{:?}", result.error_codes());
}

#[test]
fn prev_with_literal_depth_is_accepted() {
    let result = analyze_src(&format!("{HEAD}    invariant: prev(x, 3) == r\n}}"));
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

// ═══ Tip ve kapsam ════════════════════════════════════════════════

#[test]
fn prev_carries_the_argument_type() {
    // u8 == u8 tip hatasız; prev(u8) tek başına Bool değil → E5004.
    let ok = analyze_src(&format!("{HEAD}    invariant: prev(x) == r\n}}"));
    assert!(!ok.has_errors(), "{:?}", ok.error_codes());
    let bad = analyze_src(&format!("{HEAD}    invariant: prev(x)\n}}"));
    assert_eq!(count(&bad, "E5004"), 1, "{:?}", bad.error_codes());
}

#[test]
fn prev_of_bool_is_a_contract_condition() {
    let result = analyze_src(&format!(
        "{HEAD}    invariant: prev(go) -> r == prev(x)\n}}"
    ));
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn prev_is_legal_in_every_contract_kind() {
    let src = format!(
        "{HEAD}    requires: prev(go) || true\n    ensures: prev(go) || true\n    invariant: prev(r) == prev(r)\n    cover: prev(go)\n    assert: prev(go) || true\n    assume: prev(go) || true\n}}"
    );
    let result = analyze_src(&src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn nested_prev_is_accepted() {
    let result = analyze_src(&format!(
        "{HEAD}    invariant: prev(prev(x)) == prev(x, 2)\n}}"
    ));
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

// ═══ Domain kuralı ════════════════════════════════════════════════

const TWO: &str = "domain Fast { clock = posedge\n reset = sync active_high }\ndomain Slow { clock = posedge\n reset = sync active_high }\nmodule M {\n in f : clock @Fast\n in s : clock @Slow\n in a : u8 @Fast\n in a2 : u8 @Fast\n in b : u8 @Slow\n out o : bool @Slow\n o = b == 0\n";

#[test]
fn prev_across_domains_is_e3001() {
    let result = analyze_src(&format!("{TWO}    invariant: prev(a) == b\n}}"));
    assert!(
        result.error_codes().contains(&"E3001"),
        "{:?}",
        result.error_codes()
    );
}

#[test]
fn prev_within_one_domain_is_clean() {
    let result = analyze_src(&format!("{TWO}    invariant: prev(a) == a2\n}}"));
    assert!(
        !result.error_codes().contains(&"E3001"),
        "{:?}",
        result.error_codes()
    );
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}
