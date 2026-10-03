//! `volt new` / `volt init` ve şablonların kalıcı doğrulaması (ADR-0084
//! §6). Her şablon `volt new` ile üretilir ve:
//!
//! 1. gömülü kaynakla (`templates/<ad>/`) bayt aynıdır,
//! 2. `volt check` / `volt build` tanısız geçer (uyarı da yok),
//! 3. `volt test` geçer (Verilator; integration işi zorunlu kılar),
//! 4. `volt verify` bmc/prove/cover geçer (sby; verify işi zorunlu kılar).
//!
//! Verilator `-Wall`, Yosys elaborasyonu ve sürücü derlemesi çıktı
//! doğrulama ağındadır (`output_net_tests`, korpusta `templates/`).
//! Bir dil değişikliği şablonu bozarsa bu adımlardan biri düşer — bayat
//! şablon kullanıcının ilk deneyimidir.

mod tools;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tools::Tool;

/// Şablon adı, ana dosya.
const TEMPLATES: &[(&str, &str)] = &[
    ("minimal", "counter.volt"),
    ("cdc", "event_counter.volt"),
    ("fifo", "packet_buffer.volt"),
    ("mmio", "blinker.volt"),
];

fn volt() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_volt"));
    // Araç arka ucu açık: Verilator/sby yoksa Docker'a sessizce düşülmez.
    cmd.env("VOLT_TOOL_BACKEND", "local");
    cmd.env_remove("VOLT_LANG").env_remove("VOLT_MANIFEST_DIR");
    cmd
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("depo kökü")
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-new-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// `volt new <ad> --template <t>` çalıştırır; proje dizinini döndürür.
fn generate(parent: &Path, template: &str) -> PathBuf {
    let name = format!("proj_{template}");
    let out = volt()
        .args(["new", &name, "--template", template])
        .current_dir(parent)
        .output()
        .expect("volt new");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    parent.join(name)
}

fn run_in(dir: &Path, args: &[&str]) -> Output {
    volt().args(args).current_dir(dir).output().expect("volt")
}

#[test]
fn generated_files_are_the_embedded_templates() {
    let parent = temp_dir("bytes");
    for (template, _) in TEMPLATES {
        let dir = generate(&parent, template);
        let src = repo().join("templates").join(template);
        let mut expected: Vec<String> = std::fs::read_dir(&src)
            .expect("şablon dizini")
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        expected.sort();
        let mut actual: Vec<String> = std::fs::read_dir(&dir)
            .expect("proje dizini")
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        actual.sort();
        assert_eq!(actual, expected, "{template}: dosya kümesi");
        let name = format!("proj_{template}");
        for file in &expected {
            let want = std::fs::read_to_string(src.join(file))
                .unwrap()
                .replace("{{name}}", &name);
            let got = std::fs::read_to_string(dir.join(file)).unwrap();
            assert_eq!(got, want, "{template}/{file}");
        }
        let manifest = std::fs::read_to_string(dir.join("Volt.toml")).unwrap();
        assert!(
            manifest.contains(&format!("name = \"{name}\"")),
            "{manifest}"
        );
    }
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn every_template_checks_and_builds_without_diagnostics() {
    let parent = temp_dir("build");
    for (template, main) in TEMPLATES {
        let dir = generate(&parent, template);
        let test_file = main.replace(".volt", "_test.volt");
        for file in [*main, test_file.as_str()] {
            let out = run_in(&dir, &["check", file]);
            assert_eq!(out.status.code(), Some(0), "{template}: {}", text(&out));
            assert!(
                text(&out).contains("0 error(s), 0 warning(s)"),
                "{template}/{file}: {}",
                text(&out)
            );
        }
        let emit = if *template == "mmio" {
            "--emit=sva,c,rust,regmap,regmap-md"
        } else {
            "--emit=sva"
        };
        let out = run_in(&dir, &["build", main, emit]);
        assert_eq!(out.status.code(), Some(0), "{template}: {}", text(&out));
        assert!(
            !text(&out).contains("warning"),
            "{template}: {}",
            text(&out)
        );
        assert!(dir.join("build/rtl").is_dir(), "{template}: RTL yok");
    }
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn mmio_template_drivers_match_the_register_map() {
    let parent = temp_dir("regmap");
    let dir = generate(&parent, "mmio");
    let out = run_in(&dir, &["build", "blinker.volt", "--emit=c,rust,regmap"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    for file in [
        "build/sw/blinker.h",
        "build/sw/blinker.rs",
        "build/sw/blinker.json",
    ] {
        let out = run_in(&dir, &["check-regmap", "blinker.volt", "--against", file]);
        assert_eq!(out.status.code(), Some(0), "{file}: {}", text(&out));
    }
    let _ = std::fs::remove_dir_all(&parent);
}

/// `volt test` argümansız, proje dizininden — README'nin ve `Next:`
/// satırının vaat ettiği akış.
#[test]
fn every_template_passes_volt_test() {
    if tools::require(Tool::Verilator).is_none() {
        return;
    }
    let parent = temp_dir("sim");
    for (template, _) in TEMPLATES {
        let dir = generate(&parent, template);
        let out = run_in(&dir, &["test"]);
        let log = text(&out);
        assert_eq!(out.status.code(), Some(0), "{template}: {log}");
        assert!(log.contains(" passed; 0 failed"), "{template}: {log}");
        assert!(!log.contains("0 passed"), "{template}: test yok\n{log}");
    }
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn every_template_passes_volt_verify_in_every_mode() {
    if tools::require(Tool::Sby).is_none() {
        return;
    }
    let parent = temp_dir("formal");
    for (template, main) in TEMPLATES {
        let dir = generate(&parent, template);
        for mode in ["bmc", "prove", "cover"] {
            let out = run_in(&dir, &["verify", "--mode", mode, main]);
            assert_eq!(
                out.status.code(),
                Some(0),
                "{template} --mode {mode}: {}",
                text(&out)
            );
        }
    }
    let _ = std::fs::remove_dir_all(&parent);
}

// ═══ Komut sözleşmesi (cli-contract.md §9b) ═════════════════════════

#[test]
fn new_refuses_a_non_empty_directory_and_leaves_it_alone() {
    let parent = temp_dir("exists");
    std::fs::create_dir_all(parent.join("taken")).unwrap();
    std::fs::write(parent.join("taken/notes.txt"), "mine").unwrap();
    let out = run_in(&parent, &["new", "taken"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(text(&out).contains("already exists and is not empty"));
    let entries: Vec<_> = std::fs::read_dir(parent.join("taken")).unwrap().collect();
    assert_eq!(entries.len(), 1, "dizine yazıldı");
    // Boş dizin kabul edilir.
    std::fs::create_dir_all(parent.join("empty_dir")).unwrap();
    let out = run_in(&parent, &["new", "empty_dir"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    assert!(parent.join("empty_dir/Volt.toml").is_file());
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn new_rejects_invalid_names_and_unknown_templates() {
    let parent = temp_dir("names");
    for (name, needle) in [
        ("my-design", "try 'my_design'"),
        ("2fast", "try '_2fast'"),
        ("module", "Volt keyword"),
        ("always", "SystemVerilog"),
        ("crate", "Rust"),
    ] {
        let out = run_in(&parent, &["new", name]);
        assert_eq!(out.status.code(), Some(2), "{name}: {}", text(&out));
        assert!(text(&out).contains(needle), "{name}: {}", text(&out));
        assert!(!parent.join(name).exists(), "{name}: dizin oluştu");
    }
    let out = run_in(&parent, &["new", "ok_name", "--template", "nope"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        text(&out).contains("minimal, cdc, fifo, mmio"),
        "{}",
        text(&out)
    );
    assert!(!parent.join("ok_name").exists());
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn next_lines_name_the_project_commands() {
    let parent = temp_dir("next");
    let out = run_in(&parent, &["new", "blinky"]);
    let log = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(log.contains("Created minimal project 'blinky'"), "{log}");
    assert!(log.contains("Next: cd blinky"), "{log}");
    // Proje kipi (ADR-0095): komutlar dosya adı almaz.
    assert!(
        log.contains(
            "            volt check
"
        ),
        "{log}"
    );
    assert!(!log.contains("volt check counter.volt"), "{log}");
    assert!(log.contains("volt test"), "{log}");
    assert!(out.stdout.is_empty(), "durum satırları stderr'e");
    let out = run_in(
        &parent,
        &["--lang", "tr", "new", "ikinci", "--template", "fifo"],
    );
    let log = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(log.contains("Sıradaki: cd ikinci"), "{log}");
    assert!(
        log.contains(
            "            volt check
"
        ),
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn init_writes_into_the_current_directory_but_never_overwrites() {
    let parent = temp_dir("init");
    let proj = parent.join("my_proj");
    std::fs::create_dir_all(&proj).unwrap();
    std::fs::write(proj.join("NOTES.md"), "keep me").unwrap();
    std::fs::write(proj.join("counter.volt"), "// mine").unwrap();

    let out = run_in(&proj, &["init"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(
        text(&out).contains("already has counter.volt"),
        "{}",
        text(&out)
    );
    assert_eq!(
        std::fs::read_to_string(proj.join("counter.volt")).unwrap(),
        "// mine"
    );
    assert!(!proj.join("Volt.toml").exists(), "kısmi yazım");

    std::fs::remove_file(proj.join("counter.volt")).unwrap();
    let out = run_in(&proj, &["init"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let manifest = std::fs::read_to_string(proj.join("Volt.toml")).unwrap();
    assert!(manifest.contains("name = \"my_proj\""), "{manifest}");
    assert_eq!(
        std::fs::read_to_string(proj.join("NOTES.md")).unwrap(),
        "keep me"
    );
    assert!(
        !text(&out).contains("cd "),
        "init cd önermez: {}",
        text(&out)
    );

    // İkinci init: Volt.toml artık var.
    let out = run_in(&proj, &["init", "--template", "cdc"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(text(&out).contains("Volt.toml"), "{}", text(&out));
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn init_needs_a_valid_name_from_the_directory_or_the_flag() {
    let parent = temp_dir("initname");
    let proj = parent.join("my-proj");
    std::fs::create_dir_all(&proj).unwrap();
    let out = run_in(&proj, &["init"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(text(&out).contains("try 'my_proj'"), "{}", text(&out));
    let out = run_in(&proj, &["init", "--name", "my_proj", "--template", "mmio"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    assert!(proj.join("blinker.volt").is_file());
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn list_prints_every_template_to_stdout() {
    let out = volt().args(["new", "--list"]).output().expect("volt");
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    for (template, _) in TEMPLATES {
        assert!(stdout.contains(template), "{stdout}");
    }
    assert!(stdout.contains("(default)"), "{stdout}");
}
