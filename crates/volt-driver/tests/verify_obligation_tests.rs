//! Alt örnek yükümlülükleri (ADR-0097): bir örneğin `requires`/`assume`'u
//! onu süren üst modülün görevinde `assert` olur; saatsiz modülün
//! kontratı ve hiçbir şey denetlemeyen koşu başarı sayılmaz.
//!
//! Önce `volt verify` alt örneğin `requires`'ını üst görevde de VARSAYIYORDU:
//! `tests/fixtures/formal_obligations/violate.volt` (üst modül x = 12 sürer,
//! alt modül x < 10 ister) çıkış 0 ile geçiyordu.
//!
//! Araçsız testler (E5005/E5006, `.sby` düzeni, sahte sby ile JSON) her
//! ortamda koşar. Gerçek sby testleri sby yoksa atlanır; CI'ın `verify`
//! işinde `VOLT_REQUIRE_TOOLS=sby` ile zorunludur (ADR-0079 §3). Gerçek
//! koşuların hedef dizini depo içindedir (`CARGO_TARGET_TMPDIR`): Docker
//! sarmalayıcısı (yerelde `VOLT_SBY`) kısa Windows temp yollarını bağlayamaz.

mod tools;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn volt() -> Command {
    Command::new(env!("CARGO_BIN_EXE_volt"))
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/formal_obligations")
        .join(name)
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-oblig-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// Gerçek sby koşusu için depo içi hedef dizini.
fn real_dir(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("oblig-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("hedef dizini");
    dir
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[cfg(windows)]
fn write_fake_sby(dir: &Path, lines: &[String], exit: i32) -> PathBuf {
    let path = dir.join("sby.bat");
    let mut script = String::from("@echo off\r\n");
    for line in lines {
        script.push_str(&format!("echo {line}\r\n"));
    }
    script.push_str(&format!("exit /b {exit}\r\n"));
    std::fs::write(&path, script).expect("sahte sby yazılmalı");
    path
}

#[cfg(unix)]
fn write_fake_sby(dir: &Path, lines: &[String], exit: i32) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("sby");
    let mut script = String::from("#!/bin/sh\n");
    for line in lines {
        script.push_str(&format!("echo '{line}'\n"));
    }
    script.push_str(&format!("exit {exit}\n"));
    std::fs::write(&path, script).expect("sahte sby yazılmalı");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    path
}

/// Sahte sby ile `volt verify` (sby yerine VOLT_SBY).
fn verify_fake(fixture_name: &str, tag: &str, lines: &[String], exit: i32, json: bool) -> Output {
    let dir = temp_dir(tag);
    let sby = write_fake_sby(&dir, lines, exit);
    let mut cmd = volt();
    cmd.arg("verify");
    if json {
        cmd.arg("--format=json");
    }
    cmd.args(["-j", "1", "--target-dir"])
        .arg(dir.join("build"))
        .arg(fixture(fixture_name))
        .env("VOLT_SBY", &sby)
        .env("VOLT_TOOL_BACKEND", "local");
    let out = cmd.output().expect("volt");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

// ═══ Araçsız: saatsiz modül (E5005) ve boş doğrulama (E5006) ═════════

#[test]
fn clockless_module_contracts_stop_the_run_with_e5005() {
    let dir = temp_dir("clockless");
    let out = volt()
        .args(["verify", "--target-dir"])
        .arg(&dir)
        .arg(fixture("clockless.volt"))
        .output()
        .expect("volt");
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(err.contains("error[E5005]"), "{err}");
    assert!(
        err.contains("'Comb' has 2 contract(s) but no clock port"),
        "{err}"
    );
    assert!(!err.contains("no contracts found"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn clockless_module_still_builds_and_checks() {
    // ADR-0097 yalnız formal akışı değiştirir: `check` temiz kalır.
    let out = volt()
        .arg("check")
        .arg(fixture("clockless.volt"))
        .output()
        .expect("volt");
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
}

#[test]
fn only_assumptions_is_nothing_to_verify() {
    let dir = temp_dir("nothing");
    let out = volt()
        .args(["verify", "--target-dir"])
        .arg(&dir)
        .arg(fixture("nothing_to_verify.volt"))
        .output()
        .expect("volt");
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(err.contains("error[E5006]"), "{err}");
    assert!(err.contains("Top.req_0 (requires)"), "{err}");
    // sby'ye hiç gidilmedi: .sby yazılmadı.
    assert!(!dir.join("formal").exists(), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_design_without_contracts_is_nothing_to_verify() {
    let dir = temp_dir("nocontract");
    let src = dir.join("plain.volt");
    std::fs::write(
        &src,
        "module Plain {\n    in  clk : clock\n    in  a   : u8\n    out b   : u8\n\n    \
         reg r : u8 = 0\n    on clk {\n        r <= a\n    }\n    b = r\n}\n",
    )
    .expect("yaz");
    let out = volt()
        .args(["verify", "--format=json", "--target-dir"])
        .arg(dir.join("build"))
        .arg(&src)
        .output()
        .expect("volt");
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("JSON zarfı");
    let codes: Vec<&str> = json["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .filter_map(|d| d["code"].as_str())
        .collect();
    assert_eq!(codes, ["E5006"], "{json}");
    let _ = std::fs::remove_dir_all(&dir);
}

// ═══ Araçsız: görev planı ve .sby düzeni (sahte sby) ═════════════════

#[test]
fn parent_task_defines_the_child_macro_and_child_without_assertions_has_no_task() {
    let dir = temp_dir("sby-layout");
    let sby = write_fake_sby(
        &dir,
        &["SBY 12:00:00 [violate_parent] DONE (PASS, rc=0)".to_string()],
        0,
    );
    let out = volt()
        .args(["verify", "-j", "1", "--target-dir"])
        .arg(dir.join("build"))
        .arg(fixture("violate.volt"))
        .env("VOLT_SBY", &sby)
        .env("VOLT_TOOL_BACKEND", "local")
        .output()
        .expect("volt");
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(0), "{err}");
    let text = std::fs::read_to_string(dir.join("build/formal/violate.sby")).expect(".sby");
    assert!(text.starts_with("[tasks]\nparent\n\n"), "{text}");
    assert!(
        text.contains("[script]\nparent: read -define VOLT_SUB_Child\nread -formal violate.sv\n"),
        "{text}"
    );
    let sv = std::fs::read_to_string(dir.join("build/formal/violate.sv")).expect(".sv");
    assert!(sv.contains("`ifdef VOLT_SUB_Child\n"), "{sv}");
    // Kontratsız üst modül de görevdir; sayımda alt örneğin yükümlülüğü.
    assert!(err.contains("[1/1] Parent (1 property) ... ok"), "{err}");
    assert!(err.contains("Result 1 property verified"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn json_reports_the_obligation_under_its_owner_with_the_parent_as_context() {
    let lines = [
        "SBY 12:00:00 [satisfy_child] DONE (PASS, rc=0)".to_string(),
        "SBY 12:00:00 [satisfy_parent] DONE (PASS, rc=0)".to_string(),
    ];
    let out = verify_fake("satisfy.volt", "json", &lines, 0, true);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("JSON");
    let v = &json["verify"];
    let mods = v["modules"].as_array().expect("modules");
    assert_eq!(mods.len(), 2, "{v}");
    assert_eq!(mods[0]["module"], "Child");
    assert_eq!(mods[0]["properties"], 1);
    assert_eq!(mods[0]["assumed"], 1);
    assert_eq!(mods[1]["module"], "Parent");
    assert_eq!(mods[1]["properties"], 2);
    assert_eq!(mods[1]["assumed"], 0);
    let rows: Vec<(String, String, String, Option<String>)> = v["properties"]
        .as_array()
        .expect("properties")
        .iter()
        .map(|p| {
            (
                p["module"].as_str().unwrap().to_string(),
                p["name"].as_str().unwrap().to_string(),
                p["status"].as_str().unwrap().to_string(),
                p["context"].as_str().map(str::to_string),
            )
        })
        .collect();
    let row = |m: &str, n: &str, s: &str, c: Option<&str>| {
        (
            m.to_string(),
            n.to_string(),
            s.to_string(),
            c.map(str::to_string),
        )
    };
    assert_eq!(
        rows,
        [
            row("Child", "ens_0", "pass", None),
            row("Child", "req_0", "assumed", None),
            row("Parent", "inv_0", "pass", None),
            row("Child", "req_0", "pass", Some("Parent")),
        ]
    );
}

#[test]
fn unmapped_failure_in_a_contractless_parent_falls_back_to_the_obligation() {
    // Konum eşlenemese de (satır 9999) görevin ilk denetlenen kontratına
    // düşülür; kontratsız üst modülde bu alt örneğin yükümlülüğüdür.
    let lines = [
        "SBY 12:00:01 [violate_parent] engine_0: ## 0:00:00 Assert failed in Parent.c: violate.sv:9999.1-9999.5".to_string(),
        "SBY 12:00:01 [violate_parent] DONE (FAIL, rc=2)".to_string(),
    ];
    let out = verify_fake("violate.volt", "fallback", &lines, 2, false);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(6), "{err}");
    assert!(
        err.contains("Child.req_0  E5001 contract violated"),
        "{err}"
    );
    assert!(err.contains("(in Parent)"), "{err}");
    assert!(err.contains("instance 'c' of 'Child' in 'Parent'"), "{err}");
}

// ═══ Gerçek sby (yoksa atlanır; CI verify işinde zorunlu) ═════════════

fn real_verify(fixture_name: &str, tag: &str) -> Option<Output> {
    let sby = tools::require(tools::Tool::Sby)?;
    let dir = real_dir(tag);
    let out = volt()
        .args(["verify", "-j", "1", "--depth", "10", "--target-dir"])
        .arg(&dir)
        .arg(fixture(fixture_name))
        .env("VOLT_SBY", &sby)
        .output()
        .expect("volt");
    Some(out)
}

/// ADIM 1 örneği: önce yanlışlıkla geçiyordu, artık üst modülü gösteren
/// bir karşı örnekle düşer.
#[test]
fn real_sby_parent_breaking_the_childs_requires_fails() {
    let Some(out) = real_verify("violate.volt", "violate") else {
        return;
    };
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(6), "{err}");
    assert!(
        err.contains("Child.req_0  E5001 contract violated"),
        "{err}"
    );
    assert!(err.contains("(in Parent)"), "{err}");
    assert!(err.contains("instance 'c' of 'Child' in 'Parent'"), "{err}");
    assert!(err.contains("parent_cex.vcd"), "{err}");
}

#[test]
fn real_sby_parent_meeting_the_childs_requires_passes() {
    let Some(out) = real_verify("satisfy.volt", "satisfy") else {
        return;
    };
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(0), "{err}");
    assert!(
        err.contains("Child (1 property, 1 assumed) ... ok"),
        "{err}"
    );
    assert!(err.contains("Parent (2 properties) ... ok"), "{err}");
}

#[test]
fn real_sby_three_levels_fail_in_the_middle_and_at_the_top() {
    let Some(out) = real_verify("three_level_violate.volt", "three-bad") else {
        return;
    };
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(6), "{err}");
    assert!(
        err.contains("Leaf.req_0  E5001 contract violated at cycle 2 (in Mid)"),
        "{err}"
    );
    assert!(
        err.contains("Leaf.req_0  E5001 contract violated at cycle 2 (in Top)"),
        "{err}"
    );
}

#[test]
fn real_sby_three_levels_pass_when_every_level_states_its_precondition() {
    let Some(out) = real_verify("three_level_satisfy.volt", "three-ok") else {
        return;
    };
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(0), "{err}");
    // Top hem Mid'in hem Leaf'in yükümlülüğünü denetler.
    assert!(err.contains("Top (2 properties) ... ok"), "{err}");
}

#[test]
fn real_sby_only_the_bad_instance_is_blamed() {
    let Some(out) = real_verify("multi_instance.volt", "multi-bad") else {
        return;
    };
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(6), "{err}");
    assert!(
        err.contains("instance 'bad' of 'Child' in 'Parent'"),
        "{err}"
    );
    assert!(!err.contains("instance 'ok'"), "{err}");
}

#[test]
fn real_sby_every_instance_meeting_the_obligation_passes() {
    let Some(out) = real_verify("multi_instance_satisfy.volt", "multi-ok") else {
        return;
    };
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(0), "{err}");
}
