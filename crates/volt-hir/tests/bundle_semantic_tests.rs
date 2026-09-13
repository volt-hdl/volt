//! Bundle (port grubu) anlamsal testleri — ADR-0039: E4005 yön ihlali,
//! E3013 domain tutarsızlığı ve pass fixture'larının temizliği.

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

const HANDSHAKE: &str =
    "struct port Handshake {\n    out data  : u8\n    out valid : bool\n    in  ready : bool\n}\n";

// ═══ Fixture'lar ══════════════════════════════════════════════════

#[test]
fn ui_pass_47_bundle_basic_clean() {
    let (_, result) = analyze_file("pass/47_bundle_basic.volt");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn ui_pass_48_bundle_nested_clean() {
    let (_, result) = analyze_file("pass/48_bundle_nested.volt");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn ui_fail_35_bundle_direction_e4005() {
    assert_ui_fail("fail/35_bundle_direction.volt");
}

#[test]
fn ui_fail_36_bundle_domain_e3013() {
    assert_ui_fail("fail/36_bundle_domain.volt");
}

// ═══ E4005 — yön ihlali ═══════════════════════════════════════════

#[test]
fn e4005_for_assignment_to_flipped_out_field() {
    let src =
        format!("{HANDSHAKE}module M {{ in hs : Handshake\n hs.data = 0\n hs.ready = true }}");
    let result = analyze_src(&src);
    assert_eq!(
        result
            .error_codes()
            .iter()
            .filter(|c| **c == "E4005")
            .count(),
        1
    );
}

#[test]
fn e4005_for_assignment_to_in_field_of_out_bundle() {
    let src = format!("{HANDSHAKE}module M {{ out hs : Handshake\n hs.data = 0\n hs.valid = true\n hs.ready = true }}");
    let result = analyze_src(&src);
    assert!(
        result.error_codes().contains(&"E4005"),
        "{:?}",
        result.error_codes()
    );
}

#[test]
fn no_e4005_when_only_outward_fields_are_driven() {
    let src =
        format!("{HANDSHAKE}module M {{ out hs : Handshake\n hs.data = 0\n hs.valid = true }}");
    let result = analyze_src(&src);
    assert!(
        !result.error_codes().contains(&"E4005"),
        "{:?}",
        result.error_codes()
    );
}

#[test]
fn e4005_inside_comb_block() {
    let src = format!("{HANDSHAKE}module M {{ in hs : Handshake\n comb {{ hs.valid = true }}\n hs.ready = true }}");
    let result = analyze_src(&src);
    assert!(
        result.error_codes().contains(&"E4005"),
        "{:?}",
        result.error_codes()
    );
}

#[test]
fn e4005_message_carries_all_five_parts() {
    let src =
        format!("{HANDSHAKE}module M {{ in hs : Handshake\n hs.data = 0\n hs.ready = true }}");
    let result = analyze_src(&src);
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E4005")
        .expect("E4005");
    assert!(diag.message.contains("hs.data"), "{}", diag.message);
    assert!(diag.primary_span().is_some());
    assert!(
        diag.help
            .as_deref()
            .unwrap_or("")
            .contains("out hs : Handshake"),
        "{:?}",
        diag.help
    );
    assert!(
        diag.notes.iter().any(|n| n.text.contains("ADR-0039")),
        "{:?}",
        diag.notes
    );
}

#[test]
fn e4005_for_nested_field_names_dotted_path() {
    let src = "struct port Chan { out data : u8\n in ready : bool }\nstruct port Link { out tx : Chan\n in rx : Chan }\nmodule M { in link : Link\n link.tx.data = 0 }";
    let result = analyze_src(src);
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E4005")
        .expect("E4005");
    assert!(diag.message.contains("link.tx.data"), "{}", diag.message);
}

#[test]
fn no_e4005_for_plain_ports() {
    let result = analyze_src("module M { in a : u8\n out b : u8\n b = a }");
    assert!(!result.error_codes().contains(&"E4005"));
}

// ═══ E3013 — domain tutarsızlığı ═══════════════════════════════════

const TWO_DOMAINS: &str = "domain Fast { clock = posedge\n reset = sync active_high }\ndomain Slow { clock = posedge\n reset = sync active_high }\n";

#[test]
fn e3013_when_field_annotations_disagree() {
    let src = format!(
        "{TWO_DOMAINS}struct port Bus {{ out data : u8 @Fast\n in ready : bool @Slow }}\nmodule M {{ in f : clock @Fast\n in s : clock @Slow\n out bus : Bus\n reg r : u8 = 0\n on f {{ r <= r + 1 }}\n bus.data = r }}"
    );
    let result = analyze_src(&src);
    assert_eq!(
        result
            .error_codes()
            .iter()
            .filter(|c| **c == "E3013")
            .count(),
        1
    );
}

#[test]
fn e3013_when_field_annotation_conflicts_with_port_annotation() {
    let src = format!(
        "{TWO_DOMAINS}struct port Bus {{ out data : u8 @Slow\n in ready : bool }}\nmodule M {{ in f : clock @Fast\n in s : clock @Slow\n out bus : Bus @Fast\n reg r : u8 = 0\n on s {{ r <= r + 1 }}\n bus.data = r }}"
    );
    let result = analyze_src(&src);
    assert!(
        result.error_codes().contains(&"E3013"),
        "{:?}",
        result.error_codes()
    );
}

#[test]
fn no_e3013_when_whole_port_shares_one_domain() {
    let src = format!(
        "{TWO_DOMAINS}struct port Bus {{ out data : u8\n in ready : bool }}\nmodule M {{ in f : clock @Fast\n in s : clock @Slow\n out bus : Bus @Fast\n reg r : u8 = 0\n on f {{ r <= r + 1 }}\n bus.data = r }}"
    );
    let result = analyze_src(&src);
    assert!(
        !result.error_codes().contains(&"E3013"),
        "{:?}",
        result.error_codes()
    );
}

#[test]
fn no_e3013_in_single_clock_module() {
    let src = format!("{HANDSHAKE}module M {{ in clk : clock\n out hs : Handshake\n hs.data = 0\n hs.valid = true }}");
    let result = analyze_src(&src);
    assert!(
        !result.error_codes().contains(&"E3013"),
        "{:?}",
        result.error_codes()
    );
}

#[test]
fn e3013_message_names_both_domains_and_points_at_port() {
    let src = format!(
        "{TWO_DOMAINS}struct port Bus {{ out data : u8 @Fast\n in ready : bool @Slow }}\nmodule M {{ in f : clock @Fast\n in s : clock @Slow\n out bus : Bus\n reg r : u8 = 0\n on f {{ r <= r + 1 }}\n bus.data = r }}"
    );
    let result = analyze_src(&src);
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E3013")
        .expect("E3013");
    assert!(
        diag.message.contains("@Fast") && diag.message.contains("@Slow"),
        "{}",
        diag.message
    );
    assert!(
        diag.help.as_deref().unwrap_or("").contains("Bus"),
        "{:?}",
        diag.help
    );
    assert!(diag.notes.iter().any(|n| n.text.contains("ADR-0039")));
}

// ═══ Genel: bundle portları sıradan portlar gibi çözülür ══════════

#[test]
fn flattened_ports_resolve_and_typecheck() {
    let src = format!(
        "{HANDSHAKE}module M {{ in clk : clock\n in hs : Handshake\n out sum : u8\n reg acc : u8 = 0\n on clk {{ if hs.valid {{ acc <= acc + hs.data }} }}\n hs.ready = true\n sum = acc }}"
    );
    let result = analyze_src(&src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn undriven_flattened_output_is_e4002() {
    let src = format!("{HANDSHAKE}module M {{ out hs : Handshake\n hs.data = 0 }}");
    let result = analyze_src(&src);
    assert!(
        result.error_codes().contains(&"E4002"),
        "{:?}",
        result.error_codes()
    );
}
