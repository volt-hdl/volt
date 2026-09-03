//! volt CLI entegrasyon testleri — cli-contract.md §2 çıkış kodları, §5 build.

use std::path::PathBuf;
use std::process::Command;

fn volt() -> Command {
    Command::new(env!("CARGO_BIN_EXE_volt"))
}

fn fixtures() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures"))
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
