//! `volt build --emit=rust,c,regmap,regmap-md` (ADR-0053): yazılım tarafı
//! çıktıları, derlenebilirlikleri ve — kritik olan — RTL ile
//! tutarlılıkları. Üç tasarım kullanılır: `examples/soc/gpio.volt`
//! (yalın), `tests/ui/pass/58` (her erişim türü), `tests/ui/pass/72`
//! (doc yorumları + tüm alan nitelikleri).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

fn volt() -> Command {
    Command::new(env!("CARGO_BIN_EXE_volt"))
}

fn root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-sw-test-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// `--emit=<kinds>` ile derler, hedef dizini döndürür; başarı beklenir.
fn build(tag: &str, source: &Path, kinds: &str) -> PathBuf {
    let target = temp_dir(tag);
    let output = volt()
        .args(["build", "--target-dir"])
        .arg(&target)
        .arg(format!("--emit={kinds}"))
        .arg(source)
        .output()
        .expect("volt çalışmalı");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    target
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{} okunmalı: {e}", path.display()))
}

/// (kaynak, SV modül adı, yazılım dosya kökü)
const DESIGNS: &[(&str, &str, &str)] = &[
    ("examples/soc/gpio.volt", "Gpio", "gpio"),
    (
        "tests/ui/pass/58_mmio_access_control.volt",
        "TimerRegs",
        "timer_regs",
    ),
    (
        "tests/ui/pass/72_mmio_driver_generation.volt",
        "PwmRegs",
        "pwm_regs",
    ),
];

// ═══ Dosya yerleşimi ═══════════════════════════════════════════════

#[test]
fn emit_all_four_kinds_writes_sw_and_docs_files() {
    let target = build(
        "all-four",
        &root().join("examples/soc/gpio.volt"),
        "rust,c,regmap,regmap-md",
    );
    for rel in [
        "rtl/Gpio.sv",
        "sw/gpio.rs",
        "sw/gpio.h",
        "sw/gpio.json",
        "docs/gpio.md",
    ] {
        assert!(target.join(rel).is_file(), "{rel} üretilmeli");
    }
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn emit_single_kind_writes_only_that_file() {
    let target = build(
        "only-md",
        &root().join("examples/soc/gpio.volt"),
        "regmap-md",
    );
    assert!(target.join("docs/gpio.md").is_file());
    assert!(!target.join("sw").exists(), "sw/ dizini oluşturulmamalı");
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn emit_without_mmio_module_writes_nothing_and_notes_it() {
    let target = temp_dir("no-mmio");
    let output = volt()
        .args(["build", "--emit=rust,c", "--target-dir"])
        .arg(&target)
        .arg(root().join("tests/fixtures/counter.volt"))
        .env_remove("VOLT_LANG")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(0));
    assert!(!target.join("sw").exists());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no @mmio module in the unit; --emit=rust,c produced nothing"),
        "stderr: {stderr}"
    );
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn emit_duplicate_kind_is_written_once() {
    let target = temp_dir("dup");
    let output = volt()
        .args([
            "build",
            "--format",
            "json",
            "--emit=rust,rust,regmap",
            "--target-dir",
        ])
        .arg(&target)
        .arg(root().join("examples/soc/gpio.volt"))
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(0));
    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON zarfı");
    let artifacts: Vec<String> = envelope["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap().replace('\\', "/"))
        .collect();
    assert_eq!(artifacts.len(), 3, "{artifacts:?}");
    assert!(artifacts[0].ends_with("rtl/Gpio.sv"));
    assert!(artifacts[1].ends_with("sw/gpio.rs"));
    assert!(artifacts[2].ends_with("sw/gpio.json"));
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn json_envelope_lists_sw_artifacts_after_rtl_and_sva() {
    let target = temp_dir("envelope");
    let output = volt()
        .args([
            "build",
            "--format",
            "json",
            "--emit=sva,c,regmap-md",
            "--target-dir",
        ])
        .arg(&target)
        .arg(root().join("tests/ui/pass/58_mmio_access_control.volt"))
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(0));
    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["success"], true);
    let artifacts: Vec<String> = envelope["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap().replace('\\', "/"))
        .collect();
    assert!(artifacts[0].ends_with("rtl/TimerRegs.sv"), "{artifacts:?}");
    assert!(
        artifacts[1].ends_with("formal/timerregs.sva"),
        "{artifacts:?}"
    );
    assert!(artifacts[2].ends_with("sw/timer_regs.h"), "{artifacts:?}");
    assert!(
        artifacts[3].ends_with("docs/timer_regs.md"),
        "{artifacts:?}"
    );
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn human_output_lists_each_sw_file() {
    let target = temp_dir("human");
    let output = volt()
        .args(["build", "--emit=rust,c,regmap,regmap-md", "--target-dir"])
        .arg(&target)
        .arg(root().join("examples/soc/gpio.volt"))
        .env_remove("VOLT_LANG")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&output.stderr).replace('\\', "/");
    for rel in ["sw/gpio.rs", "sw/gpio.h", "sw/gpio.json", "docs/gpio.md"] {
        assert!(
            stderr.contains("Output ") && stderr.contains(rel),
            "stderr: {stderr}"
        );
    }
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn build_error_produces_no_sw_output() {
    let target = temp_dir("sw-error");
    let output = volt()
        .args(["build", "--emit=rust,regmap", "--target-dir"])
        .arg(&target)
        .arg(root().join("tests/ui/fail/44_mmio_write_readonly.volt"))
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(1));
    assert!(
        !target.join("sw").exists(),
        "hatalı tasarım için sürücü üretilmez"
    );
    let _ = std::fs::remove_dir_all(&target);
}

// ═══ Derlenebilirlik ═══════════════════════════════════════════════

/// Üretilen üç Rust sürücüsünü `#![no_std]` bir crate'e koyup
/// `cargo check` (uyarılar hata) ile derler.
#[test]
fn generated_rust_drivers_pass_cargo_check_in_a_no_std_crate() {
    let crate_dir = temp_dir("cargo-check");
    let src = crate_dir.join("src");
    std::fs::create_dir_all(&src).unwrap();
    let mut lib = String::from("#![no_std]\n#![deny(warnings)]\n");
    for (source, _, stem) in DESIGNS {
        let target = build(&format!("rs-{stem}"), &root().join(source), "rust");
        std::fs::copy(
            target.join(format!("sw/{stem}.rs")),
            src.join(format!("{stem}.rs")),
        )
        .unwrap();
        lib.push_str(&format!("pub mod {stem};\n"));
        let _ = std::fs::remove_dir_all(&target);
    }
    std::fs::write(src.join("lib.rs"), lib).unwrap();
    std::fs::write(
        crate_dir.join("Cargo.toml"),
        "[package]\nname = \"volt_sw_check\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[workspace]\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO"))
        .args(["check", "--offline", "--quiet", "--target-dir"])
        .arg(crate_dir.join("target"))
        .current_dir(&crate_dir)
        .env("RUSTFLAGS", "-D warnings")
        .env_remove("CARGO_TARGET_DIR")
        .output()
        .expect("cargo çalışmalı");
    assert!(
        output.status.success(),
        "cargo check başarısız:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_dir_all(&crate_dir);
}

/// Ortamdaki C derleyicisi (`CC`, gcc, cc, clang); yoksa None.
fn find_cc() -> Option<String> {
    let mut candidates: Vec<String> = Vec::new();
    if let Ok(cc) = std::env::var("CC") {
        candidates.push(cc);
    }
    candidates.extend(["gcc", "cc", "clang"].map(String::from));
    candidates.into_iter().find(|c| {
        Command::new(c)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

/// Üretilen başlıklar `gcc -fsyntax-only` (C99, -Wall -Wextra -Werror)
/// ile geçer. Derleyici yoksa (Windows geliştirici makinesi) yapısal
/// denetim yapılır; CI (ubuntu) gerçek derleyiciyle koşar.
#[test]
fn generated_c_headers_pass_c_compiler_syntax_check() {
    let work = temp_dir("c-syntax");
    let mut main_c = String::new();
    for (source, _, stem) in DESIGNS {
        let target = build(&format!("h-{stem}"), &root().join(source), "c");
        let header = read(&target.join(format!("sw/{stem}.h")));
        // Yapısal: guard, dengeli parantez/küme, her satır sonu ';' ya da
        // '{' / '}' / makro / yorum.
        let guard = format!("{}_H", stem.to_ascii_uppercase());
        assert!(header.contains(&format!("#ifndef {guard}")));
        assert_eq!(
            header.matches('{').count(),
            header.matches('}').count(),
            "{stem}.h küme dengesi"
        );
        assert_eq!(
            header.matches('(').count(),
            header.matches(')').count(),
            "{stem}.h parantez dengesi"
        );
        std::fs::copy(
            target.join(format!("sw/{stem}.h")),
            work.join(format!("{stem}.h")),
        )
        .unwrap();
        main_c.push_str(&format!("#include \"{stem}.h\"\n"));
        let _ = std::fs::remove_dir_all(&target);
    }
    main_c.push_str(
        "int main(void) {\n    gpio_set_data_out_pins(0x5AU);\n    \
         timer_regs_trigger_ctrl_clear();\n    pwm_regs_clear_status_wrapped();\n    \
         return (int)gpio_get_data_in_pins() + (int)timer_regs_get_status_count()\n        \
         + (int)pwm_regs_get_id_value();\n}\n",
    );
    std::fs::write(work.join("main.c"), main_c).unwrap();
    match find_cc() {
        Some(cc) => {
            let output = Command::new(&cc)
                .args([
                    "-std=c99",
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-fsyntax-only",
                    "main.c",
                ])
                .current_dir(&work)
                .output()
                .expect("C derleyicisi çalışmalı");
            assert!(
                output.status.success(),
                "{cc} -fsyntax-only başarısız:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        None => eprintln!("note: no C compiler on PATH; structural checks only"),
    }
    let _ = std::fs::remove_dir_all(&work);
}

// ═══ JSON şeması ═══════════════════════════════════════════════════

fn expect_type(v: &serde_json::Value, key: &str, is: fn(&serde_json::Value) -> bool, ctx: &str) {
    assert!(
        is(&v[key]),
        "{ctx}: '{key}' beklenen tipte değil: {}",
        v[key]
    );
}

/// `volt-regmap/1` şemasını (ADR-0053 §JSON) el yordamıyla doğrular.
fn assert_schema(v: &serde_json::Value) {
    assert_eq!(v["schema"], "volt-regmap/1");
    let top = "üst nesne";
    expect_type(v, "generator", |x| x.is_string(), top);
    expect_type(v, "source", |x| x.is_string(), top);
    expect_type(v, "name", |x| x.is_string(), top);
    expect_type(v, "base", |x| x.is_u64(), top);
    expect_type(v, "bus", |x| x.is_string(), top);
    expect_type(v, "doc", |x| x.is_string() || x.is_null(), top);
    let regs = v["registers"].as_array().expect("registers dizi");
    for r in regs {
        let ctx = format!("register {}", r["name"]);
        expect_type(r, "name", |x| x.is_string(), &ctx);
        expect_type(r, "offset", |x| x.is_u64(), &ctx);
        expect_type(r, "address", |x| x.is_u64(), &ctx);
        assert_eq!(
            r["address"].as_u64().unwrap(),
            v["base"].as_u64().unwrap() + r["offset"].as_u64().unwrap()
        );
        assert!(
            ["rw", "ro", "wo"].contains(&r["access"].as_str().unwrap()),
            "{ctx}: access"
        );
        expect_type(r, "volatile", |x| x.is_boolean(), &ctx);
        expect_type(r, "doc", |x| x.is_string() || x.is_null(), &ctx);
        let fields = r["fields"].as_array().expect("fields dizi");
        let mut next_lsb = 0;
        for f in fields {
            let fctx = format!("{ctx}.{}", f["name"]);
            expect_type(f, "name", |x| x.is_string(), &fctx);
            expect_type(f, "lsb", |x| x.is_u64(), &fctx);
            expect_type(f, "width", |x| x.is_u64(), &fctx);
            assert!(["bool", "bits", "uint"].contains(&f["type"].as_str().unwrap()));
            for b in ["reserved", "self_clearing", "w1c"] {
                expect_type(f, b, |x| x.is_boolean(), &fctx);
            }
            expect_type(f, "doc", |x| x.is_string() || x.is_null(), &fctx);
            assert_eq!(
                f["lsb"].as_u64().unwrap(),
                next_lsb,
                "{fctx}: alanlar ardışık"
            );
            next_lsb += f["width"].as_u64().unwrap();
            if f["reserved"] == true {
                assert_eq!(f["name"], "_reserved");
            }
        }
        assert!(next_lsb <= 32, "{ctx}: 32 biti aşmaz");
    }
}

#[test]
fn generated_json_is_valid_and_matches_schema() {
    for (source, module, stem) in DESIGNS {
        let target = build(&format!("json-{stem}"), &root().join(source), "regmap");
        let text = read(&target.join(format!("sw/{stem}.json")));
        let v: serde_json::Value = serde_json::from_str(&text).expect("geçerli JSON");
        assert_schema(&v);
        assert_eq!(v["name"], *module);
        let _ = std::fs::remove_dir_all(&target);
    }
}

#[test]
fn json_for_gpio_example_has_expected_registers() {
    let target = build(
        "json-gpio-regs",
        &root().join("examples/soc/gpio.volt"),
        "regmap",
    );
    let v: serde_json::Value = serde_json::from_str(&read(&target.join("sw/gpio.json"))).unwrap();
    assert_eq!(v["base"], 0);
    assert_eq!(v["bus"], "AXI4Lite");
    let regs: Vec<(String, u64, String)> = v["registers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["name"].as_str().unwrap().to_string(),
                r["offset"].as_u64().unwrap(),
                r["access"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(
        regs,
        [
            ("data_out".to_string(), 0, "rw".to_string()),
            ("dir".to_string(), 4, "rw".to_string()),
            ("data_in".to_string(), 8, "ro".to_string()),
        ]
    );
    assert_eq!(v["registers"][2]["volatile"], true);
    assert_eq!(v["registers"][0]["fields"][0]["name"], "pins");
    assert_eq!(v["registers"][0]["fields"][0]["width"], 8);
    assert_eq!(v["registers"][0]["fields"][1]["name"], "_reserved");
    assert_eq!(v["registers"][0]["fields"][1]["lsb"], 8);
    assert_eq!(v["registers"][0]["fields"][1]["width"], 24);
    let _ = std::fs::remove_dir_all(&target);
}

// ═══ Erişim hakları ════════════════════════════════════════════════

#[test]
fn readonly_registers_get_no_setter_in_rust_or_c() {
    let target = build(
        "ro-no-setter",
        &root().join("tests/ui/pass/58_mmio_access_control.volt"),
        "rust,c",
    );
    let rs = read(&target.join("sw/timer_regs.rs"));
    let h = read(&target.join("sw/timer_regs.h"));
    // `id` sabit ReadOnly, `status` volatile ReadOnly.
    for forbidden in [
        "fn set_id",
        "fn set_status",
        "fn set_status_raw",
        "fn set_id_raw",
    ] {
        assert!(
            !rs.contains(forbidden),
            "Rust: '{forbidden}' üretilmemeli\n{rs}"
        );
    }
    for forbidden in [
        "timer_regs_set_id_value",
        "timer_regs_id_write",
        "timer_regs_set_status_count",
        "timer_regs_set_status_expired",
        "timer_regs_status_write",
    ] {
        assert!(!h.contains(forbidden), "C: '{forbidden}' üretilmemeli\n{h}");
    }
    // Okuma tarafı ve @w1c temizleyicisi var.
    assert!(rs.contains("pub fn id(&self) -> u16"));
    assert!(rs.contains("pub fn status_count(&self) -> u16"));
    assert!(rs.contains("pub fn clear_status_expired(&mut self)"));
    assert!(h.contains("timer_regs_get_id_value"));
    assert!(h.contains("timer_regs_clear_status_expired"));
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn writeonly_registers_get_no_getter_in_rust_or_c() {
    let target = build(
        "wo-no-getter",
        &root().join("tests/ui/pass/58_mmio_access_control.volt"),
        "rust,c",
    );
    let rs = read(&target.join("sw/timer_regs.rs"));
    let h = read(&target.join("sw/timer_regs.h"));
    assert!(!rs.contains("fn compare(&self)"));
    assert!(!rs.contains("fn compare_raw"));
    assert!(rs.contains("pub fn set_compare(&mut self, value: u16)"));
    assert!(!h.contains("timer_regs_get_compare_value"));
    assert!(!h.contains("timer_regs_compare_read"));
    assert!(h.contains("timer_regs_set_compare_value"));
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn doc_comments_reach_all_four_outputs() {
    let target = build(
        "docs",
        &root().join("tests/ui/pass/72_mmio_driver_generation.volt"),
        "rust,c,regmap,regmap-md",
    );
    let rs = read(&target.join("sw/pwm_regs.rs"));
    assert!(rs.contains("/// PWM channel controller.\n/// One 16-bit counter compared against a duty threshold.\n#[derive(Debug)]"));
    assert!(rs.contains(
        "    /// Write 1 to restart the counter from 0.\n    /// Pulse `control.restart` (bit 1)"
    ));
    assert!(rs.contains(
        "    /// Clock prescaler, 0 = every tick.\n    /// `control.prescale` (bits 5:2)."
    ));
    let h = read(&target.join("sw/pwm_regs.h"));
    assert!(h.contains(" * PWM channel controller.\n"));
    assert!(h.contains("/* Channel control. */"));
    assert!(h.contains("/* Set when the counter wraps; write 1 to clear. */"));
    let v: serde_json::Value =
        serde_json::from_str(&read(&target.join("sw/pwm_regs.json"))).unwrap();
    assert_eq!(
        v["doc"],
        "PWM channel controller.\nOne 16-bit counter compared against a duty threshold."
    );
    assert_eq!(v["registers"][0]["doc"], "Channel control.");
    assert_eq!(
        v["registers"][0]["fields"][2]["doc"],
        "Clock prescaler, 0 = every tick."
    );
    let md = read(&target.join("docs/pwm_regs.md"));
    assert!(
        md.contains("| `0x04` | `duty` | RW | Duty threshold: pwm is high while count < duty. |")
    );
    assert!(md.contains(
        "| 16 | `wrapped` | `bool` | w1c | Set when the counter wraps; write 1 to clear. |"
    ));
    assert!(md.contains("| 31:0 | `value` | `bits<32>` | — |  |"));
    let _ = std::fs::remove_dir_all(&target);
}

// ═══ RTL ↔ sürücü tutarlılığı (KRİTİK) ═════════════════════════════

/// Satırdaki tüm `32'h<hex>` literalleri.
fn hex_literals(line: &str) -> BTreeSet<u64> {
    let mut out = BTreeSet::new();
    let mut rest = line;
    while let Some(pos) = rest.find("32'h") {
        let digits: String = rest[pos + 4..]
            .chars()
            .take_while(|c| c.is_ascii_hexdigit())
            .collect();
        out.insert(u64::from_str_radix(&digits, 16).expect("hex"));
        rest = &rest[pos + 4 + digits.len()..];
    }
    out
}

/// `wire mmio_whit = ...` gibi tek satırdaki adres kümesi.
fn line_addresses(sv: &str, marker: &str) -> BTreeSet<u64> {
    let line = sv
        .lines()
        .find(|l| l.contains(marker))
        .unwrap_or_else(|| panic!("SV'de '{marker}' satırı yok"));
    hex_literals(line)
}

/// `case (<sel>)` ... `endcase` arasındaki `32'h...:` kollarının adresleri.
fn case_arms(sv: &str, sel: &str) -> BTreeSet<u64> {
    let start = sv
        .find(&format!("case ({sel})"))
        .unwrap_or_else(|| panic!("SV'de case ({sel}) yok"));
    let body = &sv[start..];
    let end = body.find("endcase").expect("endcase");
    body[..end]
        .lines()
        .filter(|l| l.trim_start().starts_with("32'h") && l.contains(':'))
        .flat_map(|l| hex_literals(l.split(':').next().unwrap()))
        .collect()
}

/// regmap.json'daki her adresi RTL'in adres çözümlemesiyle karşılaştırır:
/// harita kümesi = `mmio_whit`/`mmio_rhit` kümesi; okunabilir register'lar
/// = `case (ar_addr)` kolları; bus'ın yazdığı register'lar = `case
/// (aw_addr)` kolları; bus'a ait RW register'ın okuma maskesi = JSON alan
/// maskesi; `@w1c` bitleri okuma görünümünde aynı bitte. Sürücü ile RTL
/// aynı `RegInfo`'dan türese de bu test ayrışmayı bağımsız olarak yakalar.
fn assert_regmap_matches_rtl(json: &serde_json::Value, sv: &str) {
    let base = json["base"].as_u64().unwrap();
    let regs = json["registers"].as_array().unwrap();
    let all: BTreeSet<u64> = regs
        .iter()
        .map(|r| r["address"].as_u64().unwrap())
        .collect();
    assert_eq!(all.len(), regs.len(), "adresler benzersiz");
    assert!(all.iter().all(|a| a >= &base));
    assert_eq!(
        line_addresses(sv, "wire mmio_whit ="),
        all,
        "yazma adres kümesi"
    );
    assert_eq!(
        line_addresses(sv, "wire mmio_rhit ="),
        all,
        "okuma adres kümesi"
    );

    let readable: BTreeSet<u64> = regs
        .iter()
        .filter(|r| r["access"] != "wo")
        .map(|r| r["address"].as_u64().unwrap())
        .collect();
    assert_eq!(
        case_arms(sv, "ar_addr"),
        readable,
        "okuma çoklayıcısı kolları"
    );

    let bus_written: BTreeSet<u64> = regs
        .iter()
        .filter(|r| r["access"] != "ro")
        .map(|r| r["address"].as_u64().unwrap())
        .collect();
    assert_eq!(
        case_arms(sv, "aw_addr"),
        bus_written,
        "yazma mantığı kolları"
    );

    for r in regs {
        let name = r["name"].as_str().unwrap();
        let fields = r["fields"].as_array().unwrap();
        let mask: u64 = fields
            .iter()
            .filter(|f| f["reserved"] == false)
            .map(|f| {
                let w = f["width"].as_u64().unwrap();
                let bits = if w >= 32 {
                    u32::MAX as u64
                } else {
                    (1u64 << w) - 1
                };
                bits << f["lsb"].as_u64().unwrap()
            })
            .fold(0, |m, b| m | b);
        let rd_line = sv
            .lines()
            .find(|l| l.contains(&format!("wire [31:0] mmio_{name}_rd = ")));
        if r["access"] == "rw" && r["volatile"] == false {
            let line = rd_line.unwrap_or_else(|| panic!("mmio_{name}_rd satırı yok"));
            if mask == u32::MAX as u64 {
                assert!(
                    !line.contains("32'h"),
                    "{name}: tam sözcük register maskesiz okunur: {line}"
                );
            } else {
                assert_eq!(
                    hex_literals(line),
                    BTreeSet::from([mask]),
                    "{name} okuma maskesi"
                );
            }
        }
        for f in fields.iter().filter(|f| f["w1c"] == true) {
            let bit = 1u64 << f["lsb"].as_u64().unwrap();
            let line = rd_line.unwrap_or_else(|| panic!("mmio_{name}_rd satırı yok"));
            assert!(
                hex_literals(line).contains(&bit),
                "{name}.{}: @w1c biti okuma görünümünde {bit:#x} olmalı: {line}",
                f["name"]
            );
        }
    }
}

#[test]
fn regmap_offsets_match_rtl_address_decode_for_every_design() {
    for (source, module, stem) in DESIGNS {
        let target = build(&format!("rtl-{stem}"), &root().join(source), "regmap");
        let sv = read(&target.join(format!("rtl/{module}.sv")));
        let json: serde_json::Value =
            serde_json::from_str(&read(&target.join(format!("sw/{stem}.json")))).unwrap();
        assert_regmap_matches_rtl(&json, &sv);
        let _ = std::fs::remove_dir_all(&target);
    }
}

#[test]
fn rtl_consistency_check_detects_a_shifted_offset() {
    // Denetimin kendisi sınanır: JSON'da bir offset kaydırılırsa test
    // fonksiyonu panik etmeli (aksi halde denetim boş bir tören olurdu).
    let target = build(
        "rtl-mutant",
        &root().join("examples/soc/gpio.volt"),
        "regmap",
    );
    let sv = read(&target.join("rtl/Gpio.sv"));
    let mut json: serde_json::Value =
        serde_json::from_str(&read(&target.join("sw/gpio.json"))).unwrap();
    json["registers"][1]["address"] = serde_json::json!(0x40);
    let caught = std::panic::catch_unwind(|| assert_regmap_matches_rtl(&json, &sv));
    assert!(caught.is_err(), "kaydırılmış offset yakalanmalı");
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn rust_constants_match_regmap_json() {
    for (source, _, stem) in DESIGNS {
        let target = build(
            &format!("consts-{stem}"),
            &root().join(source),
            "rust,c,regmap",
        );
        let v: serde_json::Value =
            serde_json::from_str(&read(&target.join(format!("sw/{stem}.json")))).unwrap();
        let rs = read(&target.join(format!("sw/{stem}.rs")));
        let h = read(&target.join(format!("sw/{stem}.h")));
        let up = stem.to_ascii_uppercase();
        let base = v["base"].as_u64().unwrap();
        assert!(rs.contains(&format!(
            "pub const BASE: usize = 0x{:04X}_{:04X};",
            base >> 16,
            base & 0xFFFF
        )));
        assert!(h.contains(&format!("#define {up}_BASE 0x{base:08X}U")));
        for r in v["registers"].as_array().unwrap() {
            let name = r["name"].as_str().unwrap().to_ascii_uppercase();
            let off = r["offset"].as_u64().unwrap();
            assert!(
                rs.contains(&format!("pub const {name}_OFFSET: usize = 0x{off:02X};")),
                "{stem}.rs: {name}_OFFSET"
            );
            assert!(
                h.contains(&format!("#define {up}_{name}_OFFSET 0x{off:02X}U")),
                "{stem}.h: {name}_OFFSET"
            );
            for f in r["fields"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|f| f["reserved"] == false)
            {
                let fname = f["name"].as_str().unwrap().to_ascii_uppercase();
                let lsb = f["lsb"].as_u64().unwrap();
                assert!(
                    h.contains(&format!("#define {up}_{name}_{fname}_SHIFT {lsb}U")),
                    "{stem}.h: {name}_{fname}_SHIFT"
                );
            }
        }
        let _ = std::fs::remove_dir_all(&target);
    }
}
