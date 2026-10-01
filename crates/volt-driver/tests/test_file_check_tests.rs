//! `volt check` test dosyasının test bloklarını da denetler: kardeş
//! `X.volt` (DUT) yüklenir, `volt test` ile aynı tam denetim (E8501-E8512)
//! koşar. Önceden `dut.countx` geçiyor ve "volt build X_test.volt"
//! öneriliyordu. Verilator gerekmez.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-tcheck-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    // `.git` tavanı: Volt.toml araması bu dizinin üstüne çıkmaz (ADR-0061).
    std::fs::create_dir_all(dir.join(".git")).expect(".git");
    dir
}

const COUNTER: &str = "pub module Counter {
    in  clk    : clock
    in  enable : bool
    out count  : u8

    reg count_r : u8 = 0
    on clk {
        if enable {
            count_r <= count_r + 1
        }
    }
    count = count_r
}
";

const GOOD_TEST: &str = "test \"counts\" {
    let dut = Counter { };
    dut.enable = true;
    step(3);
    assert_eq(dut.count, 3);
}
";

/// Kardeş `counter.volt` + `counter_test.volt` (`test` içeriği); `manifest`
/// ise Volt.toml da yazılır.
fn setup(tag: &str, test: &str, manifest: bool) -> PathBuf {
    let dir = temp_dir(tag);
    std::fs::write(dir.join("counter.volt"), COUNTER).expect("yaz");
    std::fs::write(dir.join("counter_test.volt"), test).expect("yaz");
    if manifest {
        std::fs::write(
            dir.join("Volt.toml"),
            "[package]\nname = \"p\"\nsrc = \".\"\ntop = \"Counter\"\n",
        )
        .expect("yaz");
    }
    dir
}

fn volt(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(args)
        .current_dir(dir)
        .env("VOLT_LANG", "en")
        .env_remove("VOLT_MANIFEST_DIR")
        .output()
        .expect("volt çalışmalı")
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn check_of_a_test_file_reports_an_unknown_port() {
    let dir = setup(
        "unknown",
        &GOOD_TEST.replace("dut.count,", "dut.countx,"),
        false,
    );
    let out = volt(&dir, &["check", "counter_test.volt"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(err.contains("error[E8502]"), "{err}");
    assert!(err.contains("'countx'"), "{err}");
    // Tek tanı: `volt test` ile aynı denetim bir kez sayılır.
    assert_eq!(err.matches("error[E8502]").count(), 1, "{err}");
    assert!(err.contains("Result 1 error(s)"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_of_a_clean_test_file_suggests_volt_test() {
    let dir = setup("next", GOOD_TEST, false);
    let out = volt(&dir, &["check", "counter_test.volt"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(0), "{err}");
    assert!(
        err.contains("Next: volt test counter_test.volt   (run the tests)"),
        "{err}"
    );
    assert!(!err.contains("volt build counter_test.volt"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_of_a_design_file_still_suggests_volt_build() {
    let dir = setup("design", GOOD_TEST, false);
    let out = volt(&dir, &["check", "counter.volt"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(0), "{err}");
    assert!(err.contains("Next: volt build counter.volt"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn project_check_covers_test_files() {
    let dir = setup(
        "project",
        &GOOD_TEST.replace("dut.count,", "dut.countx,"),
        true,
    );
    let out = volt(&dir, &["check"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(err.contains("Checking counter.volt"), "{err}");
    assert!(err.contains("Checking counter_test.volt"), "{err}");
    assert_eq!(err.matches("error[E8502]").count(), 1, "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn clean_project_check_lists_the_test_file_once() {
    let dir = setup("project-ok", GOOD_TEST, true);
    let out = volt(&dir, &["check"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(0), "{err}");
    assert_eq!(
        err.matches("Checking counter_test.volt").count(),
        1,
        "{err}"
    );
    assert!(err.contains("Result 0 error(s), 0 warning(s)"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}
