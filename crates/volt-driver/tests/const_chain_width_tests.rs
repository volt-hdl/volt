//! Uzun bir sabit zinciriyle verilen port genişliği SV'de ve test dilinde
//! aynıdır (#376, ADR-0132) — uçtan uca.
//!
//! `const K0 : u32 = K1`, ..., `const K{n-1} : u32 = 4` ve `in a : uint<K0>`.
//! Önceden SV üreticisi genişliği 64 düzeye kadar çözüyor (`[3:0]`), test
//! dili 32'de duruyordu: 33 ile 64 sabit arasında test dili genişliği
//! bilmiyor, 31 4 bitlik porta ulaşıyor ve 4 bitlik `q` 31 okuyordu. Şimdi
//! ikisi aynı fonksiyonu (`volt_ast::const_eval`) aynı sınırla çağırır;
//! sınırı aşan zincir her komutta aynı tanıyla (E2005) durur.
//!
//! Benzetim isteyen test Verilator yoksa atlanır; `VOLT_REQUIRE_TOOLS=
//! verilator` ile (CI'ın araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// n sabitlik zincir ve genişliği `K0` olan bir kayıt.
fn design(n: usize) -> String {
    let mut src = String::new();
    for i in 0..n - 1 {
        src.push_str(&format!("const K{i} : u32 = K{}\n", i + 1));
    }
    src.push_str(&format!("const K{} : u32 = 4\n", n - 1));
    src.push_str(
        "
module M {
    in  clk : clock
    in  a   : uint<K0>
    out q   : uint<K0>
    reg r : uint<K0> = 0
    on clk { r <= a }
    q = r
}
",
    );
    src
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-const-chain-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// `volt <cmd>` (`--target-dir dir/build`); çıktı stdout + stderr.
fn volt(cmd: &str, dir: &Path, file: &Path, verilator: Option<&Path>) -> (Output, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_volt"));
    c.arg(cmd).args(["--color", "never"]);
    if cmd != "check" {
        c.arg("--target-dir").arg(dir.join("build"));
    }
    if cmd == "test" {
        c.arg("--no-waves");
    }
    c.arg(file)
        .current_dir(dir)
        .env("VOLT_TOOL_BACKEND", "local")
        .env("VOLT_LANG", "en");
    if let Some(v) = verilator {
        c.env("VOLT_VERILATOR", v);
    }
    let out = c.output().expect("volt çalışmalı");
    let text = format!(
        "{}\n--- stderr ---\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out, text)
}

fn write(dir: &Path, n: usize, tests: &str) -> PathBuf {
    let file = dir.join("m_test.volt");
    std::fs::write(&file, format!("{}{tests}", design(n))).expect("tasarım");
    file
}

#[test]
fn a_chain_up_to_the_limit_gives_four_bits_in_the_sv_and_in_the_test_language() {
    // Sabit 31: 4 bitlik porta sığmaz -> E8512 (yazım ve hiç eşleşemeyen
    // karşılaştırma). Önceden 33 ve 64'te ikisi de kabul ediliyordu.
    let tests = "
test \"const write\" {
    let dut = M { };
    dut.a = 31;
    step(1);
    assert_eq(dut.q, 31);
}
";
    for n in [33, 64] {
        let dir = temp_dir(&format!("bits-{n}"));
        let file = write(&dir, n, "");
        let (out, all) = volt("build", &dir, &file, None);
        assert_eq!(out.status.code(), Some(0), "n={n}: {all}");
        let sv = std::fs::read_to_string(dir.join("build").join("rtl").join("M.sv"))
            .unwrap_or_else(|e| panic!("n={n}: M.sv: {e}\n{all}"));
        assert!(sv.contains("input  logic [3:0] a,"), "n={n}: {sv}");
        assert!(sv.contains("output logic [3:0] q"), "n={n}: {sv}");

        let file = write(&dir, n, tests);
        let (out, all) = volt("check", &dir, &file, None);
        assert_eq!(out.status.code(), Some(1), "n={n}: {all}");
        assert_eq!(all.matches("error[E8512]").count(), 2, "n={n}: {all}");
        assert!(all.contains("port 'a' is u4 (max 15)"), "n={n}: {all}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn a_computed_value_past_four_bits_fails_the_test_on_a_long_chain() {
    // Değerler elle: r <= a, q = r; bir çevrim sonra q yazılan değerdir.
    //  - "fits": 9 dört bite sığar -> geçer.
    //  - "too wide": 31 sığmaz -> koşuda port_overflow ile düşer. Önceden
    //    33 ve 64 sabitlik zincirde 31 porta ulaşıyor, q 31 okuyor ve
    //    test geçiyordu.
    let tests = "
test \"fits\" {
    let dut = M { };
    for i in 9..10 {
        dut.a = i;
        step(1);
        assert_eq(dut.q, i);
    }
}

test \"too wide\" {
    let dut = M { };
    for i in 31..32 {
        dut.a = i;
        step(1);
        assert_eq(dut.q, i);
    }
}
";
    let Some(verilator) = tools::require(tools::Tool::Verilator) else {
        return;
    };
    for n in [33, 64] {
        let dir = temp_dir(&format!("run-{n}"));
        let file = write(&dir, n, tests);
        let (out, all) = volt("test", &dir, &file, Some(&verilator));
        assert_eq!(out.status.code(), Some(5), "n={n}: {all}");
        assert!(all.contains("test fits ... ok"), "n={n}: {all}");
        assert!(all.contains("test too_wide ... FAILED"), "n={n}: {all}");
        assert!(
            all.contains("port 'a' (u4) cannot hold value 31"),
            "n={n}: {all}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn a_chain_past_the_limit_is_rejected_the_same_way_by_check_build_and_test() {
    // 65 sabit: değer 65 düzey derinde. SV üreticisi genişliği seçmez,
    // durur; test dili de genişliği bilmez ve koşu başlamaz. Üç komut aynı
    // tanıları verir; help sınırı söyler.
    let tests = "
test \"never runs\" {
    let dut = M { };
    for i in 31..32 {
        dut.a = i;
        step(1);
        assert_eq(dut.q, i);
    }
}
";
    let verilator = tools::require(tools::Tool::Verilator);
    let dir = temp_dir("past");
    let file = write(&dir, 65, tests);
    let mut seen: Vec<(String, Vec<String>)> = Vec::new();
    for cmd in ["check", "build", "test"] {
        if cmd == "test" && verilator.is_none() {
            continue;
        }
        let (out, all) = volt(cmd, &dir, &file, verilator.as_deref());
        assert_eq!(out.status.code(), Some(1), "{cmd}: {all}");
        assert!(!all.contains("test result"), "{cmd}: {all}");
        assert!(
            !dir.join("build").join("rtl").join("M.sv").exists(),
            "{cmd}"
        );
        let errors: Vec<String> = all
            .lines()
            .filter(|l| l.starts_with("error[") || l.trim_start().starts_with("= help"))
            .map(str::to_string)
            .collect();
        assert!(
            errors.iter().any(|l| l.starts_with("error[E2005]")),
            "{cmd}: {all}"
        );
        assert!(
            all.contains("nested more than 64 levels deep"),
            "{cmd}: {all}"
        );
        seen.push((cmd.to_string(), errors));
    }
    for (cmd, errors) in &seen[1..] {
        assert_eq!(errors, &seen[0].1, "{cmd} ile check aynı tanıları vermeli");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
