//! `volt test` test dosyası keşfi (ADR-0089): proje (Volt.toml) varsa
//! kökten özyinelemeli — `build/`, `target/`, gizli dizinler, iç içe
//! projeler ve `.gitignore`'un düz dizin girdileri atlanır; `[test] paths`
//! taramayı daraltır; proje yoksa yalnız çalışma dizini. Kardeş kuralı
//! (`X_test.volt` → aynı dizindeki `X.volt`) alt dizinde de işler, başka
//! dizindeki tasarım `use` ile gelir.
//!
//! Keşif Verilator'suz sınanır: "Compiling" satırları ve "running N
//! tests" Verilator aranmadan basılır (VOLT_VERILATOR geçersiz yola
//! çevrilir). Verilator varsa (integration işi) testler gerçekten koşar.

mod tools;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tools::Tool;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-disc-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("üst dizin")).expect("dizin");
    std::fs::write(path, text).expect("yaz");
}

const COUNTER: &str = "pub module Counter {\n    in  clk    : clock\n    in  enable : bool\n    \
                       out count  : u8\n    reg c : u8 = 0\n    on clk {\n        if enable { c <= c + 1 }\n    }\n    \
                       count = c\n}\n";

fn counter_test(name: &str, prelude: &str) -> String {
    format!(
        "{prelude}test \"{name}\" {{\n    let dut = Counter {{ }};\n    dut.enable = true;\n    \
         step(3);\n    assert_eq(dut.count, 3);\n}}\n"
    )
}

/// Kök + alt dizinler + atlanması gerekenler.
fn project(tag: &str, manifest_extra: &str) -> PathBuf {
    let root = temp_dir(tag);
    // `.git` tavanı: arama bu projenin üstüne çıkmaz (ADR-0061).
    std::fs::create_dir_all(root.join(".git")).expect(".git");
    write(
        &root.join("Volt.toml"),
        &format!("[package]\nname = \"disc\"\nsrc = \".\"\n{manifest_extra}"),
    );
    write(&root.join(".gitignore"), "vendor/\n/out\n*.log\n");
    write(&root.join("counter.volt"), COUNTER);
    write(&root.join("counter_test.volt"), &counter_test("root", ""));
    // Kardeş kuralı alt dizinde: rtl/counter_test.volt → rtl/counter.volt.
    write(&root.join("rtl/counter.volt"), COUNTER);
    write(
        &root.join("rtl/counter_test.volt"),
        &counter_test("sibling", ""),
    );
    // Başka dizindeki tasarım `use` ile (kaynak kökü ".").
    write(
        &root.join("tests/deep/uses_test.volt"),
        &counter_test("uses", "use rtl::counter::Counter\n\n"),
    );
    // Atlananlar.
    for skipped in [
        "build/b_test.volt",
        "target/t_test.volt",
        ".hidden/h_test.volt",
        "vendor/v_test.volt",
        "out/o_test.volt",
        "nested/n_test.volt",
    ] {
        write(&root.join(skipped), &counter_test("skipped", ""));
    }
    write(
        &root.join("nested/Volt.toml"),
        "[package]\nname = \"nested\"\n",
    );
    root
}

/// Verilator'suz koşu: keşif çıktısı.
fn run_test(cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "test"])
        .current_dir(cwd)
        .env("VOLT_VERILATOR", cwd.join("no-such-verilator"))
        .env_remove("VOLT_MANIFEST_DIR")
        .output()
        .expect("volt çalışmalı")
}

/// "Compiling" satırlarındaki yollar, `/` ayırıcıyla.
fn compiled(out: &Output) -> Vec<String> {
    String::from_utf8_lossy(&out.stderr)
        .lines()
        .filter_map(|l| l.trim().strip_prefix("Compiling "))
        .map(|p| p.replace('\\', "/"))
        .collect()
}

#[test]
fn project_tests_are_found_recursively_from_the_manifest_root() {
    let root = project("root", "");
    let out = run_test(&root);
    assert_eq!(
        compiled(&out),
        [
            "./counter_test.volt",
            "./rtl/counter_test.volt",
            "./tests/deep/uses_test.volt"
        ],
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("running 3 tests"), "{stdout}");
    // Derleme hatası yok (1 değil): Verilator yoksa 3, PATH'te varsa
    // testler koşar ve geçer (0).
    assert!(
        matches!(out.status.code(), Some(0 | 3)),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_subdirectory_run_still_finds_the_whole_project() {
    let root = project("sub", "");
    let out = run_test(&root.join("rtl"));
    assert_eq!(
        compiled(&out),
        [
            "../counter_test.volt",
            "./counter_test.volt",
            "../tests/deep/uses_test.volt"
        ],
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn test_paths_narrow_the_scan() {
    let root = project("paths", "\n[test]\npaths = [\"rtl\", \"tests\"]\n");
    let out = run_test(&root);
    assert_eq!(
        compiled(&out),
        ["./rtl/counter_test.volt", "./tests/deep/uses_test.volt"],
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn without_a_manifest_only_the_working_directory_is_scanned() {
    let dir = temp_dir("flat");
    std::fs::create_dir_all(dir.join(".git")).expect(".git");
    write(&dir.join("counter.volt"), COUNTER);
    write(&dir.join("counter_test.volt"), &counter_test("flat", ""));
    write(&dir.join("sub/counter.volt"), COUNTER);
    write(&dir.join("sub/counter_test.volt"), &counter_test("sub", ""));
    let out = run_test(&dir);
    assert_eq!(compiled(&out), ["./counter_test.volt"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_bare_file_name_in_a_project_subdirectory_finds_the_manifest() {
    // Önce `volt check counter_test.volt` (tests/ içinde) E1011 veriyordu:
    // manifest araması göreli "." yolunun başında duruyordu.
    let root = project("bare", "");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "check", "uses_test.volt"])
        .current_dir(root.join("tests/deep"))
        .env_remove("VOLT_MANIFEST_DIR")
        .output()
        .expect("volt çalışmalı");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn real_verilator_runs_every_discovered_test() {
    let Some(verilator) = tools::require(Tool::Verilator) else {
        return;
    };
    let root = project("real", "");
    // Aynı adlı iki alt dizin testi ayrı sim dizinine yazılır.
    write(&root.join("a/counter.volt"), COUNTER);
    write(&root.join("a/counter_test.volt"), &counter_test("a", ""));
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "test"])
        .current_dir(&root)
        .env("VOLT_VERILATOR", &verilator)
        .env_remove("VOLT_MANIFEST_DIR")
        .output()
        .expect("volt çalışmalı");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && stdout.contains("test result: ok. 4 passed"),
        "{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    for key in ["counter_test", "a/counter_test", "rtl/counter_test"] {
        assert!(root.join("build/sim").join(key).is_dir(), "build/sim/{key}");
    }
    let _ = std::fs::remove_dir_all(&root);
}
