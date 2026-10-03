//! Cover kipinde ulaşılamayan otomatik cover'lar (ADR-0086): yalnız
//! yapısal olarak ÖLÜ olanı (FSM durum grafiğinde resetten yol yok) E5001;
//! yapısal alt sınırı derinliği aşan sayaç cover'ı "--depth ≥ N" notu,
//! kanıtsız olanlar "ulaşılmadı" notu — çıkış 0. Kullanıcı cover'ı her
//! zaman E5001 kalır.
//!
//! Sahte sby (VOLT_SBY) ile araçsız uçtan uca; satır biçimi gerçek sby
//! 0.36 cover çıktısından. Gerçek sby varsa (verify işi) koşum ofseti de
//! ölçülür: 5 artışlık sayaç derinlik 7'de nota, 8'de doğrulamaya düşer.

mod tools;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tools::Tool;

fn volt() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_volt"));
    // Araç arka ucu açık: Verilator/sby yoksa Docker'a sessizce düşülmez.
    cmd.env("VOLT_TOOL_BACKEND", "local");
    cmd
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-cdepth-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// Kullanıcı cover'ı (`c == 1`, cov_0) + otomatik sarma cover'ı
/// (`c == BOUND`, cov_1). BOUND tipin en büyüğü değilse otomatik sınır
/// invariant'ı da üretilir.
fn design(bound: u32) -> String {
    format!(
        "module Deep {{\n    in  clk : clock\n    out q   : u8\n\n    cover: c == 1\n\n    \
         reg c : u8 = 0\n    on clk {{\n        if c != {bound} {{ c <= c + 1 }}\n    }}\n    q = c\n}}\n"
    )
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

fn verify(dir: &Path, sby: &Path, extra: &[&str]) -> Output {
    volt()
        .args([
            "--lang", "en", "verify", "--mode", "cover", "--depth", "12", "-j", "1",
        ])
        .args(extra)
        .arg("--target-dir")
        .arg(dir)
        .arg(dir.join("cdepth.volt"))
        .env("VOLT_SBY", sby)
        .output()
        .expect("volt çalışmalı")
}

/// Tasarımı yazar, üretilen SV'deki `volt:cov_N` satırlarını bulur
/// (ilk koşu sahte sby hatasıyla yalnız dosyaları üretir).
fn prepare(tag: &str, bound: u32) -> (PathBuf, usize, usize) {
    let dir = temp_dir(tag);
    std::fs::write(dir.join("cdepth.volt"), design(bound)).expect("kaynak");
    let sby = write_fake_sby(
        &dir,
        &["SBY 1 [cdepth_deep] DONE (ERROR, rc=16)".into()],
        16,
    );
    verify(&dir, &sby, &[]);
    let sv = std::fs::read_to_string(dir.join("formal").join("cdepth.sv")).expect("üretilen SV");
    let line_of = |marker: &str| {
        sv.lines()
            .position(|l| l.contains(marker))
            .map(|i| i + 1)
            .unwrap_or_else(|| panic!("{marker} yok:\n{sv}"))
    };
    let (user, auto) = (line_of("volt:cov_0"), line_of("volt:cov_1"));
    assert!(sv.contains("auto counter wrap: c == 255"), "{sv}");
    (dir, user, auto)
}

fn unreached(line: usize) -> String {
    format!(
        "SBY 14:36:54 [cdepth_deep] engine_0: ##   0:00:00  Unreached cover statement at cdepth.sv:{line}.20-{line}.60."
    )
}

fn done_fail() -> String {
    "SBY 14:36:54 [cdepth_deep] DONE (FAIL, rc=2)".to_string()
}

#[test]
fn only_a_too_deep_auto_cover_is_a_note_not_e5001() {
    let (dir, _, auto) = prepare("note", 255);
    let sby = write_fake_sby(&dir, &[unreached(auto), done_fail()], 2);
    let out = verify(&dir, &sby, &[]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{err}");
    assert!(!err.contains("E5001"), "{err}");
    assert!(
        err.contains("Deep.cov_1: auto cover 'c == 255'") && err.contains("needs --depth 258"),
        "{err}"
    );
    assert!(
        err.contains("ok ("),
        "ilerleme satırı düzeltilmiş sonucu gösterir: {err}"
    );
    assert!(
        err.contains(
            "Result 1 of 2 properties verified, 1 auto cover(s) not reached at this depth"
        ),
        "{err}"
    );

    let out = verify(&dir, &sby, &["--format", "json"]);
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("JSON");
    let props = json["verify"]["properties"].as_array().expect("dizi");
    let statuses: Vec<&str> = props
        .iter()
        .map(|p| p["status"].as_str().unwrap())
        .collect();
    assert_eq!(statuses, ["pass", "needs-depth"]);
    assert_eq!(props[1]["min_depth"], 258);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_user_cover_next_to_it_is_still_e5001_on_the_user_cover() {
    let (dir, user, auto) = prepare("user", 255);
    let sby = write_fake_sby(&dir, &[unreached(auto), unreached(user), done_fail()], 2);
    let out = verify(&dir, &sby, &[]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(6), "{err}");
    assert!(err.contains("E5001"), "{err}");
    // Tanı kullanıcının cover satırına (5) işaret eder, otomatik köken notu yok.
    assert!(err.contains("cdepth.volt:5:"), "{err}");
    assert!(!err.contains("auto-generated counter wrap"), "{err}");
    assert!(err.contains("needs --depth 258"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unproven_auto_cover_below_its_bound_is_a_note_without_a_bound() {
    // Sınır 5: en az derinlik 8 ≤ 12. Ulaşılmadıysa başka bir koşul
    // geciktiriyor olabilir — ölü olduğu kanıtlanmadı, E5001 değil.
    let (dir, _, auto) = prepare_small("small");
    let sby = write_fake_sby(&dir, &[unreached(auto), done_fail()], 2);
    let out = verify(&dir, &sby, &[]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{err}");
    assert!(
        !err.contains("E5001") && !err.contains("needs --depth"),
        "{err}"
    );
    assert!(err.contains("not proven unreachable"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Resetten hiç girilmeyen durum (tests/ui/fail/71): `1 -> 2` ve `_ -> 0`
/// geçiş cover'ları yapısal olarak ölü.
const DEAD_ARM: &str = "module DeadArm {\n    in  clk  : clock\n    in  go   : bool\n    out busy : bool\n\n    \
                        reg state_r : u2 = 0\n\n    on clk {\n        match state_r {\n            \
                        0 => { if go { state_r <= 0 } }\n            1 => { state_r <= 2 }\n            \
                        _ => { state_r <= 0 }\n        }\n    }\n\n    busy = state_r != 0\n}\n";

#[test]
fn a_structurally_dead_auto_cover_stays_e5001_with_the_proof() {
    let dir = temp_dir("dead");
    std::fs::write(dir.join("cdepth.volt"), DEAD_ARM).expect("kaynak");
    let err_sby = write_fake_sby(
        &dir,
        &["SBY 1 [cdepth_deadarm] DONE (ERROR, rc=16)".into()],
        16,
    );
    verify(&dir, &err_sby, &[]);
    let sv = std::fs::read_to_string(dir.join("formal").join("cdepth.sv")).expect("SV");
    let line = sv
        .lines()
        .position(|l| l.contains("volt:cov_0"))
        .map(|i| i + 1)
        .expect("cov_0");
    let sby = write_fake_sby(
        &dir,
        &[
            format!("SBY 14:36:54 [cdepth_deadarm] engine_0: ##   0:00:00  Unreached cover statement at cdepth.sv:{line}.20-{line}.60."),
            "SBY 14:36:54 [cdepth_deadarm] DONE (FAIL, rc=2)".to_string(),
        ],
        2,
    );
    let out = verify(&dir, &sby, &[]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(6), "{err}");
    assert!(
        err.contains("E5001") && err.contains("structurally unreachable"),
        "{err}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Sınır 5 (invariant da üretilir: inv_0; cover'lar cov_0, cov_1).
fn prepare_small(tag: &str) -> (PathBuf, usize, usize) {
    let dir = temp_dir(tag);
    std::fs::write(dir.join("cdepth.volt"), design(5)).expect("kaynak");
    let sby = write_fake_sby(
        &dir,
        &["SBY 1 [cdepth_deep] DONE (ERROR, rc=16)".into()],
        16,
    );
    verify(&dir, &sby, &[]);
    let sv = std::fs::read_to_string(dir.join("formal").join("cdepth.sv")).expect("SV");
    let line_of = |m: &str| {
        sv.lines()
            .position(|l| l.contains(m))
            .map(|i| i + 1)
            .expect(m)
    };
    (dir.clone(), line_of("volt:cov_0"), line_of("volt:cov_1"))
}

#[test]
fn real_sby_harness_offset_matches_the_measured_bound() {
    // ADR-0086 ölçümü: 0'dan 5'e 5 artış + koşum ofseti 3 = derinlik 8.
    let Some(sby) = tools::require(Tool::Sby) else {
        return;
    };
    let dir = temp_dir("real");
    std::fs::write(dir.join("cdepth.volt"), design(5)).expect("kaynak");
    let run = |depth: &str| {
        volt()
            .args([
                "--lang",
                "en",
                "verify",
                "--mode",
                "cover",
                "--depth",
                depth,
                "-j",
                "1",
                "--target-dir",
            ])
            .arg(&dir)
            .arg(dir.join("cdepth.volt"))
            .env("VOLT_SBY", &sby)
            .output()
            .expect("volt çalışmalı")
    };
    let out = run("7");
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{err}");
    assert!(err.contains("needs --depth 8"), "{err}");
    let out = run("8");
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{err}");
    assert!(!err.contains("needs --depth"), "8'de ulaşılmalı: {err}");
    let _ = std::fs::remove_dir_all(&dir);
}
