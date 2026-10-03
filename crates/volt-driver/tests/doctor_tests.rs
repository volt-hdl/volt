//! `volt doctor` (ADR-0084 Bölüm 1): araçlar var/yok/eski/takılı iken
//! rapor ve çıkış kodu. Araçlar sahte PATH'teki betiklerdir — gerçek
//! kurulumdan bağımsız, her makinede aynı sonuç.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use serde_json::Value;

fn volt() -> Command {
    Command::new(env!("CARGO_BIN_EXE_volt"))
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-doctor-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// Betik gövdesi: sürüm satırı basar ya da takılır.
enum Body<'a> {
    Prints(&'a str),
    Hangs,
}

/// Sahte araç: `name` adlı, `PATH`'te bulunan betik.
#[cfg(windows)]
fn fake_tool(dir: &Path, name: &str, body: Body) -> PathBuf {
    let path = dir.join(format!("{name}.bat"));
    let line = match body {
        Body::Prints(text) => format!("echo {text}"),
        Body::Hangs => r#""%SystemRoot%\System32\ping.exe" -n 30 127.0.0.1 >nul"#.to_string(),
    };
    std::fs::write(&path, format!("@echo off\r\n{line}\r\n")).expect("sahte araç");
    path
}

#[cfg(unix)]
fn fake_tool(dir: &Path, name: &str, body: Body) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join(name);
    let line = match body {
        Body::Prints(text) => format!("echo '{text}'"),
        Body::Hangs => "exec /bin/sleep 30".to_string(),
    };
    std::fs::write(&path, format!("#!/bin/sh\n{line}\n")).expect("sahte araç");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    path
}

/// Tam kurulum: her aracın sahte sürüm satırı (PATH adları `volt-tools`
/// adaylarıdır).
const FULL: &[(&str, &str)] = &[
    ("verilator", "Verilator 5.050 2026-08-01 rev v5.050"),
    ("sby", "SBY yosys-0.57"),
    ("yosys", "Yosys 0.57 (git sha1 81011ad92)"),
    ("boolector", "3.2.3"),
    ("bitwuzla", "0.7.0"),
    ("yices-smt2", "Yices 2.6.5"),
    ("z3", "Z3 version 4.13.4 - 64 bit"),
    ("sta", "2.6.0"),
    ("gcc", "gcc (Ubuntu 13.2.0-23ubuntu4) 13.2.0"),
    ("g++", "g++ (Ubuntu 13.2.0-23ubuntu4) 13.2.0"),
    ("rustc", "rustc 1.90.0 (1159e78c4 2025-09-14)"),
    ("make", "GNU Make 4.3"),
    ("docker", "Docker version 27.3.1, build ce12230"),
];

fn install(dir: &Path, tools: &[(&str, &str)]) {
    for (name, version) in tools {
        fake_tool(dir, name, Body::Prints(version));
    }
}

/// `volt doctor` yalnız `path_dir` PATH'iyle; araç değişkenleri ve
/// kullanıcının VOLT_LANG'ı temizlenir, manifest araması boş dizine
/// sabitlenir (sonuç çalışma dizininden bağımsız).
fn doctor_cmd(path_dir: &Path) -> Command {
    let empty = path_dir.join("no-manifest");
    std::fs::create_dir_all(&empty).expect("dizin");
    let mut cmd = volt();
    cmd.arg("doctor")
        .env("PATH", path_dir)
        .env("VOLT_MANIFEST_DIR", &empty)
        .env_remove("VOLT_LANG")
        .env_remove("WSL_DISTRO_NAME");
    for var in ["VOLT_VERILATOR", "VOLT_SBY", "VOLT_DOCKER", "CC", "CXX"] {
        cmd.env_remove(var);
    }
    // doctor araç başlatmaz, yalnız yoklar: varsayılan (auto) açıkça.
    cmd.env("VOLT_TOOL_BACKEND", "auto");
    cmd
}

fn run_json(mut cmd: Command) -> (Output, Value) {
    let out = cmd
        .args(["--format", "json"])
        .output()
        .expect("volt doctor");
    let json = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "JSON değil ({e}):\n{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    });
    (out, json)
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn tool<'a>(json: &'a Value, name: &str) -> &'a Value {
    json["tools"]
        .as_array()
        .expect("tools dizisi")
        .iter()
        .find(|t| t["name"] == name)
        .unwrap_or_else(|| panic!("{name} raporda yok: {json}"))
}

fn cap<'a>(json: &'a Value, id: &str) -> &'a Value {
    json["capabilities"]
        .as_array()
        .expect("capabilities dizisi")
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("{id} raporda yok: {json}"))
}

#[test]
fn no_tools_reports_every_capability_but_core_missing_exit_0() {
    let dir = temp_dir("none");
    let (out, json) = run_json(doctor_cmd(&dir));
    assert_eq!(out.status.code(), Some(0), "{json}");
    assert_eq!(json["schema"], "volt-doctor/1");
    assert_eq!(json["required_ok"], false);
    assert_eq!(cap(&json, "core")["status"], "ok");
    for id in ["simulation", "verify", "timing", "drivers"] {
        assert_eq!(cap(&json, id)["status"], "missing", "{id}");
    }
    for t in json["tools"].as_array().unwrap() {
        assert_eq!(t["status"], "missing", "{t}");
        assert_eq!(t["path"], Value::Null, "{t}");
    }
    assert_eq!(json["solver"], Value::Null);
    assert_eq!(json["docker_daemon"], Value::Null);
    assert_eq!(json["project"]["search_stop"], "override");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_tools_human_report_names_the_commands_and_the_fix() {
    let dir = temp_dir("none-human");
    let out = doctor_cmd(&dir).output().expect("volt doctor");
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(
        text.contains("✓ build, check, explain — no external tools needed"),
        "{text}"
    );
    assert!(
        text.contains("✗ test, run — Verilator, C++ compiler, make not found"),
        "{text}"
    );
    assert!(
        text.contains("see: volt explain simulation-setup"),
        "{text}"
    );
    assert!(text.contains("✗ verify — sby, Yosys not found"), "{text}");
    assert!(text.contains("see: volt explain verify-setup"), "{text}");
    // Kurulum ipucu explain konusundan gelir (ADR-0084 §2).
    assert!(text.contains("install ("), "{text}");
    assert!(
        text.contains("- timing (optional) — OpenSTA not found"),
        "{text}"
    );
    // Okur ADR numaralarını bilmez: rapor iç belgeye atıf yapmaz.
    assert!(!text.contains("ADR-"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn strict_exits_3_when_a_required_capability_is_missing() {
    let dir = temp_dir("strict");
    let mut cmd = doctor_cmd(&dir);
    let out = cmd.arg("--strict").output().expect("volt doctor");
    assert_eq!(out.status.code(), Some(3), "{}", stdout(&out));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn full_install_is_ok_with_versions_and_strict_exit_0() {
    let dir = temp_dir("full");
    install(&dir, FULL);
    let (out, json) = run_json(doctor_cmd(&dir));
    assert_eq!(out.status.code(), Some(0), "{json}");
    assert_eq!(json["required_ok"], true, "{json}");
    for id in ["core", "simulation", "verify", "timing", "drivers"] {
        assert_eq!(cap(&json, id)["status"], "ok", "{id}: {json}");
    }
    let verilator = tool(&json, "verilator");
    assert_eq!(verilator["status"], "ok");
    assert_eq!(verilator["version"], "5.050");
    assert_eq!(verilator["min_version"], "5.0");
    let path = verilator["path"].as_str().expect("yol");
    assert!(
        Path::new(path).starts_with(&dir),
        "{path} sahte dizinde değil"
    );
    assert_eq!(tool(&json, "yosys")["version"], "0.57");
    assert_eq!(tool(&json, "z3")["version"], "4.13.4");
    assert_eq!(tool(&json, "yices")["version"], "2.6.5");
    assert_eq!(tool(&json, "cxx")["version"], "13.2.0");
    assert_eq!(json["solver"], "boolector");
    assert_eq!(json["docker_daemon"]["status"], "running");

    let strict = doctor_cmd(&dir)
        .arg("--strict")
        .output()
        .expect("volt doctor");
    assert_eq!(strict.status.code(), Some(0), "{}", stdout(&strict));
    let text = stdout(&strict);
    assert!(
        text.contains("✓ test, run — Verilator 5.050, C++ compiler 13.2.0, make 4.3"),
        "{text}"
    );
    assert!(
        text.contains("✓ verify — sby 0.57, Yosys 0.57, boolector 3.2.3"),
        "{text}"
    );
    assert!(
        text.contains("also: bitwuzla 0.7.0, yices 2.6.5, z3 4.13.4"),
        "{text}"
    );
    assert!(!text.contains("optional bitwuzla not found"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Doctor `volt test` ile aynı aramayı kullanır: VOLT_VERILATOR PATH'in
/// önüne geçer (ADR-0084 §1).
#[test]
fn tool_env_override_wins_like_volt_test() {
    let dir = temp_dir("override");
    let elsewhere = dir.join("custom");
    std::fs::create_dir_all(&elsewhere).expect("dizin");
    let custom = fake_tool(&elsewhere, "my-verilator", Body::Prints("Verilator 5.028"));
    install(&dir, &[("verilator", "Verilator 5.050")]);
    let mut cmd = doctor_cmd(&dir);
    cmd.env("VOLT_VERILATOR", &custom);
    let (_, json) = run_json(cmd);
    let verilator = tool(&json, "verilator");
    assert_eq!(verilator["version"], "5.028", "{json}");
    assert_eq!(verilator["path"], custom.display().to_string());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn old_verilator_is_degraded_and_fails_strict() {
    let dir = temp_dir("old");
    install(&dir, FULL);
    fake_tool(
        &dir,
        "verilator",
        Body::Prints("Verilator 4.228 2022-10-01"),
    );
    let (out, json) = run_json(doctor_cmd(&dir));
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(tool(&json, "verilator")["status"], "too_old");
    assert_eq!(cap(&json, "simulation")["status"], "degraded");
    assert_eq!(json["required_ok"], false);
    let strict = doctor_cmd(&dir)
        .arg("--strict")
        .output()
        .expect("volt doctor");
    assert_eq!(strict.status.code(), Some(3));
    let text = stdout(&strict);
    assert!(
        text.contains("! test, run — Verilator 4.228 is older than 5.0"),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Varsayılan çözücü yoksa verify çalışır ama `--engine` gerekir.
#[test]
fn only_z3_degrades_verify_and_names_the_engine_flag() {
    let dir = temp_dir("z3");
    install(
        &dir,
        &[
            ("sby", "SBY yosys-0.57"),
            ("yosys", "Yosys 0.57"),
            ("z3", "Z3 version 4.13.4 - 64 bit"),
        ],
    );
    let (_, json) = run_json(doctor_cmd(&dir));
    assert_eq!(cap(&json, "verify")["status"], "degraded", "{json}");
    assert_eq!(json["solver"], "z3");
    let out = doctor_cmd(&dir).output().expect("volt doctor");
    let text = stdout(&out);
    assert!(
        text.contains("default solver boolector not found, use --engine z3"),
        "{text}"
    );
    assert!(text.contains("optional bitwuzla not found"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_solver_makes_verify_missing() {
    let dir = temp_dir("nosolver");
    install(&dir, &[("sby", "SBY yosys-0.57"), ("yosys", "Yosys 0.57")]);
    let (_, json) = run_json(doctor_cmd(&dir));
    assert_eq!(cap(&json, "verify")["status"], "missing", "{json}");
    let text = stdout(&doctor_cmd(&dir).output().expect("volt doctor"));
    assert!(text.contains("✗ verify — no SMT solver found"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Takılan araç raporu bekletmez: süre dolunca "unresponsive". stdout
/// dosyaya gider: öldürülen betiğin torun süreci (Windows'ta ping) miras
/// aldığı tutamaçlarla bir boruyu açık tutabilir; ölçülen, volt'un
/// kendi bitişidir.
#[test]
fn hanging_tool_is_unresponsive_within_the_time_limit() {
    let dir = temp_dir("hang");
    fake_tool(&dir, "verilator", Body::Hangs);
    let report = dir.join("report.json");
    let file = std::fs::File::create(&report).expect("rapor dosyası");
    let started = Instant::now();
    let status = doctor_cmd(&dir)
        .args(["--timeout", "1", "--format", "json"])
        .stdout(file)
        .stderr(std::process::Stdio::null())
        .status()
        .expect("volt doctor");
    let took = started.elapsed();
    assert_eq!(status.code(), Some(0));
    let json: Value =
        serde_json::from_str(&std::fs::read_to_string(&report).expect("rapor")).expect("JSON");
    assert_eq!(tool(&json, "verilator")["status"], "unresponsive", "{json}");
    assert_eq!(cap(&json, "simulation")["status"], "missing");
    assert!(took < Duration::from_secs(10), "doctor {took:?} sürdü");
}

#[test]
fn project_manifest_is_found_from_the_working_directory() {
    let dir = temp_dir("project");
    let proj = dir.join("proj");
    std::fs::create_dir_all(proj.join("sub")).expect("dizin");
    std::fs::write(proj.join("Volt.toml"), "[package]\nname = \"proj\"\n").expect("yaz");
    let mut cmd = doctor_cmd(&dir);
    cmd.env_remove("VOLT_MANIFEST_DIR")
        .current_dir(proj.join("sub"));
    let (_, json) = run_json(cmd);
    let found = json["project"]["manifest_dir"].as_str().expect("manifest");
    assert_eq!(
        Path::new(found).canonicalize().unwrap(),
        proj.canonicalize().unwrap()
    );
    assert_eq!(json["project"]["wsl_mount"], false);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn turkish_report_with_lang_flag() {
    let dir = temp_dir("tr");
    let out = doctor_cmd(&dir)
        .args(["--lang", "tr"])
        .output()
        .expect("volt doctor");
    let text = stdout(&out);
    assert!(
        text.contains("✗ test, run — Verilator, C++ derleyicisi, make bulunamadı"),
        "{text}"
    );
    assert!(
        text.contains("bkz.: volt explain simulation-setup"),
        "{text}"
    );
    assert!(text.contains("proje — Volt.toml yok"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}
