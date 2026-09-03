//! volt CLI entegrasyon testleri — cli-contract.md §2 çıkış kodları,
//! §5 build/format, F2c aşamalı anlamsal boru hattı (CDC dahil).

use std::path::PathBuf;
use std::process::Command;

fn volt() -> Command {
    Command::new(env!("CARGO_BIN_EXE_volt"))
}

fn fixtures() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures"))
}

fn ui(rel: &str) -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/ui")).join(rel)
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-cli-test-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

#[test]
fn build_counter_succeeds_and_matches_expected() {
    let target = temp_dir("build-ok");
    let output = volt()
        .args(["build", "--target-dir"])
        .arg(&target)
        .arg(fixtures().join("counter.volt"))
        .output()
        .expect("volt çalışmalı");

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let sv_path = target.join("rtl").join("counter.sv");
    let produced = std::fs::read_to_string(&sv_path).expect("counter.sv üretilmeli");
    let expected =
        std::fs::read_to_string(fixtures().join("counter.expected.sv")).expect("beklenen");
    assert_eq!(produced, expected, "CLI çıktısı da birebir eşleşmeli");

    // cli-contract.md §5 ilerleme mesajları (stderr'de, §11)
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Derleniyor"), "stderr: {stderr}");
    assert!(stderr.contains("Tamamlandı"));
    assert!(stderr.contains("Çıktı"));
    assert!(stderr.contains("counter.sv"));
    assert!(stderr.contains("satır)"));

    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn build_missing_file_is_io_error_exit_3() {
    let output = volt()
        .args(["build", "boyle-bir-dosya-yok.volt"])
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(3));
}

#[test]
fn build_compile_error_exit_1() {
    let target = temp_dir("build-err");
    let bad = target.join("bozuk.volt");
    std::fs::write(&bad, "module M { in a : }").expect("yazılmalı");

    let output = volt()
        .args(["build", "--target-dir"])
        .arg(&target)
        .arg(&bad)
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("derleme başarısız"), "stderr: {stderr}");
    assert!(
        !target.join("rtl").join("bozuk.sv").exists(),
        "hatalı build çıktı üretmemeli"
    );

    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn usage_error_exit_2() {
    let output = volt()
        .args(["build", "--boyle-bayrak-yok"])
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn check_counter_exit_0() {
    let output = volt()
        .arg("check")
        .arg(fixtures().join("counter.volt"))
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Sonuç 0 hata"), "stderr: {stderr}");
}

// ═══ F2c: CDC kontrolü CLI'da (Volt'un vaadi) ═════════════════════

#[test]
fn check_cdc_violation_exit_1_with_e3001() {
    let output = volt()
        .arg("check")
        .arg(ui("fail/01_cdc_violation.volt"))
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("E3001"), "stderr: {stderr}");
    // 5 parça: çözüm satırı sync() önermeli.
    assert!(stderr.contains("= çözüm"), "stderr: {stderr}");
    assert!(stderr.contains("sync("), "stderr: {stderr}");
}

#[test]
fn check_single_clock_pass_exit_0_no_diagnostics() {
    let output = volt()
        .arg("check")
        .arg(ui("pass/14_single_clock_no_domain.volt"))
        .output()
        .expect("volt çalışmalı");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Sonuç 0 hata, 0 uyarı"), "stderr: {stderr}");
    // UX Anayasası: hiçbir tanı yok — kullanıcı 'domain' kavramını
    // görmez (dosya YOLU 'no_domain' içerdiğinden tanı satırı sayılır).
    assert!(!stderr.contains("error["), "stderr: {stderr}");
    assert!(!stderr.contains("warning["), "stderr: {stderr}");
}

#[test]
fn check_cdc_bridge_with_sync_exit_0() {
    let output = volt()
        .arg("check")
        .arg(ui("pass/13_cdc_correct_bridge.volt"))
        .output()
        .expect("volt çalışmalı");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Sonuç 0 hata"), "stderr: {stderr}");
}

#[test]
fn build_cdc_violation_exit_1_no_sv_output() {
    let target = temp_dir("build-cdc");
    let output = volt()
        .args(["build", "--target-dir"])
        .arg(&target)
        .arg(ui("fail/01_cdc_violation.volt"))
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("E3001"), "stderr: {stderr}");
    assert!(stderr.contains("derleme başarısız"), "stderr: {stderr}");
    assert!(
        !target.join("rtl").join("01_cdc_violation.sv").exists(),
        "CDC ihlali SV üretmemeli — Volt'un vaadi"
    );

    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn check_typeck_error_via_cli_e2002() {
    // Aşama 3 (tip kontrolü) CLI'dan da çalışıyor.
    let output = volt()
        .arg("check")
        .arg(ui("fail/08_signedness_mismatch.volt"))
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("E2002"), "stderr: {stderr}");
}

#[test]
fn stage_gating_resolve_error_stops_pipeline() {
    // E1001 (aşama 2) varken sonraki aşamaların kodları görünmemeli.
    let output = volt()
        .arg("check")
        .arg(ui("fail/19_undefined_name.volt"))
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("E1001"), "stderr: {stderr}");
    assert!(
        !stderr.contains("error[E2") && !stderr.contains("error[E3"),
        "kaskad tanı olmamalı: {stderr}"
    );
}

// ═══ --format=json / --format=short (cli-contract.md §5) ══════════

#[test]
fn check_json_format_cdc_violation() {
    let output = volt()
        .args(["check", "--format", "json"])
        .arg(ui("fail/01_cdc_violation.volt"))
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(1));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let envelope: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout geçerli JSON olmalı");
    assert_eq!(envelope["version"], "1");
    assert_eq!(envelope["command"], "check");
    assert_eq!(envelope["success"], false);
    // Hatalar önce sıralanır: CI ilk kayıtta engelleyiciyi görür.
    assert_eq!(envelope["diagnostics"][0]["code"], "E3001");
    assert_eq!(envelope["diagnostics"][0]["severity"], "error");
    assert_eq!(envelope["summary"]["errors"], 1);
    assert!(envelope["diagnostics"][0]["explain_url"]
        .as_str()
        .unwrap()
        .contains("E3001"));
}

#[test]
fn check_json_format_clean_file() {
    let output = volt()
        .args(["check", "--format", "json"])
        .arg(ui("pass/14_single_clock_no_domain.volt"))
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(0));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let envelope: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout geçerli JSON olmalı");
    assert_eq!(envelope["success"], true);
    assert_eq!(envelope["summary"]["errors"], 0);
    assert_eq!(envelope["diagnostics"].as_array().unwrap().len(), 0);
}

#[test]
fn build_json_format_lists_artifact() {
    let target = temp_dir("build-json");
    let output = volt()
        .args(["build", "--format", "json", "--target-dir"])
        .arg(&target)
        .arg(fixtures().join("counter.volt"))
        .output()
        .expect("volt çalışmalı");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let envelope: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout geçerli JSON olmalı");
    assert_eq!(envelope["command"], "build");
    assert_eq!(envelope["success"], true);
    let artifacts = envelope["artifacts"].as_array().unwrap();
    assert_eq!(artifacts.len(), 1);
    assert!(artifacts[0].as_str().unwrap().contains("counter.sv"));

    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn check_short_format_single_line_diagnostics() {
    let output = volt()
        .args(["check", "--format", "short"])
        .arg(ui("fail/01_cdc_violation.volt"))
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    // dosya:satır:sütun: error[E3001]: mesaj — tek satır.
    assert!(
        stderr.lines().any(|l| l.contains(":21:5: error[E3001]:")),
        "stderr: {stderr}"
    );
}
