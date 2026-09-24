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

// ═══ Yerleşik Handshake<T> (ADR-0050) ═════════════════════════════

#[test]
fn ui_pass_66_handshake_basic_clean() {
    let (_, result) = analyze_file("pass/66_handshake_basic.volt");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn ui_pass_67_handshake_contracts_clean() {
    let (_, result) = analyze_file("pass/67_handshake_contracts.volt");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn ui_fail_52_handshake_protocol_violation_e4007() {
    assert_ui_fail("fail/52_handshake_protocol_violation.volt");
}

/// Yalnız hatalar (uyarılar — kullanılmayan clk gibi — dışarıda).
fn errors(result: &volt_hir::AnalysisResult) -> Vec<&str> {
    result
        .error_codes()
        .into_iter()
        .filter(|c| c.starts_with('E'))
        .collect()
}

const PRODUCER_HEAD: &str = "module P {\n    in  clk : clock\n    in  have : bool\n    out tx : Handshake<u8>\n    tx.data = 0\n";

#[test]
fn e4007_direct_valid_from_ready() {
    let result = analyze_src(&format!(
        "{PRODUCER_HEAD}    tx.valid = tx.ready && have\n}}"
    ));
    assert_eq!(errors(&result), vec!["E4007"]);
}

#[test]
fn e4007_through_let_chain() {
    let src = format!(
        "{PRODUCER_HEAD}    let a : bool = tx.ready\n    let b : bool = a && have\n    tx.valid = b\n}}"
    );
    let result = analyze_src(&src);
    assert_eq!(errors(&result), vec!["E4007"]);
}

#[test]
fn e4007_through_comb_block_condition() {
    let src = format!(
        "{PRODUCER_HEAD}    comb {{\n        if tx.ready {{ tx.valid = have }} else {{ tx.valid = false }}\n    }}\n}}"
    );
    let result = analyze_src(&src);
    assert_eq!(errors(&result), vec!["E4007"]);
}

#[test]
fn e4007_not_raised_when_valid_is_registered() {
    let src = format!(
        "{PRODUCER_HEAD}    reg valid_r : bool = false\n    on clk {{\n        if tx.fired {{ valid_r <= false }} else if have {{ valid_r <= true }}\n    }}\n    tx.valid = valid_r\n}}"
    );
    let result = analyze_src(&src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn e4007_not_raised_for_consumer_ready_from_valid() {
    let src = "module C {\n    in  clk : clock\n    in  rx : Handshake<u8>\n    out d : u8\n    rx.ready = rx.valid\n    d = rx.data\n}";
    let result = analyze_src(src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn e4007_has_five_parts_and_names_the_path() {
    let src = format!("{PRODUCER_HEAD}    let go : bool = have && tx.ready\n    tx.valid = go\n}}");
    let result = analyze_src(&src);
    let d = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E4007")
        .expect("E4007");
    assert!(d.message.contains("'tx'"), "{}", d.message);
    assert!(d.help.as_deref().is_some_and(|h| h.contains("valid_r")));
    assert!(
        d.spans.len() >= 3,
        "primary + ready okuması + port: {:?}",
        d.spans.len()
    );
    assert!(
        d.notes
            .iter()
            .any(|n| n.text.contains("tx.valid <- go <- tx_ready")),
        "{:?}",
        d.notes
    );
}

#[test]
fn auto_contracts_use_prev_without_e5017() {
    // Sentezlenen kontratlar kontrat bağlamında çözümlenir: prev() serbest.
    let src = format!(
        "{PRODUCER_HEAD}    reg v : bool = false\n    on clk {{ v <= have }}\n    tx.valid = v\n}}"
    );
    let result = analyze_src(&src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
    assert!(!result.error_codes().contains(&"E5017"));
}

#[test]
fn auto_contracts_count_as_reads_of_ready() {
    // Üretici ready'yi hiç okumasa da otomatik kontrat okur: W1001 yok.
    let src = format!("{PRODUCER_HEAD}    tx.valid = have\n}}");
    let result = analyze_src(&src);
    let unused_ready = |r: &volt_hir::AnalysisResult| {
        r.diagnostics
            .iter()
            .any(|d| d.code.as_str() == "W1001" && d.message.contains("tx.ready"))
    };
    assert!(!unused_ready(&result), "{:?}", result.error_codes());
    // @no_protocol_check ile kontrat yok: tx_ready gerçekten okunmuyor.
    let src = "module P {\n    in  clk : clock\n    in  have : bool\n    @no_protocol_check\n    out tx : Handshake<u8>\n    tx.data = 0\n    tx.valid = have\n}";
    let result = analyze_src(src);
    assert!(unused_ready(&result), "{:?}", result.error_codes());
}

#[test]
fn consumer_driving_valid_is_e4005() {
    let src = "module C {\n    in  clk : clock\n    in  rx : Handshake<u8>\n    rx.valid = true\n    rx.ready = true\n}";
    let result = analyze_src(src);
    assert!(
        result.error_codes().contains(&"E4005"),
        "{:?}",
        result.error_codes()
    );
}

#[test]
fn struct_payload_fields_type_check() {
    let src = "struct Req { addr : u32, prot : u3 }\nmodule C {\n    in  clk : clock\n    in  req : Handshake<Req>\n    out a : u32\n    req.ready = req.data.prot == 0\n    a = req.data.addr\n}";
    let result = analyze_src(src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn no_protocol_check_attribute_is_enforced_no_w0021() {
    let src = "@no_protocol_check\nmodule P {\n    in  clk : clock\n    out tx : Handshake<u8>\n    tx.data = 0\n    tx.valid = tx.ready\n}";
    let result = analyze_src(src);
    // Nitelik uygulanıyor (W0021 yok) ama E4007 kapanmaz: yapısal kural.
    assert!(
        !result.error_codes().contains(&"W0021"),
        "{:?}",
        result.error_codes()
    );
    assert!(
        result.error_codes().contains(&"E4007"),
        "{:?}",
        result.error_codes()
    );
}

// ═══ Kaynak adı (ADR-0075) ════════════════════════════════════════

fn messages(result: &volt_hir::AnalysisResult) -> Vec<String> {
    result
        .diagnostics
        .iter()
        .map(|d| format!("{} {}", d.code.as_str(), d.message))
        .collect()
}

/// Bundle alanı tanılarda düzleştirilmiş adla (`hs_data`) değil kaynak
/// yoluyla (`hs.data`) görünür — W1001, E4001, E4002.
#[test]
fn bundle_field_diagnostics_use_the_source_path() {
    let unused = analyze_src(&format!(
        "{HANDSHAKE}module C {{\n    in  _clk : clock\n    in  hs  : Handshake\n    out y   : u8\n    hs.ready = true\n    y = 0\n}}\n"
    ));
    let m = messages(&unused);
    assert!(
        m.contains(&"W1001 unused input port: 'hs.data'".to_string()),
        "{m:?}"
    );
    assert!(m.iter().all(|s| !s.contains("hs_")), "{m:?}");
    let w = unused
        .diagnostics
        .iter()
        .find(|d| d.message.contains("hs.data"))
        .unwrap();
    let help = w.help.as_deref().unwrap_or_default();
    assert!(help.ends_with(": _hs"), "{help}");

    let double = analyze_src(&format!(
        "{HANDSHAKE}module P {{\n    in  _clk : clock\n    out hs  : Handshake\n    hs.data = 1\n    hs.data = 2\n    hs.valid = hs.ready\n}}\n"
    ));
    let m = messages(&double);
    assert!(
        m.contains(&"E4001 'hs.data' is already driven".to_string()),
        "{m:?}"
    );

    let undriven = analyze_src(&format!(
        "{HANDSHAKE}module P {{\n    in  _clk : clock\n    out hs  : Handshake\n    hs.valid = hs.ready\n}}\n"
    ));
    let m = messages(&undriven);
    assert!(
        m.contains(&"E4002 output port 'hs.data' is not driven".to_string()),
        "{m:?}"
    );
    let e = undriven
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E4002")
        .unwrap();
    let help = e.help.as_deref().unwrap_or_default();
    assert!(help.contains("hs.data = ..."), "{help}");
}

/// `_` önekli bundle portu bütün alanlarını susturur (öneri bunu söyler).
#[test]
fn underscore_bundle_port_silences_all_fields() {
    let r = analyze_src(&format!(
        "{HANDSHAKE}module C {{\n    in  _clk : clock\n    in  _hs : Handshake\n    out y   : u8\n    _hs.ready = true\n    y = 0\n}}\n"
    ));
    assert!(r.diagnostics.is_empty(), "{:?}", messages(&r));
}
