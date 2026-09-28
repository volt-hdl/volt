//! ADR-0095 §2: proje kipi — dosya argümansız `check`, `build`, `run`,
//! `verify`. Üst modül Volt.toml `[package] top`'tan ya da çıkarımla
//! (hiç örneklenmemiş modül); birden fazla aday kullanım hatasıdır, sessiz
//! seçim yok. Dosya argümanlı çağrılar değişmez (golden testler).
//!
//! Verilator/sby gerekmez: `run`/`verify` araç aranmadan önce dosyaları
//! yazar; geçersiz `VOLT_VERILATOR`/`VOLT_SBY` ile araç başlatılamaz
//! (çıkış kodu 3) ama seçilen üst modül yazılan dosyalardan okunur.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-proj-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    // `.git` tavanı: Volt.toml araması bu dizinin üstüne çıkmaz (ADR-0061).
    std::fs::create_dir_all(dir.join(".git")).expect(".git");
    dir
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("üst dizin")).expect("dizin");
    std::fs::write(path, text).expect("yaz");
}

const LEAF: &str =
    "pub module Leaf {\n    in  clk : clock\n    in  d   : u4\n    out q   : u4\n    \
                    reg r : u4 = 0\n    on clk { r <= d }\n    q = r\n}\n";

const TOP: &str =
    "use leaf::Leaf;\n\npub module Top {\n    in  clk : clock\n    in  d   : u4\n    \
                   out q   : u4\n    let l = Leaf { clk: clk, d: d }\n    q = l.q\n}\n";

const OTHER: &str = "pub module Other {\n    in  clk : clock\n    out z : bool\n    z = true\n}\n";

/// `leaf.volt` + `top.volt` (+ ek dosyalar), Volt.toml `extra` ile.
fn project(tag: &str, extra: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = temp_dir(tag);
    write(
        &dir.join("Volt.toml"),
        &format!("[package]\nname = \"p\"\nsrc = \".\"\n{extra}"),
    );
    write(&dir.join("leaf.volt"), LEAF);
    write(&dir.join("top.volt"), TOP);
    for (name, text) in files {
        write(&dir.join(name), text);
    }
    dir
}

fn volt(dir: &Path, args: &[&str]) -> Output {
    // Var olan ama çalıştırılamayan "araçlar" (sim_golden deseni).
    for fake in ["no-verilator", "no-sby"] {
        let _ = std::fs::write(dir.join(fake), "not a program");
    }
    Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(args)
        .current_dir(dir)
        .env("VOLT_LANG", "en")
        .env_remove("VOLT_MANIFEST_DIR")
        .env("VOLT_VERILATOR", dir.join("no-verilator"))
        .env("VOLT_SBY", dir.join("no-sby"))
        .env("VOLT_TOOL_BACKEND", "local")
        .output()
        .expect("volt çalışmalı")
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn check_without_a_file_checks_every_source_once() {
    // Arrange: leaf.volt top.volt'un birimine girer — ayrıca denetlenmez.
    let dir = project("check", "", &[("other.volt", OTHER)]);

    // Act
    let out = volt(&dir, &["check"]);

    // Assert
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(0), "{err}");
    assert!(err.contains("Checking other.volt"), "{err}");
    assert!(err.contains("Checking top.volt"), "{err}");
    assert!(!err.contains("Checking leaf.volt"), "{err}");
    assert!(
        err.contains("Next: volt build   (emit SystemVerilog)"),
        "{err}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_reports_errors_of_an_imported_file_once() {
    let bad_leaf = LEAF.replace("q = r", "q = undefined_name");
    let dir = project("check-err", "", &[("leaf.volt", bad_leaf.as_str())]);
    let out = volt(&dir, &["check", "--format", "short"]);
    let text = String::from_utf8_lossy(&out.stdout).into_owned() + &stderr(&out);
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert_eq!(text.matches("undefined_name").count(), 1, "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_json_prints_one_envelope_per_checked_file() {
    let dir = project("check-json", "", &[("other.volt", OTHER)]);
    let out = volt(&dir, &["check", "--format", "json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let envelopes: Vec<serde_json::Value> = serde_json::Deserializer::from_str(&stdout)
        .into_iter()
        .map(|v| v.expect("JSON"))
        .collect();
    assert_eq!(envelopes.len(), 2, "{stdout}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn build_infers_the_only_uninstantiated_module() {
    // Arrange: Top örnekler Leaf'i → tek aday Top.
    let dir = project("infer", "", &[]);

    // Act
    let out = volt(&dir, &["build"]);

    // Assert
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(0), "{err}");
    assert!(err.contains("Compiling top.volt"), "{err}");
    assert!(dir.join("build/rtl/Top.sv").is_file());
    assert!(dir.join("build/rtl/Leaf.sv").is_file());
    assert!(err.contains("Next: volt run      (simulate)"), "{err}");
    assert!(!err.contains("volt run top.volt"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn several_candidates_are_a_usage_error_listing_them() {
    // Arrange: Top ve Other hiçbir yerde örneklenmiyor.
    let dir = project("ambiguous", "", &[("other.volt", OTHER)]);

    for command in ["build", "run", "verify"] {
        // Act
        let out = volt(&dir, &[command]);

        // Assert: sessiz seçim yok, çıktı yok.
        let err = stderr(&out);
        assert_eq!(out.status.code(), Some(2), "{command}: {err}");
        assert!(err.contains("2 top-module candidates"), "{command}: {err}");
        assert!(err.contains("Other (other.volt)"), "{command}: {err}");
        assert!(err.contains("Top (top.volt)"), "{command}: {err}");
        assert!(err.contains("top = \"Other\""), "{command}: {err}");
        assert!(!dir.join("build/rtl").exists(), "{command}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn explicit_top_wins_over_inference() {
    let dir = project("explicit", "top = \"Other\"\n", &[("other.volt", OTHER)]);
    let out = volt(&dir, &["build"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(dir.join("build/rtl/Other.sv").is_file());
    assert!(!dir.join("build/rtl/Top.sv").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_top_list_builds_every_top_file() {
    let dir = project(
        "list",
        "top = [\"Top\", \"Other\"]\n",
        &[("other.volt", OTHER)],
    );
    let out = volt(&dir, &["build"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(0), "{err}");
    assert!(dir.join("build/rtl/Top.sv").is_file(), "{err}");
    assert!(dir.join("build/rtl/Other.sv").is_file(), "{err}");
    // run tek modül ister: --top ile seçilir.
    let run = volt(&dir, &["run"]);
    assert_eq!(run.status.code(), Some(2));
    assert!(
        stderr(&run).contains("volt run --top Top"),
        "{}",
        stderr(&run)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unknown_explicit_top_lists_the_project_modules() {
    let dir = project("unknown", "top = \"Nope\"\n", &[]);
    let out = volt(&dir, &["build"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(2), "{err}");
    assert!(err.contains("top module 'Nope'"), "{err}");
    assert!(err.contains("Leaf, Top"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn run_without_a_file_simulates_the_top_module() {
    // Arrange: dosyada iki modül (Top + Leaf birimde) — dosya kipinde
    // --top gerekirdi; proje kipi Volt.toml'dan seçer.
    let dir = project("run", "top = \"Top\"\n", &[]);

    // Act: Verilator başlatılamaz (çıkış 3) ama testbench yazılmıştır.
    let out = volt(&dir, &["run"]);

    // Assert
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(3), "{err}");
    let tb = std::fs::read_to_string(dir.join("build/sim/top/tb.cpp")).expect("tb.cpp");
    assert!(tb.contains("#include \"VTop.h\""), "{tb}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn run_top_flag_finds_the_module_in_the_project() {
    let dir = project("run-top", "", &[("other.volt", OTHER)]);
    let out = volt(&dir, &["run", "--top", "Other"]);
    assert_eq!(out.status.code(), Some(3), "{}", stderr(&out));
    let tb = std::fs::read_to_string(dir.join("build/sim/other/tb.cpp")).expect("tb.cpp");
    assert!(tb.contains("#include \"VOther.h\""));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn verify_without_a_file_verifies_the_top_file() {
    let dir = project("verify", "top = \"Top\"\n", &[]);
    let out = volt(&dir, &["verify"]);
    let err = stderr(&out);
    assert!(err.contains("Verifying top.volt"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn without_a_project_the_usage_error_points_to_a_file_or_volt_new() {
    let dir = temp_dir("none");
    for command in ["check", "build", "run", "verify"] {
        let out = volt(&dir, &[command]);
        let err = stderr(&out);
        assert_eq!(out.status.code(), Some(2), "{command}: {err}");
        assert!(err.contains("no Volt.toml was found"), "{command}: {err}");
        assert!(
            err.contains(&format!("volt {command} design.volt")),
            "{command}: {err}"
        );
        assert!(err.contains("volt new <name>"), "{command}: {err}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_project_without_sources_is_a_usage_error() {
    let dir = temp_dir("empty");
    write(
        &dir.join("Volt.toml"),
        "[package]\nname = \"e\"\nsrc = \"rtl\"\n",
    );
    let out = volt(&dir, &["check"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(2), "{err}");
    assert!(err.contains("no .volt source files"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_files_are_not_project_sources() {
    // Test dosyasının sarmalayıcı modülü üst modül adayı değildir.
    let wrapper = "pub module Wrap {\n    in clk : clock\n    out z : bool\n    z = true\n}\n";
    let dir = project("tests", "", &[("top_test.volt", wrapper)]);
    let out = volt(&dir, &["build"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(dir.join("build/rtl/Top.sv").is_file());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_argument_ignores_the_project_top() {
    // Dosya verilince eski davranış: Volt.toml `top` okunmaz.
    let dir = project("file-arg", "top = \"Nope\"\n", &[("other.volt", OTHER)]);
    let out = volt(&dir, &["build", "other.volt"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(0), "{err}");
    assert!(err.contains("Next: volt run other.volt"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}
