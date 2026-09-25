//! Hedef dillerin ayrılmış sözcükleri (ADR-0078) uçtan uca: her ad yolu
//! için ui/fail tanısının kodu ve satırı (`volt check`), bileşik adların
//! iletisi, reddedilmeyen konumların temiz SV'si, Verilator C++
//! sözcüklü portların susturma sarması ve `__SYM__` testbench adı,
//! `@mmio` sürücülerinin Rust/C derlemesi. Gerçek Verilator/rustc/cc
//! gerektiren testler araç yoksa atlanır (CI `integration` işi koşar).

mod tools;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn volt(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en"])
        .args(args)
        .output()
        .expect("volt çalışmalı")
}

fn check_json(file: &Path) -> Value {
    let out = volt(&["check", "--format", "json", file.to_str().unwrap()]);
    serde_json::from_slice(&out.stdout).expect("json")
}

/// `//~ KOD` + `//~^ ERROR <parça>`: kod, anotasyonun üstündeki satırda
/// ve iletisi parçayı içererek raporlanmalı.
fn assert_ui_fail(name: &str) {
    let file = root().join("tests/ui/fail").join(name);
    let src = std::fs::read_to_string(&file).expect("okunmalı");
    let code = src
        .lines()
        .next()
        .and_then(|l| l.strip_prefix("//~ "))
        .expect("ilk satır //~ KOD")
        .trim()
        .to_string();
    let (line, part) = src
        .lines()
        .enumerate()
        .find_map(|(i, l)| {
            l.trim_start()
                .strip_prefix("//~^ ERROR ")
                .map(|p| (i as u64, p.trim().to_string()))
        })
        .expect("//~^ ERROR satırı");
    let env = check_json(&file);
    let found: Vec<(u64, String)> = env["diagnostics"]
        .as_array()
        .expect("tanılar")
        .iter()
        .filter(|d| d["code"] == code.as_str())
        .filter_map(|d| {
            Some((
                d["spans"][0]["start"]["line"].as_u64()?,
                d["message"].as_str()?.to_string(),
            ))
        })
        .collect();
    assert!(
        found.iter().any(|(l, m)| *l == line && m.contains(&part)),
        "{name}: {code} satır {line} '{part}' bekleniyor, bulunan {found:?}"
    );
}

#[test]
fn ui_fail_122_sv_keyword_port() {
    assert_ui_fail("122_sv_keyword_port.volt");
}

#[test]
fn ui_fail_123_sv_keyword_reg() {
    assert_ui_fail("123_sv_keyword_reg.volt");
}

#[test]
fn ui_fail_124_sv_keyword_wire() {
    assert_ui_fail("124_sv_keyword_wire.volt");
}

#[test]
fn ui_fail_125_sv_keyword_let() {
    assert_ui_fail("125_sv_keyword_let.volt");
}

#[test]
fn ui_fail_126_sv_keyword_const() {
    assert_ui_fail("126_sv_keyword_const.volt");
}

#[test]
fn ui_fail_127_sv_keyword_module_name() {
    assert_ui_fail("127_sv_keyword_module_name.volt");
}

#[test]
fn ui_fail_128_sv_keyword_instance() {
    assert_ui_fail("128_sv_keyword_instance.volt");
}

#[test]
fn ui_fail_129_sv_keyword_extern() {
    assert_ui_fail("129_sv_keyword_extern.volt");
}

#[test]
fn ui_fail_130_sv_keyword_struct_leaf() {
    assert_ui_fail("130_sv_keyword_struct_leaf.volt");
}

#[test]
fn ui_fail_131_sv_keyword_bundle_field() {
    assert_ui_fail("131_sv_keyword_bundle_field.volt");
}

#[test]
fn ui_fail_132_sv_keyword_enum_localparam() {
    assert_ui_fail("132_sv_keyword_enum_localparam.volt");
}

#[test]
fn ui_fail_133_sv_keyword_instance_output() {
    assert_ui_fail("133_sv_keyword_instance_output.volt");
}

#[test]
fn ui_fail_134_mmio_rust_keyword() {
    assert_ui_fail("134_mmio_rust_keyword.volt");
}

#[test]
fn ui_fail_135_mmio_c_keyword() {
    assert_ui_fail("135_mmio_c_keyword.volt");
}

#[test]
fn ui_fail_136_mmio_register_keyword() {
    assert_ui_fail("136_mmio_register_keyword.volt");
}

#[test]
fn ui_fail_137_verilator_model_member() {
    assert_ui_fail("137_verilator_model_member.volt");
}

/// Alt modülün portu üst modülün örnek bağlantısında (`.table(a)`) önce
/// görünür; üst modül kaynakta önce gelse de tek tanı, alt modülün
/// portunda (güvenlik ağı kesin denetimden önce koşmaz).
#[test]
fn keyword_port_is_reported_once_whatever_the_module_order() {
    let top = "module Top {\n    in  a : u8\n    out y : u8\n    let u = Sub { table: a }\n    y = u.y\n}\n";
    let sub = "module Sub {\n    in  table : u8\n    out y     : u8\n    y = table\n}\n";
    for (tag, src, line) in [
        ("top-first", format!("{top}\n{sub}"), 9),
        ("sub-first", format!("{sub}\n{top}"), 2),
    ] {
        let dir = temp_dir(tag);
        let file = dir.join("order.volt");
        std::fs::write(&file, src).unwrap();
        let env = check_json(&file);
        let found: Vec<(String, u64)> = env["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|d| {
                Some((
                    d["code"].as_str()?.to_string(),
                    d["spans"][0]["start"]["line"].as_u64()?,
                ))
            })
            .collect();
        assert_eq!(found, [("E1013".to_string(), line)], "{tag}");
    }
}

// ═══ Derleme ve üretilen çıktı ══════════════════════════════════════

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-kw-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("geçici dizin");
    dir
}

/// `volt build`; başarı zorunlu, hedef dizin döner.
fn build(tag: &str, file: &Path, extra: &[&str]) -> PathBuf {
    let target = temp_dir(tag).join("out");
    let mut args = vec!["build", "--target-dir", target.to_str().unwrap()];
    args.extend_from_slice(extra);
    args.push(file.to_str().unwrap());
    let out = volt(&args);
    assert!(
        out.status.success(),
        "build başarısız:\n{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    target
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

#[test]
fn keyword_codes_in_check_equal_build() {
    // Her fail fixture'ında build de aynı E1013/E8513 dışı kodlarla düşer
    // (parite, ADR-0070); E8513 yalnız test/run'a özgü değil — test bloğu
    // `check`'te de denetlenir.
    for name in ["123_sv_keyword_reg.volt", "134_mmio_rust_keyword.volt"] {
        let file = root().join("tests/ui/fail").join(name);
        let target = temp_dir("parity");
        let out = volt(&[
            "build",
            "--format",
            "json",
            "--target-dir",
            target.to_str().unwrap(),
            file.to_str().unwrap(),
        ]);
        let env: Value = serde_json::from_slice(&out.stdout).expect("json");
        let codes: Vec<&str> = env["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|d| d["code"].as_str())
            .collect();
        assert!(codes.contains(&"E1013"), "{name}: {codes:?}");
        assert!(!out.status.success());
    }
}

#[test]
fn cpp_word_ports_are_wrapped_in_symrsvdword_lint_off() {
    let target = build(
        "cppword",
        &root().join("tests/ui/pass/109_verilator_cpp_word_ports.volt"),
        &[],
    );
    let sv = read(&target.join("rtl/IrqLatch.sv"));
    for name in ["char", "interrupt"] {
        let decl = sv
            .lines()
            .position(|l| {
                l.trim_end()
                    .trim_end_matches(',')
                    .ends_with(&format!(" {name}"))
            })
            .unwrap_or_else(|| panic!("{name} bildirimi yok:\n{sv}"));
        let lines: Vec<&str> = sv.lines().collect();
        assert_eq!(
            lines[decl - 1].trim(),
            "// verilator lint_off SYMRSVDWORD",
            "{sv}"
        );
        assert_eq!(
            lines[decl + 1].trim(),
            "// verilator lint_on SYMRSVDWORD",
            "{sv}"
        );
        assert!(lines[decl - 2].contains(&format!("__SYM__{name}")), "{sv}");
    }
    // Sıradan portlar sarılmaz.
    assert_eq!(sv.matches("lint_off SYMRSVDWORD").count(), 2, "{sv}");
}

#[test]
fn safe_keyword_positions_build_clean() {
    let target = build(
        "safe",
        &root().join("tests/ui/pass/108_sv_keyword_safe_names.volt"),
        &[],
    );
    let sv = read(&target.join("rtl/Safe.sv"));
    for name in [
        "Packed",
        "p_packed",
        "p_table",
        "b_table",
        "release_force",
        "edge_0",
        "buf_rd_data",
    ] {
        assert!(sv.contains(name), "{name} yok:\n{sv}");
    }
}

// ═══ Gerçek araçlar ═════════════════════════════════════════════════

/// ADR-0079 §3: `VOLT_REQUIRE_TOOLS=verilator` ise yokluk hata.
fn verilator() -> Option<PathBuf> {
    tools::require(tools::Tool::Verilator)
}

/// Reddedilmeyen konumlardaki anahtar sözcükler ve susturulmuş C++
/// sözcükleri Verilator `-Wall` ile SIFIR satır verir.
#[test]
fn accepted_designs_lint_clean_in_verilator() {
    let Some(verilator) = verilator() else {
        eprintln!("note: Verilator not found; lint skipped");
        return;
    };
    for (fixture, top) in [
        ("108_sv_keyword_safe_names.volt", "Safe"),
        ("109_verilator_cpp_word_ports.volt", "IrqLatch"),
    ] {
        let target = build(top, &root().join("tests/ui/pass").join(fixture), &[]);
        let rtl = target.join("rtl");
        let out = Command::new(&verilator)
            .args(["--lint-only", "-Wall", "--top-module", top])
            .arg(format!("{top}.sv"))
            .current_dir(&rtl)
            .output()
            .expect("verilator çalışmalı");
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        // 5.05x `--lint-only`'de de "Verilation Report" özeti basar;
        // ölçüt uyarı/hata satırı olmaması.
        let findings = text.lines().filter(|l| l.starts_with('%')).count();
        assert!(out.status.success() && findings == 0, "{fixture}:\n{text}");
    }
}

const CPP_WORD_TEST: &str = "module IrqLatch {
    in  clk       : clock
    in  char      : u8
    out interrupt : bool
    out stack     : u8

    reg seen : bool = false
    reg last : u8 = 0
    on clk {
        if char != 0 { seen <= true }
        last <= char
    }
    interrupt = seen
    stack = last
}

test \"cpp words reach the renamed model members\" {
    let dut = IrqLatch { };
    assert_false(dut.interrupt);
    dut.char = 7;
    step(1);
    assert_true(dut.interrupt);
    assert_eq(dut.stack, 7);
    for i in 1..4 {
        dut.char = i;
        step(1);
        assert_eq(dut.stack, i);
    }
}
";

/// `volt test` Verilator C++ sözcüğü olan üst portlarla uçtan uca koşar:
/// SV'deki susturma + testbench'in `__SYM__` adları.
#[test]
fn volt_test_runs_with_cpp_word_ports() {
    let Some(verilator) = verilator() else {
        eprintln!("note: Verilator not found; simulation skipped");
        return;
    };
    let dir = temp_dir("cppsim");
    let file = dir.join("irq_test.volt");
    std::fs::write(&file, CPP_WORD_TEST).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "test", "--target-dir"])
        .arg(dir.join("out"))
        .arg(&file)
        .env("VOLT_VERILATOR", &verilator)
        .output()
        .expect("volt çalışmalı");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "{text}");
    assert!(text.contains("test result: ok. 1 passed"), "{text}");
}

/// `volt run` üst modülün model üyesiyle çakışan portunu E8513 ile
/// Verilator'a gitmeden reddeder.
#[test]
fn volt_run_refuses_model_member_ports() {
    let dir = temp_dir("run8513");
    let file = dir.join("probe.volt");
    std::fs::write(
        &file,
        "module Probe {\n    in  clk  : clock\n    out eval : u8\n    eval = 1\n}\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "run", "--target-dir"])
        .arg(dir.join("out"))
        .arg(&file)
        // Araç denetiminden önce durmalı: sahte yol verilir.
        .env("VOLT_VERILATOR", dir.join("no-verilator"))
        .output()
        .expect("volt çalışmalı");
    let text = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(text.contains("error[E8513]"), "{text}");
    assert!(text.contains("VProbe::eval"), "{text}");
}

// ═══ @mmio sürücüleri: Rust ve C derlemesi ══════════════════════════

const MMIO_NEAR_KEYWORDS: &str = "@mmio(base = 0x4000_0000, bus = AXI4Lite)
module NearRegs {
    in  clk : clock
    out y   : bool

    @reg(offset = 0x00, access = ReadWrite)
    ctrl : { mod_ : bool, safe : bool, default_ : u4, interrupt : bool, @reserved : bits<25> }
    @reg(offset = 0x04, access = ReadWrite)
    union_ : { value : u8, @reserved : bits<24> }
    y = regs.ctrl.mod_ && regs.ctrl.safe && regs.ctrl.interrupt && regs.union_.value == 0 && regs.ctrl.default_ == 0
}
";

/// Anahtar sözcüğe komşu ama geçerli adlar (`safe` Rust'ta bağlamsal,
/// `interrupt` C/C++'ta sıradan ad) üretilen sürücüde derlenir.
#[test]
fn near_keyword_mmio_names_compile_in_rust_and_c() {
    let dir = temp_dir("mmio-near");
    let file = dir.join("near_regs.volt");
    std::fs::write(&file, MMIO_NEAR_KEYWORDS).unwrap();
    let target = build("mmio-near-out", &file, &["--emit=rust,c"]);
    let rs = target.join("sw/near_regs.rs");
    let h = target.join("sw/near_regs.h");
    assert!(read(&rs).contains("pub fn set_ctrl_safe(&mut self, safe: bool)"));
    if let Some(rustc) = tools::require(tools::Tool::Rustc) {
        let out = Command::new(rustc)
            .args(["--edition", "2021", "--crate-type", "lib", "-o"])
            .arg(dir.join("near_regs.rlib"))
            .arg(&rs)
            .output()
            .expect("rustc");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    for (tool, std) in [
        (tools::Tool::Cc, "-std=c11"),
        (tools::Tool::Cxx, "-std=c++17"),
    ] {
        let Some(cc) = tools::require(tool) else {
            continue;
        };
        let out = Command::new(cc)
            .args([std, "-Wall", "-Wextra", "-Werror", "-fsyntax-only", "-x"])
            .arg(if std.contains("++") { "c++" } else { "c" })
            .arg(&h)
            .output()
            .expect("cc");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
