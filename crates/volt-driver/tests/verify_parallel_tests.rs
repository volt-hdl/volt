//! `volt verify -j` / `--fail-fast` (ADR-0055): tek sby süreci + modül
//! başına görev; rapor KAYNAK SIRASINDA ve deterministik; bir modülün
//! karşı örneği diğer görevleri durdurmaz; `--fail-fast` ilk karşı
//! örnekte durur. sby kurulu olmayan ortamda VOLT_SBY üzerinden sahte
//! sby betikleriyle uçtan uca koşar.

use std::path::{Path, PathBuf};
use std::process::Command;

fn volt() -> Command {
    Command::new(env!("CARGO_BIN_EXE_volt"))
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-verify-par-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// Üç kontratlı modül (Alpha, Beta, Gamma — kaynak sırası): iş adı
/// `three`, görevler `alpha/beta/gamma`, çalışma dizinleri `three_<görev>`.
/// `@no_auto_contracts` (ADR-0066): bu dosya paralel görev akışını sınar,
/// modül başına TEK özellik varsayar — otomatik sayaç cover'ı sayımı bozar.
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

/// Sahte sby betiğinin adımları.
enum Step {
    /// Bir sby satırı bas.
    Echo(&'static str),
    /// Verilen saniye kadar bekle (fail-fast'in süreci kesmesi için).
    Sleep(u32),
    /// Aldığı argümanları `sby_args.txt`'ye yaz (çalışma dizini formal/).
    DumpArgs,
    /// `three_<görev>/engine_0/trace.vcd` üret.
    Trace(&'static str),
}

#[cfg(windows)]
fn write_fake_sby(dir: &Path, steps: &[Step], exit: i32) -> PathBuf {
    let path = dir.join("sby.bat");
    let mut script = String::from("@echo off\r\n");
    for step in steps {
        match step {
            Step::Echo(line) => script.push_str(&format!("echo {line}\r\n")),
            Step::Sleep(secs) => {
                script.push_str(&format!("ping -n {} 127.0.0.1 >nul\r\n", secs + 1))
            }
            Step::DumpArgs => script.push_str("echo %*> sby_args.txt\r\n"),
            Step::Trace(task) => {
                script.push_str(&format!("mkdir three_{task}\\engine_0 2>nul\r\n"));
                script.push_str(&format!(
                    "echo dummy> three_{task}\\engine_0\\trace.vcd\r\n"
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
            Step::Sleep(secs) => script.push_str(&format!("sleep {secs}\n")),
            Step::DumpArgs => script.push_str("echo \"$@\" > sby_args.txt\n"),
            Step::Trace(task) => {
                script.push_str(&format!("mkdir -p three_{task}/engine_0\n"));
                script.push_str(&format!("echo dummy > three_{task}/engine_0/trace.vcd\n"));
            }
        }
    }
    script.push_str(&format!("exit {exit}\n"));
    std::fs::write(&path, script).expect("sahte sby yazılmalı");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    path
}

const PASS_ALPHA: &str = "SBY 12:00:01 [three_alpha] DONE (PASS, rc=0)";
const PASS_BETA: &str = "SBY 12:00:01 [three_beta] DONE (PASS, rc=0)";
const PASS_GAMMA: &str = "SBY 12:00:01 [three_gamma] DONE (PASS, rc=0)";
const STEP_BETA: &str =
    "SBY 12:00:01 [three_beta] engine_0: ## 0:00:00 Checking assertions in step 7..";
const FAILED_BETA: &str =
    "SBY 12:00:01 [three_beta] engine_0: ## 0:00:00 Assert failed in Beta: three.sv:9999.1-9999.5";
const FAIL_BETA: &str = "SBY 12:00:01 [three_beta] DONE (FAIL, rc=2)";

/// Tamamlanma sırası KARIŞIK (Gamma, Alpha, Beta) — rapor yine kaynak
/// sırasında olmalı.
fn scrambled_pass(dir: &Path) -> PathBuf {
    write_fake_sby(
        dir,
        &[
            Step::DumpArgs,
            Step::Echo(PASS_GAMMA),
            Step::Echo(PASS_ALPHA),
            Step::Echo(PASS_BETA),
        ],
        0,
    )
}

/// Beta FAIL, diğerleri PASS — FAIL önce gelir.
fn beta_fails_others_pass(dir: &Path) -> PathBuf {
    write_fake_sby(
        dir,
        &[
            Step::Echo(STEP_BETA),
            Step::Echo(FAILED_BETA),
            Step::Trace("beta"),
            Step::Echo(FAIL_BETA),
            Step::Echo(PASS_GAMMA),
            Step::Echo(PASS_ALPHA),
        ],
        2,
    )
}

struct Run {
    target: PathBuf,
    output: std::process::Output,
}

fn run_verify(tag: &str, sby_of: fn(&Path) -> PathBuf, extra: &[&str]) -> Run {
    let target = temp_dir(tag);
    let src = write_three(&target);
    let sby = sby_of(&target);
    let output = volt()
        .args(["verify", "--target-dir"])
        .arg(&target)
        .args(extra)
        .arg(&src)
        .env("VOLT_SBY", &sby)
        .env_remove("VOLT_LANG")
        .output()
        .expect("volt çalışmalı");
    Run { target, output }
}

fn stderr(run: &Run) -> String {
    String::from_utf8_lossy(&run.output.stderr).into_owned()
}

fn json(run: &Run) -> serde_json::Value {
    let stdout = String::from_utf8_lossy(&run.output.stdout);
    serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("JSON zarfı: {e}\n{stdout}"))
}

/// Süre alanlarını maskeler — determinizm karşılaştırması için.
fn strip_durations(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Object(map) => {
            map.remove("duration_ms");
            for child in map.values_mut() {
                strip_durations(child);
            }
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(strip_durations),
        _ => {}
    }
}

fn statuses(v: &serde_json::Value) -> Vec<String> {
    v["verify"]["modules"]
        .as_array()
        .expect("modules")
        .iter()
        .map(|m| {
            format!(
                "{}:{}",
                m["module"].as_str().unwrap(),
                m["status"].as_str().unwrap()
            )
        })
        .collect()
}

// ═══ Yapılandırma ve bayraklar ═══════════════════════════════════════

#[test]
fn sby_config_has_one_task_per_module_in_source_order() {
    let target = temp_dir("config");
    let src = write_three(&target);
    let output = volt()
        .args(["verify", "--target-dir"])
        .arg(&target)
        .arg(&src)
        .env("PATH", "")
        .env_remove("VOLT_SBY")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(3), "sby yok → 3");
    let sby = std::fs::read_to_string(target.join("formal/three.sby")).expect("three.sby");
    assert!(
        sby.starts_with("[tasks]\nalpha\nbeta\ngamma\n\n[options]\n"),
        "{sby}"
    );
    assert!(sby.contains("read -formal three.sv\n"), "{sby}");
    assert!(
        sby.contains("alpha: prep -top Alpha\nbeta: prep -top Beta\ngamma: prep -top Gamma\n"),
        "{sby}"
    );
    assert!(sby.ends_with("[files]\nthree.sv\n"), "{sby}");
    assert!(target.join("formal/three.sv").is_file(), "tek .sv");
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn jobs_flag_is_passed_to_sby_before_the_config_file() {
    let run = run_verify("jobs-4", scrambled_pass, &["-j", "4"]);
    assert_eq!(
        run.output.status.code(),
        Some(0),
        "stderr: {}",
        stderr(&run)
    );
    let args = std::fs::read_to_string(run.target.join("formal/sby_args.txt")).expect("args");
    assert!(args.trim().starts_with("-j 4 -f three.sby"), "args: {args}");
    let _ = std::fs::remove_dir_all(&run.target);
}

#[test]
fn jobs_default_is_auto_and_resolves_to_a_positive_count() {
    let run = run_verify("jobs-auto", scrambled_pass, &["--format", "json"]);
    assert_eq!(
        run.output.status.code(),
        Some(0),
        "stderr: {}",
        stderr(&run)
    );
    let args = std::fs::read_to_string(run.target.join("formal/sby_args.txt")).expect("args");
    let n: usize = args
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| panic!("args: {args}"));
    assert!(n >= 1);
    assert_eq!(json(&run)["verify"]["jobs"], n);
    let _ = std::fs::remove_dir_all(&run.target);
}

#[test]
fn jobs_zero_or_garbage_is_usage_error_exit_2() {
    for bad in ["0", "abc", "-3"] {
        let target = temp_dir(&format!("jobs-bad-{}", bad.len()));
        let src = write_three(&target);
        let output = volt()
            .args(["verify", "-j", bad, "--target-dir"])
            .arg(&target)
            .arg(&src)
            .output()
            .expect("volt çalışmalı");
        assert_eq!(output.status.code(), Some(2), "-j {bad}");
        let _ = std::fs::remove_dir_all(&target);
    }
}

// ═══ Determinizm ═════════════════════════════════════════════════════

#[test]
fn j1_and_j4_report_identical_results() {
    let one = run_verify("det-j1", scrambled_pass, &["-j", "1", "--format", "json"]);
    let four = run_verify("det-j4", scrambled_pass, &["-j", "4", "--format", "json"]);
    assert_eq!(one.output.status.code(), Some(0), "{}", stderr(&one));
    assert_eq!(four.output.status.code(), Some(0), "{}", stderr(&four));
    let (mut a, mut b) = (json(&one), json(&four));
    strip_durations(&mut a);
    strip_durations(&mut b);
    // Yapıt yolları hedef dizine bağlı; jobs alanı bilerek farklı.
    for v in [&mut a, &mut b] {
        v["artifacts"] = serde_json::Value::Null;
        v["verify"]["jobs"] = serde_json::Value::Null;
    }
    assert_eq!(a, b);
    let _ = std::fs::remove_dir_all(&one.target);
    let _ = std::fs::remove_dir_all(&four.target);
}

#[test]
fn report_is_in_source_order_even_when_completion_order_is_scrambled() {
    let run = run_verify("order", scrambled_pass, &["-j", "4", "--format", "json"]);
    assert_eq!(run.output.status.code(), Some(0), "{}", stderr(&run));
    assert_eq!(
        statuses(&json(&run)),
        ["Alpha:pass", "Beta:pass", "Gamma:pass"]
    );
    let props: Vec<String> = json(&run)["verify"]["properties"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            format!(
                "{}.{}",
                p["module"].as_str().unwrap(),
                p["name"].as_str().unwrap()
            )
        })
        .collect();
    assert_eq!(props, ["Alpha.inv_0", "Beta.inv_0", "Gamma.inv_0"]);
    let _ = std::fs::remove_dir_all(&run.target);
}

#[test]
fn progress_lines_follow_completion_order_with_running_counter() {
    let run = run_verify("progress", scrambled_pass, &["-j", "4"]);
    let err = stderr(&run);
    let gamma = err.find("[1/3] Gamma (1 property) ... ok (").expect(&err);
    let alpha = err.find("[2/3] Alpha (1 property) ... ok (").expect(&err);
    let beta = err.find("[3/3] Beta (1 property) ... ok (").expect(&err);
    assert!(gamma < alpha && alpha < beta, "{err}");
    let _ = std::fs::remove_dir_all(&run.target);
}

#[test]
fn same_input_gives_same_report_across_runs() {
    let first = run_verify(
        "rerun-1",
        beta_fails_others_pass,
        &["-j", "3", "--format", "json"],
    );
    let second = run_verify(
        "rerun-2",
        beta_fails_others_pass,
        &["-j", "3", "--format", "json"],
    );
    let (mut a, mut b) = (json(&first), json(&second));
    strip_durations(&mut a);
    strip_durations(&mut b);
    for v in [&mut a, &mut b] {
        v["artifacts"] = serde_json::Value::Null;
        v["diagnostics"] = serde_json::Value::Null; // karşı örnek yolu hedef dizine bağlı
    }
    assert_eq!(a, b);
    let _ = std::fs::remove_dir_all(&first.target);
    let _ = std::fs::remove_dir_all(&second.target);
}

// ═══ Hata durumu ═════════════════════════════════════════════════════

#[test]
fn one_failing_module_does_not_stop_the_others_exit_6() {
    let run = run_verify("fail-continue", beta_fails_others_pass, &["-j", "4"]);
    assert_eq!(run.output.status.code(), Some(6), "{}", stderr(&run));
    let err = stderr(&run);
    assert!(err.contains("[1/3] Beta (1 property) ... FAIL ("), "{err}");
    assert!(err.contains("[2/3] Gamma (1 property) ... ok ("), "{err}");
    assert!(err.contains("[3/3] Alpha (1 property) ... ok ("), "{err}");
    assert!(err.contains("error[E5001]"), "{err}");
    assert!(err.contains("Failures:"), "{err}");
    assert!(
        err.contains("Beta.inv_0  E5001 contract violated at cycle 7"),
        "{err}"
    );
    assert!(err.contains("Result 1 of 3 properties failed in"), "{err}");
    assert!(err.contains("Next: volt explain E5001"), "{err}");
    let _ = std::fs::remove_dir_all(&run.target);
}

#[test]
fn failing_module_json_marks_only_that_module_and_keeps_source_order() {
    let run = run_verify(
        "fail-json",
        beta_fails_others_pass,
        &["-j", "4", "--format", "json"],
    );
    assert_eq!(run.output.status.code(), Some(6));
    let v = json(&run);
    assert_eq!(v["success"], false);
    assert_eq!(v["diagnostics"][0]["code"], "E5001");
    assert_eq!(statuses(&v), ["Alpha:pass", "Beta:fail", "Gamma:pass"]);
    assert_eq!(v["verify"]["properties"][1]["status"], "fail");
    assert_eq!(v["verify"]["fail_fast"], false);
    let _ = std::fs::remove_dir_all(&run.target);
}

#[test]
fn counterexample_is_copied_from_the_task_workdir() {
    let run = run_verify("cex", beta_fails_others_pass, &["-j", "2"]);
    assert_eq!(run.output.status.code(), Some(6));
    assert!(
        run.target.join("formal/beta_cex.vcd").is_file(),
        "beta_cex.vcd"
    );
    assert!(stderr(&run).contains("beta_cex.vcd"), "{}", stderr(&run));
    let _ = std::fs::remove_dir_all(&run.target);
}

#[test]
fn j1_and_j4_agree_when_a_module_fails() {
    let one = run_verify(
        "fail-j1",
        beta_fails_others_pass,
        &["-j", "1", "--format", "json"],
    );
    let four = run_verify(
        "fail-j4",
        beta_fails_others_pass,
        &["-j", "4", "--format", "json"],
    );
    assert_eq!(one.output.status.code(), Some(6));
    assert_eq!(four.output.status.code(), Some(6));
    assert_eq!(statuses(&json(&one)), statuses(&json(&four)));
    let _ = std::fs::remove_dir_all(&one.target);
    let _ = std::fs::remove_dir_all(&four.target);
}

fn beta_fails_then_sleeps(dir: &Path) -> PathBuf {
    write_fake_sby(
        dir,
        &[
            Step::Echo(STEP_BETA),
            Step::Echo(FAILED_BETA),
            Step::Echo(FAIL_BETA),
            Step::Sleep(3),
            Step::Echo(PASS_GAMMA),
            Step::Echo(PASS_ALPHA),
        ],
        2,
    )
}

#[test]
fn fail_fast_stops_at_the_first_counterexample() {
    let run = run_verify(
        "fail-fast",
        beta_fails_then_sleeps,
        &["-j", "4", "--fail-fast", "--format", "json"],
    );
    assert_eq!(run.output.status.code(), Some(6), "{}", stderr(&run));
    let v = json(&run);
    assert_eq!(v["verify"]["fail_fast"], true);
    assert_eq!(
        statuses(&v),
        ["Alpha:skipped", "Beta:fail", "Gamma:skipped"]
    );
    assert_eq!(v["verify"]["properties"][0]["status"], "skipped");
    assert!(v["verify"]["properties"][0]["duration_ms"].is_null());
    let _ = std::fs::remove_dir_all(&run.target);
}

#[test]
fn fail_fast_human_summary_counts_skipped_tasks() {
    let run = run_verify(
        "fail-fast-human",
        beta_fails_then_sleeps,
        &["-j", "4", "--fail-fast"],
    );
    assert_eq!(run.output.status.code(), Some(6));
    let err = stderr(&run);
    assert!(err.contains("[1/3] Beta (1 property) ... FAIL ("), "{err}");
    assert!(!err.contains("Gamma (1 property) ... ok"), "{err}");
    assert!(
        err.contains("2 module task(s) skipped (--fail-fast)"),
        "{err}"
    );
    let _ = std::fs::remove_dir_all(&run.target);
}

#[test]
fn fail_fast_with_all_passing_skips_nothing_exit_0() {
    let run = run_verify(
        "fail-fast-pass",
        scrambled_pass,
        &["--fail-fast", "--format", "json"],
    );
    assert_eq!(run.output.status.code(), Some(0), "{}", stderr(&run));
    assert_eq!(
        statuses(&json(&run)),
        ["Alpha:pass", "Beta:pass", "Gamma:pass"]
    );
    let _ = std::fs::remove_dir_all(&run.target);
}

fn beta_missing(dir: &Path) -> PathBuf {
    write_fake_sby(dir, &[Step::Echo(PASS_ALPHA), Step::Echo(PASS_GAMMA)], 16)
}

#[test]
fn task_without_done_line_is_tool_error_exit_3_with_rerun_hint() {
    let run = run_verify("missing", beta_missing, &["-j", "2"]);
    assert_eq!(run.output.status.code(), Some(3), "{}", stderr(&run));
    let err = stderr(&run);
    assert!(err.contains("tool error for module 'Beta'"), "{err}");
    assert!(err.contains("-f three.sby beta"), "{err}");
    assert!(err.contains("Beta  tool error"), "{err}");
    let _ = std::fs::remove_dir_all(&run.target);
}

fn beta_fails_gamma_missing(dir: &Path) -> PathBuf {
    write_fake_sby(
        dir,
        &[
            Step::Echo(FAILED_BETA),
            Step::Echo(FAIL_BETA),
            Step::Echo(PASS_ALPHA),
        ],
        2,
    )
}

#[test]
fn counterexample_beats_tool_error_in_exit_code() {
    let run = run_verify(
        "fail-and-error",
        beta_fails_gamma_missing,
        &["-j", "2", "--format", "json"],
    );
    assert_eq!(run.output.status.code(), Some(6));
    assert_eq!(
        statuses(&json(&run)),
        ["Alpha:pass", "Beta:fail", "Gamma:error"]
    );
    let _ = std::fs::remove_dir_all(&run.target);
}

// ═══ Çıktı biçimi ════════════════════════════════════════════════════

#[test]
fn json_has_per_property_status_and_duration() {
    let run = run_verify(
        "json-props",
        scrambled_pass,
        &["-j", "4", "--format", "json"],
    );
    let v = json(&run);
    assert_eq!(v["verify"]["jobs"], 4);
    assert_eq!(v["verify"]["mode"], "bmc");
    assert_eq!(v["verify"]["depth"], 20);
    assert_eq!(v["verify"]["engine"], "z3");
    let props = v["verify"]["properties"].as_array().expect("properties");
    assert_eq!(props.len(), 3);
    for p in props {
        assert_eq!(p["status"], "pass");
        assert_eq!(p["keyword"], "invariant");
        assert!(p["duration_ms"].is_u64(), "{p}");
    }
    let mods = v["verify"]["modules"].as_array().expect("modules");
    assert_eq!(mods[0]["task"], "alpha");
    assert_eq!(mods[0]["properties"], 1);
    let _ = std::fs::remove_dir_all(&run.target);
}

#[test]
fn human_summary_reports_property_count_time_and_jobs() {
    let run = run_verify("summary", scrambled_pass, &["-j", "2"]);
    assert_eq!(run.output.status.code(), Some(0));
    let err = stderr(&run);
    assert!(err.contains("3 properties verified in "), "{err}");
    assert!(err.contains("(2 jobs; bmc, depth 20)"), "{err}");
    let _ = std::fs::remove_dir_all(&run.target);
}

#[test]
fn turkish_progress_and_summary() {
    let target = temp_dir("tr");
    let src = write_three(&target);
    let sby = scrambled_pass(&target);
    let output = volt()
        .args(["verify", "--lang=tr", "-j", "2", "--target-dir"])
        .arg(&target)
        .arg(&src)
        .env("VOLT_SBY", &sby)
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(0));
    let err = String::from_utf8_lossy(&output.stderr);
    assert!(err.contains("[1/3] Gamma (1 özellik) ... tamam ("), "{err}");
    assert!(err.contains("3 özellik"), "{err}");
    assert!(err.contains("(2 iş; bmc, derinlik 20)"), "{err}");
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn short_format_prints_diagnostics_without_progress() {
    let run = run_verify(
        "short",
        beta_fails_others_pass,
        &["-j", "2", "--format", "short"],
    );
    assert_eq!(run.output.status.code(), Some(6));
    let err = stderr(&run);
    assert!(err.contains("E5001"), "{err}");
    assert!(!err.contains("[1/3]"), "{err}");
    assert!(!err.contains("Failures:"), "{err}");
    let _ = std::fs::remove_dir_all(&run.target);
}
