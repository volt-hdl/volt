//! Çıktı kümesi (ADR-0042 ek, ADR-0081 Aşama 3 bulgusu): `build` ve
//! `verify` yalnız ana dosyanın modüllerini ve onların örnekleme
//! kapanışını yazar. `use` ile yüklenen dosyanın örneklenmeyen modülleri
//! (ve onların SVA, SDC/XDC, `@mmio` sürücüleri, formal görevleri)
//! çıktıya girmez; yalnız fn içeren dosya SV üretmez.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn multifile(rel: &str) -> PathBuf {
    root().join("tests/ui/multifile").join(rel)
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-unit-out-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

fn build(file: &Path, target: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("VOLT_TOOL_BACKEND", "local")
        .args(["--lang", "en", "build", "--target-dir"])
        .arg(target)
        .args(extra)
        .arg(file)
        .output()
        .expect("volt çalışmalı")
}

/// `target` altındaki tüm dosyalar, göreli ve `/` ayraçlı.
fn files_under(target: &Path) -> BTreeSet<String> {
    fn walk(dir: &Path, base: &Path, out: &mut BTreeSet<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, base, out);
            } else {
                let rel = p.strip_prefix(base).unwrap().to_string_lossy();
                out.insert(rel.replace('\\', "/"));
            }
        }
    }
    let mut out = BTreeSet::new();
    walk(target, target, &mut out);
    out
}

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| (*s).to_string()).collect()
}

#[test]
fn a_fn_only_file_writes_no_sv_and_says_it_is_a_library() {
    let lib = multifile("fn/fnlib.volt");
    for extra in [&[][..], &["--single-file"][..]] {
        let target = temp_dir("fnonly");
        let out = build(&lib, &target, extra);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(0), "{extra:?}: {stderr}");
        assert_eq!(files_under(&target), BTreeSet::new(), "{extra:?}: {stderr}");
        assert!(stderr.contains("0 SV file(s)"), "{stderr}");
        assert!(
            stderr.contains("no module in 'fnlib.volt'") && stderr.contains("use fnlib::"),
            "{stderr}"
        );
        // Simüle/kanıtlanacak modül yok: "Next: volt run" önerilmez.
        assert!(!stderr.contains("volt run"), "{stderr}");
        let _ = std::fs::remove_dir_all(&target);
    }
}

#[test]
fn a_fn_only_file_json_build_lists_no_artifacts() {
    let target = temp_dir("fnonly-json");
    let out = build(&multifile("fn/fnlib.volt"), &target, &["--format", "json"]);
    assert_eq!(out.status.code(), Some(0));
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("JSON");
    assert_eq!(json["success"], true, "{json}");
    assert_eq!(
        json["artifacts"].as_array().map(Vec::len),
        Some(0),
        "{json}"
    );
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn importing_only_a_fn_emits_no_library_module_in_any_output() {
    let target = temp_dir("fnuse");
    let out = build(
        &multifile("fnuse/main.volt"),
        &target,
        &["--emit=sva,sdc,xdc,rust,c,regmap"],
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{stderr}");
    assert_eq!(
        files_under(&target),
        set(&[
            "constraints/Top.sdc",
            "constraints/Top.xdc",
            "formal/top.sva",
            "rtl/Top.sv",
        ]),
        "{stderr}"
    );
    // W0022 yalnız yazılan kısıtın saatleri için: periph.volt'taki
    // Blink/GpioRegs saatleri için değil.
    assert_eq!(stderr.matches("warning[W0022]").count(), 1, "{stderr}");
    assert!(!stderr.contains("periph.volt"), "{stderr}");
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn single_file_build_of_a_fn_import_holds_only_the_root_module() {
    let target = temp_dir("fnuse-single");
    let out = build(&multifile("fnuse/main.volt"), &target, &["--single-file"]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(files_under(&target), set(&["rtl/main.sv"]));
    let sv = std::fs::read_to_string(target.join("rtl/main.sv")).unwrap();
    assert!(sv.contains("module Top ("), "{sv}");
    assert!(
        !sv.contains("module Blink") && !sv.contains("module GpioRegs"),
        "{sv}"
    );
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn an_instantiated_library_module_and_its_children_are_still_emitted() {
    // basic/: Top örnekler lib::Ticker; lib.volt'taki Hidden örneklenmez.
    let target = temp_dir("basic");
    let out = build(&multifile("basic/main.volt"), &target, &[]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(files_under(&target), set(&["rtl/Ticker.sv", "rtl/Top.sv"]));
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn verify_of_a_fn_import_has_a_task_only_for_the_root_module() {
    // sby aranmadan önce .sv + .sby yazılır; sby yoksa çıkış 3.
    let target = temp_dir("fnuse-verify");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("VOLT_TOOL_BACKEND", "local")
        .args(["--lang", "en", "verify", "--target-dir"])
        .arg(&target)
        .arg(multifile("fnuse/main.volt"))
        .env_remove("VOLT_SBY")
        .env("PATH", "")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(
        out.status.code(),
        Some(3),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let sby = std::fs::read_to_string(target.join("formal/main.sby")).expect("main.sby");
    let tasks: Vec<&str> = sby
        .split("[tasks]\n")
        .nth(1)
        .and_then(|t| t.split("\n\n").next())
        .map(|t| t.lines().collect())
        .unwrap_or_default();
    assert_eq!(tasks, ["top"], "{sby}");
    let sv = std::fs::read_to_string(target.join("formal/main.sv")).expect("main.sv");
    assert!(
        !sv.contains("module Blink") && !sv.contains("module GpioRegs"),
        "{sv}"
    );
    let _ = std::fs::remove_dir_all(&target);
}
