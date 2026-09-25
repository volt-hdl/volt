//! ADR-0079 §3: "araç yoksa atla" deseni CI'da sessizce yeşil kalmasın.
//! `VOLT_REQUIRE_TOOLS` içindeki araç bulunamazsa test atlanmaz, düşer.
//! Kanıt kalıcıdır: bu ikili kendi sonda testini alt süreçte, araçlar
//! PATH'ten çıkarılmış olarak koşturur (aracı gizleme mutasyonu).

mod tools;

use std::process::{Command, Output};

use tools::Tool;

/// Alt süreçte koşan sonda; normal koşuda da zararsızdır (araç varsa
/// bulur, yoksa ve zorunlu değilse atlar).
#[test]
fn probe_require_verilator() {
    let _ = tools::require(Tool::Verilator);
}

/// Sondayı Verilator'suz bir ortamda koşturur.
fn run_probe(require: Option<&str>) -> Output {
    let mut cmd = Command::new(std::env::current_exe().expect("test ikilisi"));
    cmd.args(["--exact", "probe_require_verilator", "--nocapture"])
        .args(["--test-threads", "1"])
        .env("PATH", "")
        .env_remove("VOLT_VERILATOR")
        .env_remove(tools::REQUIRE_ENV);
    if let Some(value) = require {
        cmd.env(tools::REQUIRE_ENV, value);
    }
    cmd.output().expect("test ikilisi çalışmalı")
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn missing_tool_skips_only_when_not_required() {
    let skip = run_probe(None);
    assert!(skip.status.success(), "{}", text(&skip));
    assert!(text(&skip).contains("SKIP: verilator"), "{}", text(&skip));

    let fail = run_probe(Some("sby, verilator"));
    assert!(
        !fail.status.success(),
        "zorunlu araç yokken geçti:\n{}",
        text(&fail)
    );
    assert!(text(&fail).contains("zorunlu kılıyor"), "{}", text(&fail));

    let all = run_probe(Some("all"));
    assert!(!all.status.success(), "{}", text(&all));

    // Yazım hatası hiçbir aracı sessizce zorunluluktan çıkarmaz.
    let typo = run_probe(Some("verilater"));
    assert!(!typo.status.success(), "{}", text(&typo));
    assert!(text(&typo).contains("bilinmeyen araç"), "{}", text(&typo));
}

#[test]
fn require_list_parses_names_and_all() {
    assert_eq!(tools::parse_required("").unwrap(), []);
    assert_eq!(
        tools::parse_required("verilator, cc").unwrap(),
        [Tool::Verilator, Tool::Cc]
    );
    assert_eq!(tools::parse_required("all").unwrap().len(), Tool::ALL.len());
    assert!(tools::parse_required("yosys,gcc").is_err());
}
