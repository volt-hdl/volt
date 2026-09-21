//! `volt check-regmap` ve `volt build --check-regmap` (ADR-0063) — CLI
//! sözleşmesi: çıkış kodları, insan/JSON çıktısı, üç biçim, bayat dosya
//! senaryosu ve Seviye 1'in SV mutasyonlarını yakalaması.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

use volt_sw_emit::check::{self, DriftKind, Format};

fn volt() -> Command {
    Command::new(env!("CARGO_BIN_EXE_volt"))
}

fn root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-regmap-test-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

fn gpio() -> PathBuf {
    root().join("examples/soc/gpio.volt")
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// `volt build --emit=c,rust,regmap` → hedef dizin (başarı beklenir).
fn generate(tag: &str, source: &Path) -> PathBuf {
    let target = temp_dir(tag);
    let o = volt()
        .args([
            "--lang",
            "en",
            "build",
            "--emit=c,rust,regmap",
            "--target-dir",
        ])
        .arg(&target)
        .arg(source)
        .output()
        .expect("volt çalışmalı");
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    target
}

fn check_regmap(design: &Path, against: &[&Path], extra: &[&str]) -> Output {
    let mut cmd = volt();
    cmd.args(["--lang", "en", "check-regmap"])
        .args(extra)
        .arg(design);
    for a in against {
        cmd.arg("--against").arg(a);
    }
    cmd.output().expect("volt çalışmalı")
}

fn json_of(o: &Output) -> serde_json::Value {
    serde_json::from_slice(&o.stdout).unwrap_or_else(|e| {
        panic!(
            "stdout JSON olmalı ({e}): {}",
            String::from_utf8_lossy(&o.stdout)
        )
    })
}

/// `gpio.volt`'un eski sürümü: `dir` 0x0C'de ve bugün olmayan `irq_en`
/// register'ı var (firmware deposunda kalan bayat başlığın kaynağı).
fn old_gpio(dir: &Path) -> PathBuf {
    let src = std::fs::read_to_string(gpio()).unwrap();
    let old = src
        .replacen(
            "@reg(offset = 0x04, access = ReadWrite)\n    dir",
            "@reg(offset = 0x0C, access = ReadWrite)\n    dir",
            1,
        )
        .replacen(
            "    on clk {",
            "    @reg(offset = 0x10, access = ReadWrite)\n    irq_en : { enable : bool, @reserved : bits<31> }\n\n    on clk {",
            1,
        );
    assert_ne!(old, src, "mutasyon uygulanmalı");
    let path = dir.join("gpio.volt");
    std::fs::write(&path, old).unwrap();
    path
}

// ═══ Seviye 2: volt check-regmap ═══════════════════════════════════

#[test]
fn freshly_generated_files_match_in_all_three_formats() {
    let t = generate("fresh", &gpio());
    let files = ["sw/gpio.h", "sw/gpio.rs", "sw/gpio.json"].map(|f| t.join(f));
    let refs: Vec<&Path> = files.iter().map(PathBuf::as_path).collect();
    let o = check_regmap(&gpio(), &refs, &[]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let err = stderr(&o);
    assert_eq!(err.matches("Match ").count(), 3, "{err}");
    assert!(err.contains("Result 3 of 3 file(s) match"), "{err}");
    assert!(err.contains("hash match"), "{err}");
    let _ = std::fs::remove_dir_all(&t);
}

#[test]
fn stale_header_reports_offset_and_extra_register_with_exit_1() {
    let t = temp_dir("stale-src");
    let old = generate("stale", &old_gpio(&t));
    let o = check_regmap(&gpio(), &[&old.join("sw/gpio.h")], &[]);
    assert_eq!(o.status.code(), Some(1), "{}", stderr(&o));
    let err = stderr(&o);
    assert!(err.contains("error[E9003]: register map drift in"), "{err}");
    assert!(
        err.contains("= note: GPIO_DIR offset: file 0x0C, RTL 0x04"),
        "{err}"
    );
    assert!(
        err.contains("= note: extra in file: GPIO_IRQ_EN (0x10), not in RTL"),
        "{err}"
    );
    assert!(err.contains("the file is stale"), "{err}");
    assert!(
        err.contains("= help: regenerate with volt build --emit=c"),
        "{err}"
    );
    assert!(err.contains("volt explain E9003"), "{err}");
    let _ = std::fs::remove_dir_all(&t);
    let _ = std::fs::remove_dir_all(&old);
}

#[test]
fn register_missing_from_an_old_file_is_reported_missing() {
    // Ters yön: dosya bugünkü gpio'dan, tasarım irq_en'i olan yeni sürüm.
    let t = temp_dir("missing-src");
    let newer = old_gpio(&t);
    let files = generate("missing", &gpio());
    let o = check_regmap(&newer, &[&files.join("sw/gpio.rs")], &[]);
    assert_eq!(o.status.code(), Some(1), "{}", stderr(&o));
    assert!(
        stderr(&o).contains("= note: missing in file: GPIO_IRQ_EN (0x10)"),
        "{}",
        stderr(&o)
    );
    let _ = std::fs::remove_dir_all(&t);
    let _ = std::fs::remove_dir_all(&files);
}

#[test]
fn json_output_lists_drift_items_for_ci() {
    let t = temp_dir("json-src");
    let old = generate("json", &old_gpio(&t));
    let fresh = generate("json-fresh", &gpio());
    let o = check_regmap(
        &gpio(),
        &[&old.join("sw/gpio.json"), &fresh.join("sw/gpio.json")],
        &["--format", "json"],
    );
    assert_eq!(o.status.code(), Some(1));
    let v = json_of(&o);
    assert_eq!(v["command"], "check-regmap");
    assert_eq!(v["success"], false);
    assert_eq!(v["diagnostics"][0]["code"], "E9003");
    let files = v["regmap_check"]["files"].as_array().unwrap();
    assert_eq!(files.len(), 2);
    assert_eq!(files[0]["status"], "drift");
    assert_eq!(files[0]["format"], "regmap");
    assert_eq!(files[0]["cause"], "stale");
    let kinds: Vec<&str> = files[0]["drift"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["offset", "extra_register"]);
    assert_eq!(files[0]["drift"][0]["file"], "0x0C");
    assert_eq!(files[0]["drift"][0]["rtl"], "0x04");
    assert_eq!(files[1]["status"], "match");
    assert_eq!(files[1]["fast_path"], true);
    assert_eq!(
        files[1]["design_hash"],
        v["regmap_check"]["design_hashes"]["Gpio"]
    );
    let _ = std::fs::remove_dir_all(&t);
    let _ = std::fs::remove_dir_all(&old);
    let _ = std::fs::remove_dir_all(&fresh);
}

#[test]
fn hand_edited_rust_driver_is_reported_as_edited() {
    let t = generate("edited", &gpio());
    let rs = t.join("sw/gpio.rs");
    let text = std::fs::read_to_string(&rs).unwrap();
    let edited = text.replacen(
        "pub const DATA_IN_MASK: u32 = 0x0000_00FF;",
        "pub const DATA_IN_MASK: u32 = 0x0000_FFFF;",
        1,
    );
    assert_ne!(text, edited);
    std::fs::write(&rs, edited).unwrap();
    let o = check_regmap(&gpio(), &[&rs], &[]);
    assert_eq!(o.status.code(), Some(1));
    let err = stderr(&o);
    assert!(
        err.contains("GPIO_DATA_IN mask: file 0x0000FFFF, RTL 0x000000FF"),
        "{err}"
    );
    assert!(err.contains("edited after generation"), "{err}");
    let _ = std::fs::remove_dir_all(&t);
}

#[test]
fn non_volt_files_get_an_explicit_e9004() {
    let t = temp_dir("notvolt");
    let hand = t.join("gpio.h");
    std::fs::write(
        &hand,
        "#define GPIO_BASE 0x0U\n#define GPIO_DIR_OFFSET 0x04U\n",
    )
    .unwrap();
    let txt = t.join("gpio.txt");
    std::fs::write(&txt, "whatever\n").unwrap();
    let o = check_regmap(&gpio(), &[&hand, &txt], &["--format", "json"]);
    assert_eq!(o.status.code(), Some(1));
    let v = json_of(&o);
    let codes: Vec<&str> = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].as_str().unwrap())
        .collect();
    assert_eq!(codes, ["E9004", "E9004"]);
    assert_eq!(v["regmap_check"]["files"][0]["reason"], "not_volt");
    assert_eq!(v["regmap_check"]["files"][1]["reason"], "extension");

    let o = check_regmap(&gpio(), &[&hand], &[]);
    let err = stderr(&o);
    assert!(err.contains("error[E9004]"), "{err}");
    assert!(
        err.contains("is not a Volt-generated register map"),
        "{err}"
    );
    assert!(err.contains("hand-written files are not parsed"), "{err}");
    let _ = std::fs::remove_dir_all(&t);
}

#[test]
fn unreadable_file_is_exit_3_and_design_without_mmio_is_exit_2() {
    let t = temp_dir("io");
    let o = check_regmap(&gpio(), &[&t.join("absent.h")], &[]);
    assert_eq!(o.status.code(), Some(3), "{}", stderr(&o));
    let files = generate("io-files", &gpio());
    let plain = root().join("tests/fixtures/counter.volt");
    let o = check_regmap(&plain, &[&files.join("sw/gpio.h")], &[]);
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
    assert!(stderr(&o).contains("has no @mmio module"), "{}", stderr(&o));
    let _ = std::fs::remove_dir_all(&t);
    let _ = std::fs::remove_dir_all(&files);
}

#[test]
fn against_is_required() {
    let o = volt().args(["check-regmap"]).arg(gpio()).output().unwrap();
    assert_eq!(o.status.code(), Some(2), "clap kullanım hatası");
}

// ═══ Seviye 1: volt build --check-regmap ═══════════════════════════

const DESIGNS: &[&str] = &[
    "examples/soc/gpio.volt",
    "tests/ui/pass/57_mmio_basic.volt",
    "tests/ui/pass/58_mmio_access_control.volt",
    "tests/ui/pass/72_mmio_driver_generation.volt",
];

#[test]
fn build_check_regmap_passes_for_every_mmio_design() {
    for design in DESIGNS {
        let t = temp_dir("l1");
        let o = volt()
            .args([
                "--lang",
                "en",
                "build",
                "--emit=c,rust,regmap",
                "--check-regmap",
                "--target-dir",
            ])
            .arg(&t)
            .arg(root().join(design))
            .output()
            .unwrap();
        assert_eq!(o.status.code(), Some(0), "{design}: {}", stderr(&o));
        assert!(
            stderr(&o).contains("Regmap drivers match the RTL address decode (1 @mmio module(s))"),
            "{design}: {}",
            stderr(&o)
        );
        let _ = std::fs::remove_dir_all(&t);
    }
}

#[test]
fn build_check_regmap_without_emit_checks_in_memory_and_writes_no_driver() {
    let t = temp_dir("l1-mem");
    let o = volt()
        .args(["build", "--check-regmap", "--target-dir"])
        .arg(&t)
        .arg(gpio())
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(!t.join("sw").exists(), "--emit yoksa sürücü yazılmaz");
    let _ = std::fs::remove_dir_all(&t);
}

#[test]
fn build_check_regmap_on_a_design_without_mmio_notes_it() {
    let t = temp_dir("l1-none");
    let o = volt()
        .args(["--lang", "en", "build", "--check-regmap", "--target-dir"])
        .arg(&t)
        .arg(root().join("tests/fixtures/counter.volt"))
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0));
    assert!(
        stderr(&o).contains("--check-regmap checked nothing"),
        "{}",
        stderr(&o)
    );
    let _ = std::fs::remove_dir_all(&t);
}

/// Üretilen SV'yi okuyup `mutate` ile bozar, pwm_regs sürücüsüyle karşılaştırır.
fn l1_drift(mutate: impl Fn(&str) -> String) -> Vec<(DriftKind, String)> {
    // Testler paralel koşar: her çağrı kendi dizinini alır.
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let tag = format!("l1-mut-{}", NEXT.fetch_add(1, Ordering::Relaxed));
    let t = generate(
        &tag,
        &root().join("tests/ui/pass/72_mmio_driver_generation.volt"),
    );
    let sv = std::fs::read_to_string(t.join("rtl/PwmRegs.sv")).unwrap();
    let bad = mutate(&sv);
    assert_ne!(bad, sv, "SV mutasyonu uygulanmalı");
    let mut out = Vec::new();
    for f in ["sw/pwm_regs.h", "sw/pwm_regs.rs", "sw/pwm_regs.json"] {
        let fmt = Format::from_path(Path::new(f)).unwrap();
        let text = std::fs::read_to_string(t.join(f)).unwrap();
        let view = check::parse(fmt, &text).unwrap().view;
        assert!(
            check::check_rtl(&view, &sv).is_empty(),
            "{f}: temiz SV uyumlu"
        );
        let drift = check::check_rtl(&view, &bad);
        if out.is_empty() {
            out = drift.iter().map(|d| (d.kind, d.subject.clone())).collect();
        } else {
            let again: Vec<_> = drift.iter().map(|d| (d.kind, d.subject.clone())).collect();
            assert_eq!(again, out, "{f}: üç biçim aynı kalemi görmeli");
        }
    }
    let _ = std::fs::remove_dir_all(&t);
    out
}

#[test]
fn level1_catches_an_address_decode_shift_in_the_sv() {
    let d = l1_drift(|sv| sv.replace("32'h40010004", "32'h40010014"));
    assert!(
        d.contains(&(DriftKind::Offset, "PWM_REGS_DUTY".to_string())),
        "{d:?}"
    );
    assert!(
        d.iter().any(|(k, _)| *k == DriftKind::MissingRegister),
        "{d:?}"
    );
}

#[test]
fn level1_catches_a_read_mask_change_in_the_sv() {
    let d = l1_drift(|sv| sv.replace("mmio_duty & 32'hFFFF;", "mmio_duty & 32'hFFF;"));
    assert_eq!(d, [(DriftKind::Mask, "PWM_REGS_DUTY".to_string())]);
}

#[test]
fn level1_catches_a_lost_write_arm_as_access_drift() {
    // `aw_addr` çözümlemesinden period kolu silinirse RTL onu yazamaz.
    let d = l1_drift(|sv| {
        let arm = sv.find("case (aw_addr)").unwrap();
        let (head, tail) = sv.split_at(arm);
        format!(
            "{head}{}",
            tail.replacen("32'h40010008:", "32'hFFFFFFF0:", 1)
        )
    });
    assert_eq!(d, [(DriftKind::Access, "PWM_REGS_PERIOD".to_string())]);
}

#[test]
fn level1_catches_a_nonzero_reset_and_a_missing_w1c_bit() {
    let d = l1_drift(|sv| sv.replacen("mmio_control <= 32'd0;", "mmio_control <= 32'd5;", 1));
    assert_eq!(d, [(DriftKind::Reset, "PWM_REGS_CONTROL".to_string())]);
    let d = l1_drift(|sv| sv.replace("(mmio_status_wrapped ? 32'h10000 : 32'd0)", "32'd0"));
    assert_eq!(
        d,
        [(
            DriftKind::FieldAccess,
            "PWM_REGS_STATUS.WRAPPED".to_string()
        )]
    );
}

/// İnceleme bulgusu: bu tasarımlarda `--check-regmap` yanlışlıkla
/// "derleyici hatası" (E9003) veriyordu.
#[test]
fn build_check_regmap_handles_camel_case_and_ambiguous_names() {
    let designs = [
        (
            "camel",
            "@mmio(base = 0x0000_0000, bus = AXI4Lite)\nmodule Cam {\n    in  clk : clock\n    out o   : u8\n\n    @reg(offset = 0x00, access = ReadWrite)\n    dataOut : { pins : u8, @reserved : bits<24> }\n\n    o = regs.dataOut.pins\n}\n",
        ),
        (
            "ambiguous",
            "@mmio(base = 0x0000_0000, bus = AXI4Lite)\nmodule Irqs {\n    in  clk : clock\n    out o   : bool\n\n    @reg(offset = 0x00, access = ReadWrite)\n    irq : { status_rx : bool, @reserved : bits<31> }\n\n    @reg(offset = 0x04, access = ReadWrite)\n    irq_status : { tx : bool, rx2 : bool, @reserved : bits<30> }\n\n    o = regs.irq.status_rx\n}\n",
        ),
    ];
    for (tag, src) in designs {
        let t = temp_dir(&format!("l1-{tag}"));
        let design = t.join(format!("{tag}.volt"));
        std::fs::write(&design, src).unwrap();
        let o = volt()
            .args([
                "--lang",
                "en",
                "build",
                "--emit=c,rust,regmap",
                "--check-regmap",
                "--target-dir",
            ])
            .arg(t.join("out"))
            .arg(&design)
            .output()
            .unwrap();
        assert_eq!(o.status.code(), Some(0), "{tag}: {}", stderr(&o));
        let files: Vec<PathBuf> = std::fs::read_dir(t.join("out/sw"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        let refs: Vec<&Path> = files.iter().map(PathBuf::as_path).collect();
        let o = check_regmap(&design, &refs, &[]);
        assert_eq!(o.status.code(), Some(0), "{tag}: {}", stderr(&o));
        let _ = std::fs::remove_dir_all(&t);
    }
}
