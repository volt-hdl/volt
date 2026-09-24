//! `volt verify` sonuç durumları (ADR-0075): SymbiYosys'in beş durumu —
//! PASS, FAIL, UNKNOWN, TIMEOUT, ERROR — ayrı iletiyle ve ayrı çıkış
//! koduyla (0, 6, 7, 8, 3) raporlanır. Önce UNKNOWN (prove kipinde
//! tümevarım tamamlanamadı) "tool error" diye, çıkış 3 ile raporlanıyordu.
//! sby kurulu olmayan ortamda VOLT_SBY üzerinden sahte sby ile uçtan uca;
//! sahte satırlar gerçek sby 0.36 çıktısından (hdlc/formal) alınmıştır.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn volt() -> Command {
    Command::new(env!("CARGO_BIN_EXE_volt"))
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-verify-st-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// Üç kontratlı modül (Alpha, Beta, Gamma), modül başına tek invariant;
/// iş adı `three`, çalışma dizinleri `three_<görev>`.
fn write_three(dir: &Path) -> PathBuf {
    let mut src = String::new();
    for name in ["Alpha", "Beta", "Gamma"] {
        src.push_str(&format!(
            "@no_auto_contracts\nmodule {name} {{\n    in  clk    : clock\n    in  enable : bool\n    out count  : u8\n\n    \
             invariant: count_r <= 10\n\n    reg count_r : u8 = 0\n\n    on clk {{\n        if enable {{\n            \
             if count_r == 10 {{\n                count_r <= 0\n            }} else {{\n                \
             count_r <= count_r + 1\n            }}\n        }}\n    }}\n\n    count = count_r\n}}\n\n"
        ));
    }
    let path = dir.join("three.volt");
    std::fs::write(&path, src).expect("three.volt yazılmalı");
    path
}

enum Step {
    Echo(String),
    /// `three_<görev>/engine_0/trace_induct.vcd` üret (prove UNKNOWN izi).
    InductTrace(&'static str),
}

#[cfg(windows)]
fn write_fake_sby(dir: &Path, steps: &[Step], exit: i32) -> PathBuf {
    let path = dir.join("sby.bat");
    let mut script = String::from("@echo off\r\n");
    for step in steps {
        match step {
            Step::Echo(line) => script.push_str(&format!("echo {line}\r\n")),
            Step::InductTrace(task) => {
                script.push_str(&format!("mkdir three_{task}\\engine_0 2>nul\r\n"));
                script.push_str(&format!(
                    "echo dummy> three_{task}\\engine_0\\trace_induct.vcd\r\n"
                ));
            }
        }
    }
    script.push_str(&format!("exit /b {exit}\r\n"));
    std::fs::write(&path, script).expect("sahte sby yazılmalı");
    path
}

#[cfg(unix)]
fn write_fake_sby(dir: &Path, steps: &[Step], exit: i32) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("sby");
    let mut script = String::from("#!/bin/sh\n");
    for step in steps {
        match step {
            Step::Echo(line) => script.push_str(&format!("echo '{line}'\n")),
            Step::InductTrace(task) => {
                script.push_str(&format!("mkdir -p three_{task}/engine_0\n"));
                script.push_str(&format!(
                    "echo dummy > three_{task}/engine_0/trace_induct.vcd\n"
                ));
            }
        }
    }
    script.push_str(&format!("exit {exit}\n"));
    std::fs::write(&path, script).expect("sahte sby yazılmalı");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    path
}

fn echo(line: &str) -> Step {
    Step::Echo(line.to_string())
}

fn pass(task: &str) -> Step {
    echo(&format!("SBY 16:34:39 [three_{task}] DONE (PASS, rc=0)"))
}

/// Gerçek prove UNKNOWN çıktısının biçimi: temel durum geçer, tümevarım
/// başarısız iddiayı adlandırır, DONE (UNKNOWN, rc=4).
fn unknown(task: &'static str, module: &str) -> Vec<Step> {
    vec![
        Step::InductTrace(task),
        echo(&format!(
            "SBY 16:34:39 [three_{task}] engine_0.induction: ##   0:00:00  Trying induction in step 8.."
        )),
        echo(&format!(
            "SBY 16:34:39 [three_{task}] engine_0.induction: ##   0:00:00  Temporal induction failed!"
        )),
        echo(&format!(
            "SBY 16:34:39 [three_{task}] engine_0.induction: ##   0:00:00  Assert failed in {module}: three.sv:9999.20-9999.42"
        )),
        echo(&format!(
            "SBY 16:34:39 [three_{task}] summary: engine_0 (smtbmc z3) returned pass for basecase"
        )),
        echo(&format!(
            "SBY 16:34:39 [three_{task}] summary: engine_0 (smtbmc z3) returned FAIL for induction"
        )),
        echo(&format!("SBY 16:34:39 [three_{task}] DONE (UNKNOWN, rc=4)")),
    ]
}

fn timeout(task: &str) -> Vec<Step> {
    vec![
        echo(&format!(
            "SBY 16:34:51 [three_{task}] Reached TIMEOUT (5 seconds). Terminating all subprocesses."
        )),
        echo(&format!(
            "SBY 16:34:51 [three_{task}] summary: engine_0 (smtbmc z3) did not return a status"
        )),
        echo(&format!("SBY 16:34:51 [three_{task}] DONE (TIMEOUT, rc=8)")),
    ]
}

fn error(task: &str) -> Step {
    echo(&format!("SBY 16:34:57 [three_{task}] DONE (ERROR, rc=16)"))
}

fn fail(task: &str, module: &str) -> Vec<Step> {
    vec![
        echo(&format!(
            "SBY 12:00:01 [three_{task}] engine_0: ## 0:00:00 Assert failed in {module}: three.sv:9999.1-9999.5"
        )),
        echo(&format!("SBY 12:00:01 [three_{task}] DONE (FAIL, rc=2)")),
    ]
}

fn run(tag: &str, steps: Vec<Step>, sby_exit: i32, extra: &[&str]) -> (Output, PathBuf) {
    let dir = temp_dir(tag);
    let file = write_three(&dir);
    let sby = write_fake_sby(&dir, &steps, sby_exit);
    let out = volt()
        .arg("verify")
        .args(extra)
        .args(["-j", "1", "--target-dir"])
        .arg(&dir)
        .arg(&file)
        .env("VOLT_SBY", &sby)
        .env_remove("VOLT_LANG")
        .output()
        .expect("volt çalışmalı");
    (out, dir)
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn unknown_is_e5002_not_proven_exit_7_not_a_tool_error() {
    let mut steps = vec![pass("alpha")];
    steps.extend(unknown("beta", "Beta"));
    steps.push(pass("gamma"));
    let (out, dir) = run("unknown", steps, 4, &["--mode", "prove"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(7), "{err}");
    assert!(
        err.contains("error[E5002]: contract not proven: the induction step failed"),
        "{err}"
    );
    assert!(err.contains("not inductive at depth 20"), "{err}");
    assert!(
        err.contains(
            "= help: try a larger --depth, or add an invariant that makes the property inductive"
        ),
        "{err}"
    );
    assert!(err.contains("(sby status UNKNOWN)"), "{err}");
    assert!(err.contains("[2/3] Beta (1 property) ... unknown"), "{err}");
    assert!(
        err.contains("Beta.inv_0  E5002 not proven (induction step failed)"),
        "{err}"
    );
    assert!(
        err.contains("Result 0 of 3 properties failed, 1 not proven"),
        "{err}"
    );
    assert!(err.contains("Next: volt explain E5002"), "{err}");
    // Yanıltıcı eski metinler yok: araç hatası değil, karşı örnek değil.
    assert!(!err.contains("tool error"), "{err}");
    assert!(!err.contains("= counterexample:"), "{err}");
    assert!(
        err.contains("induction trace (may start from an unreachable state)"),
        "{err}"
    );
    assert!(
        dir.join("formal").join("beta_induct.vcd").is_file(),
        "tümevarım izi kopyalanmalı"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unknown_turkish_message() {
    let mut steps = vec![pass("alpha")];
    steps.extend(unknown("beta", "Beta"));
    steps.push(pass("gamma"));
    let dir = temp_dir("unknown-tr");
    let file = write_three(&dir);
    let sby = write_fake_sby(&dir, &steps, 4);
    let out = volt()
        .args(["verify", "--lang=tr", "--mode", "prove", "--target-dir"])
        .arg(&dir)
        .arg(&file)
        .env("VOLT_SBY", &sby)
        .output()
        .expect("volt çalışmalı");
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(7), "{err}");
    assert!(
        err.contains("kontrat kanıtlanamadı: tümevarım adımı başarısız"),
        "{err}"
    );
    assert!(
        err.contains("daha büyük bir --depth deneyin ya da özelliği tümevarımsal yapan bir invariant ekleyin"),
        "{err}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn timeout_exit_8_with_its_own_message_and_sby_option() {
    let mut steps = vec![pass("alpha")];
    steps.extend(timeout("beta"));
    steps.push(pass("gamma"));
    let (out, dir) = run("timeout", steps, 8, &["--timeout", "5"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(8), "{err}");
    assert!(
        err.contains("error: SymbiYosys timed out for module 'Beta' after 5s (sby status TIMEOUT)"),
        "{err}"
    );
    assert!(err.contains("neither proven nor refuted"), "{err}");
    assert!(err.contains("= help: raise --timeout"), "{err}");
    assert!(
        err.contains("Beta  timeout (no result; see --timeout)"),
        "{err}"
    );
    assert!(err.contains("1 module task(s) timed out"), "{err}");
    assert!(!err.contains("tool error"), "{err}");
    let sby = std::fs::read_to_string(dir.join("formal").join("three.sby")).expect(".sby");
    assert!(sby.contains("\ndepth 20\ntimeout 5\n"), "{sby}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn timeout_flag_rejects_zero_as_usage_error() {
    let out = volt()
        .args(["verify", "--timeout", "0", "x.volt"])
        .output()
        .expect("volt çalışmalı");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn without_timeout_flag_sby_has_no_timeout_line() {
    let (out, dir) = run(
        "no-timeout",
        vec![pass("alpha"), pass("beta"), pass("gamma")],
        0,
        &[],
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let sby = std::fs::read_to_string(dir.join("formal").join("three.sby")).expect(".sby");
    assert!(!sby.contains("timeout"), "{sby}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn error_exit_3_names_the_sby_status_and_says_nothing_about_contracts() {
    let (out, dir) = run(
        "error",
        vec![pass("alpha"), error("beta"), pass("gamma")],
        16,
        &[],
    );
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(3), "{err}");
    assert!(
        err.contains("tool error for module 'Beta' (sby status ERROR, exit code 16)"),
        "{err}"
    );
    assert!(
        err.contains("this says nothing about the contracts"),
        "{err}"
    );
    assert!(err.contains("Beta  tool error"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn json_reports_each_status_separately() {
    let mut steps = vec![pass("alpha")];
    steps.extend(unknown("beta", "Beta"));
    steps.extend(timeout("gamma"));
    let (out, dir) = run("json", steps, 12, &["--format", "json", "--mode", "prove"]);
    // Zaman aşımı kanıtlanamamaya baskın (8 > 7).
    assert_eq!(out.status.code(), Some(8), "{}", stderr(&out));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("JSON zarfı");
    let statuses = |key: &str| -> Vec<String> {
        v["verify"][key]
            .as_array()
            .expect("dizi")
            .iter()
            .map(|m| m["status"].as_str().unwrap_or_default().to_string())
            .collect()
    };
    assert_eq!(statuses("modules"), ["pass", "unknown", "timeout"]);
    assert_eq!(statuses("properties"), ["pass", "unknown", "timeout"]);
    assert_eq!(v["diagnostics"][0]["code"], "E5002");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Öncelik: karşı örnek (6) > araç hatası (3) > zaman aşımı (8) >
/// kanıtlanamadı (7).
#[test]
fn exit_code_precedence_follows_the_most_definite_result() {
    let mut s = fail("alpha", "Alpha");
    s.push(error("beta"));
    s.extend(unknown("gamma", "Gamma"));
    let (out, dir) = run("prec-fail", s, 22, &["--mode", "prove"]);
    assert_eq!(out.status.code(), Some(6), "{}", stderr(&out));
    let _ = std::fs::remove_dir_all(&dir);

    let mut s = vec![error("alpha")];
    s.extend(timeout("beta"));
    s.extend(unknown("gamma", "Gamma"));
    let (out, dir) = run("prec-error", s, 28, &["--mode", "prove"]);
    assert_eq!(out.status.code(), Some(3), "{}", stderr(&out));
    let _ = std::fs::remove_dir_all(&dir);

    let mut s = vec![pass("alpha")];
    s.extend(unknown("beta", "Beta"));
    s.extend(unknown("gamma", "Gamma"));
    let (out, dir) = run("prec-unknown", s, 4, &["--mode", "prove"]);
    assert_eq!(out.status.code(), Some(7), "{}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("2 not proven"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}
