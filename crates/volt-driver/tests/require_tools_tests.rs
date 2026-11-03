//! ADR-0079 §3: "araç yoksa atla" deseni CI'da sessizce yeşil kalmasın.
//! `VOLT_REQUIRE_TOOLS` içindeki araç bulunamazsa test atlanmaz, düşer.
//! Kanıt kalıcıdır: bu ikili kendi sonda testini alt süreçte, araçlar
//! PATH'ten çıkarılmış olarak koşturur (aracı gizleme mutasyonu).

mod tools;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tools::Tool;

/// Sondalar yalnız `run_probe`/`probe_cmd`'nin alt sürecinde iş yapar
/// (`VOLT_PROBE=1`): normal koşuda araç sorgusu yapıp deftere satır
/// yazsalardı, araçlı işin sıfır-SKIP kuralı sondanın kendisini sayardı.
fn in_probe() -> bool {
    std::env::var("VOLT_PROBE").is_ok_and(|v| v == "1")
}

/// Alt süreçte koşan sonda: Verilator araması.
#[test]
fn probe_require_verilator() {
    if in_probe() {
        let _ = tools::require(Tool::Verilator);
    }
}

/// Alt süreçte koşan ikinci sonda: `VOLT_PROBE_OPT_IN` açılımı.
#[test]
fn probe_opt_in() {
    if in_probe() {
        let _ = tools::opt_in("VOLT_PROBE_OPT_IN");
    }
}

/// Sondayı Verilator'suz bir ortamda koşturur. Alt süreç bu koşunun
/// defterine (`VOLT_TOOL_LEDGER`) YAZMAZ: sondanın bilinçli atlaması CI'ın
/// SKIP sayısına girmesin.
fn run_probe(require: Option<&str>) -> Output {
    probe_cmd("probe_require_verilator", require, None)
        .output()
        .expect("test ikilisi çalışmalı")
}

fn probe_cmd(test: &str, require: Option<&str>, ledger: Option<&Path>) -> Command {
    let mut cmd = Command::new(std::env::current_exe().expect("test ikilisi"));
    cmd.args(["--exact", test, "--nocapture"])
        .args(["--test-threads", "1"])
        .env("VOLT_PROBE", "1")
        .env("PATH", "")
        .env_remove("VOLT_VERILATOR")
        .env_remove("VOLT_PROBE_OPT_IN")
        .env_remove(tools::REQUIRE_ENV)
        .env_remove(tools::LEDGER_ENV);
    if let Some(value) = require {
        cmd.env(tools::REQUIRE_ENV, value);
    }
    if let Some(path) = ledger {
        cmd.env(tools::LEDGER_ENV, path);
    }
    cmd
}

fn ledger_path(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("require-tools-ledger");
    std::fs::create_dir_all(&dir).expect("temp dizini");
    let path = dir.join(format!("{tag}-{}.tsv", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
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

/// ADR-0107: atlayan test deftere `SKIP` yazar — ikili, test adı ve araç
/// ile; CI bu satırları sayar (`scripts/ci/tool_ledger.py`).
#[test]
fn ledger_records_skip_with_binary_and_test_name() {
    let ledger = ledger_path("skip");
    let out = probe_cmd("probe_require_verilator", None, Some(&ledger))
        .output()
        .expect("test ikilisi çalışmalı");
    assert!(out.status.success(), "{}", text(&out));
    assert_eq!(
        read(&ledger),
        "SKIP	require_tools_tests	probe_require_verilator	verilator	0
"
    );
    let _ = std::fs::remove_file(&ledger);
}

/// Araç bulunursa `RAN` ve zorunluluk bilgisi yazılır: araçsız işte
/// görülen test, araçlı işte aracı ZORUNLUYKEN koşmuş olmalı.
#[test]
fn ledger_records_ran_and_whether_the_tool_was_required() {
    let fake = std::env::current_exe().expect("test ikilisi");
    for (require, flag) in [(None, "0"), (Some("verilator"), "1")] {
        let ledger = ledger_path(&format!("ran-{flag}"));
        let out = probe_cmd("probe_require_verilator", require, Some(&ledger))
            .env("VOLT_VERILATOR", &fake)
            .output()
            .expect("test ikilisi çalışmalı");
        assert!(out.status.success(), "{}", text(&out));
        assert_eq!(
            read(&ledger),
            format!(
                "RAN	require_tools_tests	probe_require_verilator	verilator	{flag}
"
            )
        );
        let _ = std::fs::remove_file(&ledger);
    }
}

/// Ortam değişkeniyle açılan ağır test (Docker uçtan uca) de sayılır.
#[test]
fn ledger_records_opt_in_gates() {
    for (value, line) in [
        (
            None,
            "SKIP	require_tools_tests	probe_opt_in	VOLT_PROBE_OPT_IN	0
",
        ),
        (
            Some("1"),
            "RAN	require_tools_tests	probe_opt_in	VOLT_PROBE_OPT_IN	1
",
        ),
    ] {
        let ledger = ledger_path(&format!("opt-in-{}", value.unwrap_or("0")));
        let mut cmd = probe_cmd("probe_opt_in", None, Some(&ledger));
        if let Some(v) = value {
            cmd.env("VOLT_PROBE_OPT_IN", v);
        }
        let out = cmd.output().expect("test ikilisi çalışmalı");
        assert!(out.status.success(), "{}", text(&out));
        assert_eq!(read(&ledger), line);
        let _ = std::fs::remove_file(&ledger);
    }
}
