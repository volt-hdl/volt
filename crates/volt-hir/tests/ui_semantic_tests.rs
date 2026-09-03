//! tests/ui dosyalarının anlamsal (F1b) doğrulaması.
//!
//! Parser seviyesi ui taraması volt-syntax'ta; burada isim çözümleme
//! ve const eval'in ui/fail beklentileri denetlenir.

use volt_hir::analyze;
use volt_span::SourceMap;
use volt_syntax::{parse, FileId};

fn analyze_file(rel: &str) -> volt_hir::AnalysisResult {
    let path = format!("{}/../../tests/ui/{rel}", env!("CARGO_MANIFEST_DIR"));
    let src = std::fs::read_to_string(&path).expect("ui dosyası okunmalı");
    let parsed = parse(FileId(0), &src);
    assert!(
        parsed.diagnostics.is_empty(),
        "{rel} ayrışmalı: {:?}",
        parsed.error_codes()
    );
    analyze(&parsed.ast)
}

/// Fixture anotasyonlarını doğrular — error-recovery.md §8.1 kural 4:
/// yalnız hata KODU değil, hatanın SATIRI da eşleşmeli.
///
/// Kalıp: satır 1 `//~ EXXXX` beklenen kodu verir; `//~^ ERROR ...`
/// satırı bir üstündeki satırda o kodun raporlanmasını bekler.
fn assert_ui_fail(rel: &str) {
    let path = format!("{}/../../tests/ui/{rel}", env!("CARGO_MANIFEST_DIR"));
    let src = std::fs::read_to_string(&path).expect("ui dosyası okunmalı");

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
        .map(|i| i as u32) // position 0-tabanlı → bir üst satır = i (1-tabanlı)
        .expect("fixture '//~^ ERROR' anotasyonu içermeli");

    let parsed = parse(FileId(0), &src);
    assert!(
        parsed.diagnostics.is_empty(),
        "{rel} ayrışmalı: {:?}",
        parsed.error_codes()
    );
    let result = analyze(&parsed.ast);

    let mut map = SourceMap::new();
    map.add_file(rel, src.clone());
    let found: Vec<(u32, u32)> = result
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == expected_code)
        .filter_map(|d| d.primary_span())
        .map(|s| map.line_col_utf8(s.span))
        .collect();
    assert!(
        !found.is_empty(),
        "{rel}: {expected_code} bekleniyor, bulunan: {:?}",
        result.error_codes()
    );
    assert!(
        found.iter().any(|&(line, _)| line == expected_line),
        "{rel}: {expected_code} satır {expected_line} bekleniyor, \
         raporlanan konumlar: {found:?}"
    );
}

#[test]
fn ui_fail_19_undefined_name_e1001_with_help() {
    let result = analyze_file("fail/19_undefined_name.volt");
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E1001")
        .expect("E1001 bekleniyor");
    // 5 parça kuralı: çözüm önerisi her tanıda zorunlu.
    assert!(
        !diag.help.as_deref().unwrap_or("").is_empty(),
        "E1001 yardım metni taşımalı"
    );
    assert_ui_fail("fail/19_undefined_name.volt");
}

#[test]
fn ui_fail_21_cyclic_const_e2020() {
    assert_ui_fail("fail/21_cyclic_const.volt");
}

#[test]
fn ui_fail_22_runtime_in_type_e2021() {
    assert_ui_fail("fail/22_runtime_in_type.volt");
}

#[test]
fn ui_fail_03_double_driver_e4001() {
    assert_ui_fail("fail/03_double_driver.volt");
}

#[test]
fn ui_fail_04_undriven_output_e4002() {
    // Satır kontrolü BİLEREK yok: fixture'daki `//~^` kapanış parantezini
    // (satır 11) işaret ediyor; tanı ise doğru olarak port bildirimini
    // (satır 7) gösteriyor. Fixture anotasyonu taşınana kadar yalnız kod
    // denetlenir — tests/ui bu turda salt okunur.
    let result = analyze_file("fail/04_undriven_output.volt");
    assert!(
        result.error_codes().contains(&"E4002"),
        "E4002 bekleniyor: {:?}",
        result.error_codes()
    );
}

#[test]
fn ui_fail_02_width_mismatch_e2001() {
    assert_ui_fail("fail/02_width_mismatch.volt");
}

#[test]
fn ui_fail_08_signedness_mismatch_e2002() {
    assert_ui_fail("fail/08_signedness_mismatch.volt");
}

#[test]
fn ui_fail_09_bits_arithmetic_e2004() {
    assert_ui_fail("fail/09_bits_arithmetic.volt");
}

#[test]
fn ui_fail_10_index_out_of_bounds_e2006() {
    assert_ui_fail("fail/10_index_out_of_bounds.volt");
}

#[test]
fn ui_fail_11_reversed_range_e2007() {
    assert_ui_fail("fail/11_reversed_range.volt");
}

#[test]
fn ui_fail_12_invalid_cast_to_trit_e2009() {
    assert_ui_fail("fail/12_invalid_cast_to_trit.volt");
}

#[test]
fn ui_fail_17_literal_overflow_e2010() {
    assert_ui_fail("fail/17_literal_overflow.volt");
}

#[test]
fn ui_fail_20_reg_type_ambiguous_e2012() {
    assert_ui_fail("fail/20_reg_type_ambiguous.volt");
}

// ═══ Domain çıkarımı ve CDC (F2c, domain-inference.md) ════════════

#[test]
fn ui_fail_01_cdc_violation_e3001() {
    // VOLT'UN VAADİ: CDC hatası derlenmemeli.
    assert_ui_fail("fail/01_cdc_violation.volt");
}

#[test]
fn ui_fail_07_unknown_domain_e3002() {
    assert_ui_fail("fail/07_unknown_domain.volt");
}

#[test]
fn ui_fail_13_ambiguous_domain_e3010() {
    assert_ui_fail("fail/13_ambiguous_domain.volt");
}

#[test]
fn ui_fail_14_combinational_cdc_e3001() {
    assert_ui_fail("fail/14_combinational_cdc.volt");
}

#[test]
fn ui_fail_15_register_two_domains_e3011() {
    assert_ui_fail("fail/15_register_two_domains.volt");
}

#[test]
fn ui_pass_13_cdc_bridge_clean_with_sync() {
    // sync() köprüsü hatasız geçmeli; hiçbir domain tanısı (E3xxx/W3xxx)
    // üretilmemeli (K9). W1001 (kullanılmayan fast_clk) domain dışıdır.
    let result = analyze_file("pass/13_cdc_correct_bridge.volt");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
    assert!(
        !result
            .error_codes()
            .iter()
            .any(|c| c.starts_with("E3") || c.starts_with("W3")),
        "domain tanısı olmamalı: {:?}",
        result.error_codes()
    );
}

#[test]
fn ui_pass_14_single_clock_no_domain_word() {
    // UX Anayasası: tek saatli tasarımda 'domain' hiç görünmez (K2).
    let result = analyze_file("pass/14_single_clock_no_domain.volt");
    assert!(
        result.diagnostics.is_empty(),
        "temiz geçmeli: {:?}",
        result.error_codes()
    );
}

#[test]
fn ui_pass_files_have_no_semantic_errors() {
    // Uyarılar serbest; hatalar regresyondur.
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/ui/pass");
    let mut checked = 0;
    for entry in std::fs::read_dir(dir).expect("ui/pass okunmalı") {
        let path = entry.expect("girdi").path();
        if path.extension().and_then(|e| e.to_str()) != Some("volt") {
            continue;
        }
        let src = std::fs::read_to_string(&path).expect("dosya okunmalı");
        let parsed = parse(FileId(0), &src);
        assert!(parsed.diagnostics.is_empty());
        let result = analyze(&parsed.ast);
        assert!(
            !result.has_errors(),
            "{:?} anlamsal hata üretmemeli: {:?}",
            path.file_name(),
            result.error_codes()
        );
        checked += 1;
    }
    assert_eq!(checked, 21);
}
