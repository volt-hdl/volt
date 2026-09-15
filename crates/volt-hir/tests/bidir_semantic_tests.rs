//! Çift yönlü port anlamsal testleri — ADR-0051: E4008 doğrudan atama,
//! W3007 senkronizasyonsuz okuma, sentetik register'ların sessizliği ve
//! pass/fail fixture'ları.

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

/// Fixture anotasyonu: satır 1 `//~ EXXXX`, `//~^ ERROR` bir üst satırı bekler.
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
        "{rel}: {expected_code} satır {expected_line}'de bekleniyor, bulunan satırlar {lines:?} — kodlar {:?}",
        result.error_codes()
    );
}

const PAD: &str = "module Pad {\n    in clk : clock\n    in en : bool\n    opendrain sda : bool\n    on clk {\n        if en { sda.drive_low() } else { sda.release() }\n    }\n}\n";

// ═══ Fixture'lar ══════════════════════════════════════════════════

#[test]
fn ui_pass_68_inout_bidirectional_warns_w3007_only() {
    let (_, result) = analyze_file("pass/68_inout_bidirectional.volt");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
    let codes = result.error_codes();
    assert!(codes.iter().all(|c| *c == "W3007"), "{codes:?}");
    assert_eq!(
        codes.len(),
        1,
        "yalnız SramPort'un doğrudan dq okuması: {codes:?}"
    );
}

#[test]
fn ui_pass_69_opendrain_basic_clean() {
    let (_, result) = analyze_file("pass/69_opendrain_basic.volt");
    assert!(
        result.diagnostics.is_empty(),
        "temiz geçmeli: {:?}",
        result.error_codes()
    );
}

#[test]
fn ui_fail_53_opendrain_direct_assign_e4008() {
    assert_ui_fail("fail/53_opendrain_direct_assign.volt");
}

#[test]
fn ui_fail_54_inout_unsynchronized_w3007() {
    assert_ui_fail("fail/54_inout_unsynchronized.volt");
}

// ═══ E4008 — doğrudan atama ═══════════════════════════════════════

#[test]
fn nonblocking_assignment_to_inout_port_is_e4008() {
    let result = analyze_src(
        "module M {\n    in clk : clock\n    inout dq : u8\n    on clk { dq <= 0 }\n}\n",
    );
    assert_eq!(result.error_codes(), vec!["E4008"]);
}

#[test]
fn continuous_assignment_to_inout_port_is_e4008() {
    let result = analyze_src(
        "module M {\n    in clk : clock\n    in v : u8\n    inout dq : u8\n    dq = v\n}\n",
    );
    assert!(
        result.error_codes().contains(&"E4008"),
        "{:?}",
        result.error_codes()
    );
}

#[test]
fn e4008_names_the_port_and_its_methods() {
    let result = analyze_src(
        "module M {\n    in clk : clock\n    opendrain sda : bool\n    sda = true\n}\n",
    );
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E4008")
        .expect("E4008");
    assert!(
        diag.message.contains("opendrain port 'sda'"),
        "{}",
        diag.message
    );
    let help = diag.help.as_deref().unwrap_or("");
    assert!(help.contains("sda.drive_low()"), "{help}");
    assert!(help.contains("sda.release()"), "{help}");
}

// ═══ W3007 — senkronizasyonsuz okuma ══════════════════════════════

#[test]
fn direct_read_in_on_block_is_w3007() {
    let result = analyze_src("module M {\n    in clk : clock\n    opendrain sda : bool\n    out q : bool\n    reg q_r : bool = false\n    on clk { q_r <= sda.read() }\n    q = q_r\n}\n");
    assert_eq!(result.error_codes(), vec!["W3007"]);
    assert!(!result.has_errors());
}

#[test]
fn bare_port_read_and_comb_read_are_w3007() {
    // (clk hiç kullanılmadığından W1001 de gelir; bu testin konusu değil.)
    let bidir_codes = |r: &volt_hir::AnalysisResult| -> Vec<&'static str> {
        r.error_codes()
            .into_iter()
            .filter(|c| *c != "W1001")
            .collect()
    };
    let result = analyze_src("module M {\n    in clk : clock\n    inout pin : bool\n    out q : bool\n    let level = pin\n    q = level\n}\n");
    assert_eq!(bidir_codes(&result), vec!["W3007"]);
    let result = analyze_src("module M {\n    in clk : clock\n    inout pin : bool\n    out q : bool\n    comb { q = pin.read() }\n}\n");
    assert_eq!(bidir_codes(&result), vec!["W3007"]);
}

#[test]
fn sync_source_read_is_neither_w3007_nor_w3002() {
    let result = analyze_src("module M {\n    in clk : clock\n    opendrain sda : bool\n    out q : bool\n    wire sda_s : bool\n    sda_s = sync(sda.read(), clk)\n    reg q_r : bool = false\n    on clk { q_r <= sda_s }\n    q = q_r\n}\n");
    assert!(
        result.diagnostics.is_empty(),
        "temiz olmalı: {:?}",
        result.error_codes()
    );
}

#[test]
fn contract_read_of_a_pad_is_not_w3007() {
    let result = analyze_src("module M {\n    in clk : clock\n    opendrain sda : bool\n    invariant: sda.driving -> !sda.read()\n    on clk { sda.drive_low() }\n}\n");
    assert!(
        result.diagnostics.is_empty(),
        "temiz olmalı: {:?}",
        result.error_codes()
    );
}

#[test]
fn w3007_message_suggests_sync() {
    let result = analyze_src("module M {\n    in clk : clock\n    inout pin : bool\n    out q : bool\n    q = pin.read()\n}\n");
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "W3007")
        .expect("W3007");
    assert!(diag.code.is_warning());
    let help = diag.help.as_deref().unwrap_or("");
    assert!(help.contains("sync(pin.read()"), "{help}");
}

// ═══ Sentetik register'lar ════════════════════════════════════════

#[test]
fn synthesised_drive_registers_are_not_reported_as_unread() {
    // sda_drive_low kaynakta okunmaz; SV'de tri-state tamponu okur.
    let result = analyze_src(PAD);
    assert!(
        result.diagnostics.is_empty(),
        "W1004/W4002 üretmemeli: {:?}",
        result.error_codes()
    );
}

#[test]
fn inout_data_register_reset_value_type_checks_for_every_scalar_type() {
    for ty in ["bool", "u8", "i16", "bits<12>", "u3"] {
        let src = format!(
            "module M {{\n    in clk : clock\n    inout dq : {ty}\n    on clk {{ dq.release() }}\n}}\n"
        );
        let result = analyze_src(&src);
        assert!(!result.has_errors(), "{ty}: {:?}", result.error_codes());
    }
}

#[test]
fn opendrain_bus_shared_by_two_instances_is_clean() {
    let src = format!(
        "{PAD}module Bus {{\n    in clk : clock\n    in a : bool\n    in b : bool\n    out line : bool\n    wire sda_bus : bool\n    let pa = Pad {{ clk, en: a, sda: sda_bus }}\n    let pb = Pad {{ clk, en: b, sda: sda_bus }}\n    line = sda_bus\n}}\n"
    );
    let result = analyze_src(&src);
    assert!(
        result.diagnostics.is_empty(),
        "temiz olmalı: {:?}",
        result.error_codes()
    );
}

#[test]
fn user_written_name_clashing_with_the_drive_register_is_e1003() {
    let result = analyze_src("module M {\n    in clk : clock\n    opendrain sda : bool\n    reg sda_drive_low : bool = false\n    on clk { sda.drive_low() }\n}\n");
    assert!(
        result.error_codes().contains(&"E1003"),
        "{:?}",
        result.error_codes()
    );
}
