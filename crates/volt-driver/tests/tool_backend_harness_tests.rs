//! Test düzeninde araç arka ucu açıktır: `volt test`, `volt run` ya da
//! `volt verify` çalıştıran her test dosyası ve betik `VOLT_TOOL_BACKEND`
//! değerini kendisi verir (`local`, `docker` ya da otomatik geri düşüşü
//! sınayan testte bilerek `auto`). Sahte araç yolu bozulduğunda Verilator
//! ya da sby bulunamaz; otomatik geri düşüş o zaman sessizce gerçek
//! Docker konteynerleri başlatırdı (bir koşuda 6 konteyner). Açık `local`
//! ile test aracın yokluğunu açıkça bildirir.

use std::path::{Path, PathBuf};

fn repo(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

/// Dosya `volt`u araç isteyen bir komutla çalıştırıyor mu?
fn runs_tools(text: &str) -> bool {
    text.contains("CARGO_BIN_EXE_volt")
        && ["\"test\"", "\"run\"", "\"verify\""]
            .iter()
            .any(|c| text.contains(c))
}

/// Arka ucu açıkça ayarlıyor mu (ortamdan silmek ayarlamak değildir)?
fn sets_backend(text: &str) -> bool {
    text.contains(".env(\"VOLT_TOOL_BACKEND\"") || text.contains("VOLT_TOOL_BACKEND=")
}

#[test]
fn every_tool_running_test_sets_the_tool_backend() {
    let mut missing = Vec::new();
    let mut files: Vec<PathBuf> = std::fs::read_dir(repo("crates/volt-driver/tests"))
        .expect("tests dizini")
        .map(|e| e.expect("girdi").path())
        .filter(|p| p.extension().is_some_and(|x| x == "rs"))
        .collect();
    files.sort();
    for f in files {
        let text = std::fs::read_to_string(&f).expect("okunmalı");
        if runs_tools(&text) && !sets_backend(&text) {
            missing.push(f.display().to_string());
        }
    }
    assert!(
        missing.is_empty(),
        "araç çalıştıran test arka ucu açıkça ayarlamıyor (.env(\"VOLT_TOOL_BACKEND\", \"local\")): {missing:#?}"
    );
}

#[test]
fn the_book_checker_sets_the_tool_backend() {
    let text = std::fs::read_to_string(repo("book/tools/check_book.py")).expect("check_book.py");
    assert!(
        text.contains("VOLT_TOOL_BACKEND=TOOL_BACKEND"),
        "check_book.py"
    );
}
