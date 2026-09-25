//! `extern module` SV kaynağı (ADR-0076): `@source("...")` yolu
//! `check`/`build`'de denetlenir (E1012), `run`/`test`/`verify` dosyayı
//! araca üretilen SV ile birlikte verir; örneklenen extern'ün kaynağı
//! yoksa bu üç komut E1012 verir, `build`/`check` vermez.
//!
//! Araçsız testler her ortamda koşar (sahte sby; çalıştırılamayan
//! `VOLT_VERILATOR` — sürücü dosyaları yazar, araç başlatılamaz). Gerçek
//! Verilator / sby testleri araç PATH'te yoksa atlanır.

mod tools;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn volt() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_volt"));
    cmd.env("VOLT_LANG", "en");
    cmd
}

fn repo(rel: &str) -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")).join(rel)
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-extern-src-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

const FIXTURE: &str = "tests/fixtures/extern_source/ext_top.volt";
const FIXTURE_TEST: &str = "tests/fixtures/extern_source/ext_top_test.volt";

/// Fikstürü `@source` satırları olmadan geçici dizine yazar.
fn without_source(dir: &Path) -> PathBuf {
    let src = std::fs::read_to_string(repo(FIXTURE)).expect("fikstür");
    let stripped: String = src
        .lines()
        .filter(|l| !l.starts_with("@source"))
        .map(|l| format!("{l}\n"))
        .collect();
    let path = dir.join("ext_top.volt");
    std::fs::write(&path, stripped).expect("yazılmalı");
    path
}

fn check_json(file: &Path) -> serde_json::Value {
    let out = volt()
        .args(["check", "--format=json"])
        .arg(file)
        .output()
        .expect("volt çalışmalı");
    serde_json::from_slice(&out.stdout).expect("json")
}

fn codes(env: &serde_json::Value) -> Vec<String> {
    env["diagnostics"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .map(|d| d["code"].as_str().unwrap_or_default().to_string())
        .collect()
}

/// `//~ KOD` + `//~^ ERROR` (üst satır) — `volt check`.
fn assert_ui_fail(rel: &str) {
    let file = repo(rel);
    let src = std::fs::read_to_string(&file).expect("okunmalı");
    let code = src
        .lines()
        .next()
        .and_then(|l| l.strip_prefix("//~ "))
        .expect("//~ KOD")
        .trim();
    let line = src
        .lines()
        .position(|l| l.trim_start().starts_with("//~^ ERROR"))
        .expect("//~^ ERROR") as u64;
    let env = check_json(&file);
    let found: Vec<u64> = env["diagnostics"]
        .as_array()
        .expect("tanılar")
        .iter()
        .filter(|d| d["code"] == code)
        .filter_map(|d| d["spans"][0]["start"]["line"].as_u64())
        .collect();
    assert!(
        found.contains(&line),
        "{rel}: {code} satır {line}, bulunan {found:?}\n{env:#}"
    );
}

// ═══ check / build ════════════════════════════════════════════════

#[test]
fn ui_fail_99_missing_source_file_e1012() {
    assert_ui_fail("tests/ui/fail/99_extern_source_missing_file.volt");
}

#[test]
fn ui_fail_100_source_on_volt_module_e0009() {
    assert_ui_fail("tests/ui/fail/100_source_on_volt_module.volt");
}

#[test]
fn fixture_and_ui_pass_check_clean() {
    for rel in [
        FIXTURE,
        FIXTURE_TEST,
        "tests/ui/pass/101_extern_source.volt",
    ] {
        let env = check_json(&repo(rel));
        assert!(codes(&env).is_empty(), "{rel}: {:?}", codes(&env));
    }
}

#[test]
fn source_leaving_the_project_is_e1012() {
    let dir = temp_dir("outside");
    let proj = dir.join("proj");
    std::fs::create_dir_all(&proj).expect("dizin");
    std::fs::write(proj.join("Volt.toml"), "[package]\nname = \"p\"\n").expect("toml");
    std::fs::write(dir.join("secret.sv"), "module ExtDelay; endmodule\n").expect("sv");
    let file = proj.join("top.volt");
    let src = std::fs::read_to_string(repo("tests/ui/pass/101_extern_source.volt"))
        .expect("pass")
        .replace("rtl/101_ext_delay.sv", "../secret.sv");
    std::fs::write(&file, src).expect("volt");
    let env = check_json(&file);
    assert_eq!(codes(&env), ["E1012"]);
    let msg = env["diagnostics"][0]["message"]
        .as_str()
        .unwrap_or_default();
    assert!(msg.contains("leaves the project"), "{msg}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `build` extern gövdesini istemez: `@source`'suz tasarım derlenir ve
/// çıktı `@source`'lu hâliyle aynıdır (yalnız örnekleme).
#[test]
fn build_does_not_need_the_source_and_its_output_is_unchanged() {
    let dir = temp_dir("build");
    let nosrc = without_source(&dir);
    let build = |file: &Path, out: &str| {
        let target = dir.join(out);
        let o = volt()
            .args(["build", "--target-dir"])
            .arg(&target)
            .arg(file)
            .output()
            .expect("volt");
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        std::fs::read_to_string(target.join("rtl").join("ExtTop.sv")).expect("ExtTop.sv")
    };
    let with = build(&repo(FIXTURE), "a");
    let without = build(&nosrc, "b");
    let body = |s: &str| {
        s.split("module ExtTop")
            .nth(1)
            .unwrap_or_default()
            .to_string()
    };
    assert_eq!(body(&with), body(&without));
    let _ = std::fs::remove_dir_all(&dir);
}

// ═══ run / test / verify — kaynaksız extern E1012 ═══════════════════

#[test]
fn run_test_verify_without_source_are_e1012_exit_1() {
    let dir = temp_dir("missing");
    let file = without_source(&dir);
    std::fs::copy(repo(FIXTURE_TEST), dir.join("ext_top_test.volt")).expect("test");
    let cases: [(&str, Vec<&str>, PathBuf); 3] = [
        ("run", vec!["run", "--cycles", "2"], file.clone()),
        ("test", vec!["test"], dir.join("ext_top_test.volt")),
        ("verify", vec!["verify"], file.clone()),
    ];
    for (command, args, input) in cases {
        let out = volt()
            .args(&args)
            .arg("--target-dir")
            .arg(dir.join("out"))
            .arg(&input)
            .env("VOLT_VERILATOR", dir.join("no-such-verilator"))
            .env("VOLT_SBY", dir.join("no-such-sby"))
            .output()
            .expect("volt");
        let err = stderr(&out);
        assert_eq!(out.status.code(), Some(1), "{command}: {err}");
        assert!(
            err.contains(&format!(
                "error[E1012]: extern module 'ExtInvert' has no SystemVerilog source; 'volt {command}' needs its body"
            )),
            "{command}: {err}"
        );
        assert!(
            err.contains("@source(\"rtl/ExtDelay.sv\")"),
            "{command}: {err}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

// ═══ Dosyalar araca verilir ════════════════════════════════════════

/// verify: extern gövdesi `build/formal/`'a kopyalanır, `.sby`'de
/// üretilen SV'den ÖNCE okunur (sahte sby PASS basar).
#[test]
fn verify_hands_extern_source_to_sby() {
    let dir = temp_dir("verify");
    #[cfg(windows)]
    let sby = {
        let p = dir.join("sby.bat");
        std::fs::write(
            &p,
            "@echo off\r\necho SBY [ext_top_exttop] DONE (PASS, rc=0)\r\nexit /b 0\r\n",
        )
        .expect("sby");
        p
    };
    #[cfg(unix)]
    let sby = {
        use std::os::unix::fs::PermissionsExt;
        let p = dir.join("sby");
        std::fs::write(
            &p,
            "#!/bin/sh\necho 'SBY [ext_top_exttop] DONE (PASS, rc=0)'\nexit 0\n",
        )
        .expect("sby");
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        p
    };
    let out = volt()
        .args(["verify", "--target-dir"])
        .arg(&dir)
        .arg(repo(FIXTURE))
        .env("VOLT_SBY", &sby)
        .output()
        .expect("volt");
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let formal = dir.join("formal");
    let copied = std::fs::read_to_string(formal.join("extern_ext_ops.sv")).expect("kopya");
    assert!(copied.contains("module ExtInvert"), "{copied}");
    let sby_text = std::fs::read_to_string(formal.join("ext_top.sby")).expect(".sby");
    assert!(
        sby_text.contains("[script]\nread_verilog -sv -noassert -noassume extern_ext_ops.sv\nread -formal ext_top.sv\n"),
        "{sby_text}"
    );
    assert!(
        sby_text.contains("[files]\nextern_ext_ops.sv\next_top.sv\n"),
        "{sby_text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Argümanlarını çalışma dizinindeki `verilator_args.txt`'ye yazıp
/// başarısız çıkan sahte Verilator.
fn arg_dumping_verilator(dir: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let p = dir.join("verilator.bat");
        std::fs::write(
            &p,
            "@echo off\r\necho %*> verilator_args.txt\r\nexit /b 1\r\n",
        )
        .expect("sahte");
        p
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let p = dir.join("verilator");
        std::fs::write(&p, "#!/bin/sh\necho \"$@\" > verilator_args.txt\nexit 1\n").expect("sahte");
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        p
    }
}

/// Verilator'a extern gövdesi üretilen SV'den ÖNCE verilir.
fn assert_extern_before_design(sim_dir: &Path) {
    let args = std::fs::read_to_string(sim_dir.join("verilator_args.txt")).expect("argümanlar");
    let ext = args.find("extern_ext_ops.sv").expect(&args);
    let design = args.find("ExtTop.sv").expect(&args);
    assert!(ext < design, "{args}");
}

/// run/test: extern gövdesi sim dizinine kopyalanır ve Verilator
/// girdisidir (üretilen SV'den önce).
#[test]
fn run_and_test_hand_extern_source_to_verilator() {
    let dir = temp_dir("sim-args");
    let fake = arg_dumping_verilator(&dir);
    let run = volt()
        .args(["run", "--cycles", "2", "--target-dir"])
        .arg(dir.join("r"))
        .arg(repo(FIXTURE))
        .env("VOLT_VERILATOR", &fake)
        .output()
        .expect("volt");
    assert_ne!(run.status.code(), Some(1), "{}", stderr(&run));
    assert_extern_before_design(&dir.join("r").join("sim").join("ext_top"));
    let test = volt()
        .args(["test", "--target-dir"])
        .arg(dir.join("t"))
        .arg(repo(FIXTURE_TEST))
        .env("VOLT_VERILATOR", &fake)
        .output()
        .expect("volt");
    assert_ne!(test.status.code(), Some(1), "{}", stderr(&test));
    assert_extern_before_design(&dir.join("t").join("sim").join("ext_top_test"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// run/test: extern gövdesi sim dizinine kopyalanır (Verilator girdisi);
/// araç başlatılamasa da dosyalar yazılmış olmalı.
#[test]
fn run_and_test_stage_extern_source_for_verilator() {
    let dir = temp_dir("sim");
    let fake = dir.join("fake-verilator.txt");
    std::fs::write(&fake, "not a program").expect("sahte");
    let run = volt()
        .args(["run", "--cycles", "2", "--target-dir"])
        .arg(dir.join("r"))
        .arg(repo(FIXTURE))
        .env("VOLT_VERILATOR", &fake)
        .output()
        .expect("volt");
    assert_ne!(run.status.code(), Some(1), "{}", stderr(&run));
    let staged = dir
        .join("r")
        .join("sim")
        .join("ext_top")
        .join("extern_ext_ops.sv");
    assert!(staged.is_file(), "{}", stderr(&run));

    let test = volt()
        .args(["test", "--target-dir"])
        .arg(dir.join("t"))
        .arg(repo(FIXTURE_TEST))
        .env("VOLT_VERILATOR", &fake)
        .output()
        .expect("volt");
    assert_ne!(test.status.code(), Some(1), "{}", stderr(&test));
    let staged = dir
        .join("t")
        .join("sim")
        .join("ext_top_test")
        .join("extern_ext_ops.sv");
    assert!(staged.is_file(), "{}", stderr(&test));
    let _ = std::fs::remove_dir_all(&dir);
}

// ═══ Gerçek araçlar (PATH'te yoksa atlanır) ══════════════════════════

/// ADR-0079 §3: `VOLT_REQUIRE_TOOLS` içindeki araç yoksa test düşer.
fn on_path(tool: &str) -> bool {
    let tool = match tool {
        "verilator" => tools::Tool::Verilator,
        "sby" => tools::Tool::Sby,
        other => panic!("bilinmeyen araç {other}"),
    };
    tools::require(tool).is_some()
}

#[test]
fn real_verilator_runs_the_extern_body() {
    if !on_path("verilator") {
        eprintln!("SKIP: verilator yok");
        return;
    }
    let dir = temp_dir("real-sim");
    let out = volt()
        .args(["test", "--target-dir"])
        .arg(&dir)
        .arg(repo(FIXTURE_TEST))
        .env_remove("VOLT_VERILATOR")
        .output()
        .expect("volt");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "{stdout}\n{}", stderr(&out));
    assert!(stdout.contains("1 passed; 0 failed"), "{stdout}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn real_sby_proves_the_invariant_with_the_extern_body() {
    if !on_path("sby") {
        eprintln!("SKIP: sby yok");
        return;
    }
    let dir = temp_dir("real-formal");
    let out = volt()
        .args(["verify", "--mode", "prove", "--depth", "4", "--target-dir"])
        .arg(&dir)
        .arg(repo(FIXTURE))
        .env_remove("VOLT_SBY")
        .output()
        .expect("volt");
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let _ = std::fs::remove_dir_all(&dir);
}
