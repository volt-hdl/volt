//! Simülasyonda kontratlar (ADR-0064) — CLI düzeyi.
//!
//! Üretilen dosya testleri her ortamda koşar: `VOLT_VERILATOR`
//! çalıştırılamayan bir dosyaya çevrilir, sürücü SV/testbench'i yazar ve
//! araç başlatılamaz (sim_golden_tests hilesi). Çalışma zamanı testleri
//! gerçek bir Verilator ister (`VOLT_VERILATOR` ya da `PATH`); yoksa
//! atlanır — CI'ın Verilator'lu işi bunları koşturur.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// İletiler metin olarak denetlenir: dil ortamdan bağımsız sabitlenir.
fn volt() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_volt"));
    cmd.env("VOLT_LANG", "en");
    cmd
}

fn repo(rel: &str) -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")).join(rel)
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-contracts-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// Satır numaraları testlerde sabittir: Cnt kontratları 15-20., Sub'ınkiler 5-6. satırda.
const DESIGN: &str = "\
module Sub {
    in  clk : clock
    in  x   : u8

    invariant: x < 200
    requires: x != 13
}

module Cnt {
    in  clk   : clock
    in  en    : bool
    in  lim   : u8
    out count : u8

    invariant: count < 50
    ensures: lim != 9 || count != 3
    assume: lim != 7
    invariant: !prev(en) || count != 0
    cover: count == 3
    cover: count == 222

    reg c : u8 = 0
    on clk {
        if en {
            c <= c + 1
        }
    }
    count = c

    let u = Sub { clk: clk, x: lim }
}
";

fn write_design(dir: &Path, tests: &str) -> PathBuf {
    let file = dir.join("cnt_test.volt");
    std::fs::write(&file, format!("{DESIGN}\n{tests}")).expect("yaz");
    file
}

fn test_block(name: &str, body: &str) -> String {
    format!("test \"{name}\" {{\n    let dut = Cnt {{ }};\n{body}}}\n")
}

// ═══ Üretilen dosyalar (Verilator gerekmez) ═══════════════════════

/// Sürücüyü başlatılamayan bir "Verilator" ile koşturur; üretilen sim
/// dizinini döndürür.
fn generate(tag: &str, args: &[&str]) -> (PathBuf, PathBuf) {
    let dir = temp_dir(tag);
    let file = write_design(&dir, &test_block("t", "    step(1);\n"));
    let output = volt()
        .env("VOLT_VERILATOR", &file)
        .args(args)
        .arg("--target-dir")
        .arg(dir.join("build"))
        .current_dir(&dir)
        .output()
        .expect("volt");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(3), "stderr: {stderr}");
    assert!(stderr.contains("error: cannot run"), "stderr: {stderr}");
    let sim = dir.join("build/sim/cnt_test");
    (dir, sim)
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn volt_test_generates_monitors_by_default() {
    let (dir, sim) = generate("gen-on", &["test", "cnt_test.volt"]);
    let sv = read(&sim.join("Cnt.sv"));
    let tb = read(&sim.join("tb_Cnt.cpp"));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(sv.contains("volt_contract_fail(\"Cnt.inv_0\");"), "{sv}");
    assert!(sv.contains("volt_contract_fail(\"Sub.inv_0\");"), "{sv}");
    assert!(sv.contains("volt_cover_report(\"Cnt.cov_1\""), "{sv}");
    assert!(tb.contains("#include \"svdpi.h\""), "{tb}");
    assert!(tb.contains("volt_contracts_begin();"), "{tb}");
}

#[test]
fn no_contracts_flag_generates_no_monitor() {
    let (dir, sim) = generate("gen-off", &["test", "cnt_test.volt", "--no-contracts"]);
    let sv = read(&sim.join("Cnt.sv"));
    let tb = read(&sim.join("tb_Cnt.cpp"));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(!sv.contains("DPI-C"), "{sv}");
    assert!(!sv.contains("volt_"), "{sv}");
    assert!(!tb.contains("svdpi"), "{tb}");
    assert!(!tb.contains("volt_contracts_begin"), "{tb}");
}

#[test]
fn volt_run_adds_monitors_only_with_the_contracts_flag() {
    let run = |tag, extra: &[&str]| {
        let mut args = vec!["run", "cnt_test.volt", "--top", "Cnt"];
        args.extend_from_slice(extra);
        let (dir, _) = generate(tag, &args);
        let sv = read(&dir.join("build/sim/cnt_test/Cnt.sv"));
        let _ = std::fs::remove_dir_all(&dir);
        sv
    };
    assert!(!run("run-off", &[]).contains("DPI-C"));
    assert!(run("run-on", &["--contracts"]).contains("volt_contract_fail(\"Cnt.inv_0\");"));
}

#[test]
fn unsupported_contract_expression_warns_w5001_and_still_builds() {
    let dir = temp_dir("w5001");
    let file = dir.join("m_test.volt");
    std::fs::write(
        &file,
        "module M {\n    in clk : clock\n    in a : u4\n    invariant: match a { 0 => true, _ => a != 7 }\n    invariant: a != 9\n}\n\ntest \"t\" {\n    let dut = M { };\n    step(1);\n}\n",
    )
    .expect("yaz");
    let output = volt()
        .env("VOLT_VERILATOR", &file)
        .args(["test", "m_test.volt", "--target-dir", "build"])
        .current_dir(&dir)
        .output()
        .expect("volt");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let sv = std::fs::read_to_string(dir.join("build/sim/m_test/M.sv")).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    // Derleme geçer, yalnız araç başlatılamaz (çıkış 3, derleme hatası 1 değil).
    assert_eq!(output.status.code(), Some(3), "stderr: {stderr}");
    assert!(stderr.contains("warning[W5001]"), "stderr: {stderr}");
    assert!(!stderr.contains("E0003"), "stderr: {stderr}");
    // İzlenebilen kontrat verify'daki adıyla izlenir.
    assert!(sv.contains("volt_contract_fail(\"M.inv_1\");"), "{sv}");
    assert!(!sv.contains("\"M.inv_0\""), "{sv}");
}

#[test]
fn helps_document_the_contract_flags() {
    for (cmd, flag) in [("test", "--no-contracts"), ("run", "--contracts")] {
        let output = volt().args([cmd, "--help"]).output().expect("volt");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains(flag), "{cmd}: {stdout}");
        assert!(stdout.contains("ADR-0064"), "{cmd}: {stdout}");
    }
}

// ═══ Ham reset portu (ADR-0065) ile birlikte ═══════════════════════

/// Ham reset portlu tasarım: izleyici koruması senkronizör ÇIKIŞIYLA
/// (`rst_sync_clk_stage1`) yapılır, testbench ham portu polaritesiyle
/// sürer ve bırakma zincirini bekler. Satırlar: invariant 9, cover 10.
const RAW_RESET_DESIGN: &str = "domain D { clock = posedge, reset = async active_low }

module RstCnt {
    in  clk   : clock @D
    in  rst_n : reset(async, active_low)
    in  en    : bool
    out count : u8

    invariant: count < 10
    cover: count == 3

    reg c : u8 = 0
    on clk {
        if en {
            if c == 10 {
                c <= 0
            } else {
                c <= c + 1
            }
        }
    }
    count = c
}
";

const RAW_RESET_TESTS: &str = "test \"holds_below_limit\" {
    let dut = RstCnt { };
    dut.en = true;
    step(5);
    assert_eq(dut.count, 5);
}

test \"reaches_limit\" {
    let dut = RstCnt { };
    dut.en = true;
    step(20);
}
";

fn write_raw_reset_design(dir: &Path) -> PathBuf {
    let file = dir.join("rstcnt_test.volt");
    std::fs::write(
        &file,
        format!(
            "{RAW_RESET_DESIGN}
{RAW_RESET_TESTS}"
        ),
    )
    .expect("yaz");
    file
}

#[test]
fn raw_reset_monitor_is_guarded_by_the_synchronizer_output() {
    let dir = temp_dir("raw-gen");
    let file = write_raw_reset_design(&dir);
    let output = volt()
        .env("VOLT_VERILATOR", &file)
        .args(["test", "rstcnt_test.volt", "--target-dir", "build"])
        .current_dir(&dir)
        .output()
        .expect("volt");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let sim = dir.join("build/sim/rstcnt_test");
    let sv = std::fs::read_to_string(sim.join("RstCnt.sv")).unwrap_or_default();
    let tb = std::fs::read_to_string(sim.join("tb_RstCnt.cpp")).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(output.status.code(), Some(3), "stderr: {stderr}");
    // Zincir gövdenin başında, izleyiciler register'larla AYNI sinyalle korunur.
    assert!(sv.contains("// reset synchronizer: rst_n -> clk"), "{sv}");
    assert!(
        sv.contains(
            "if (!rst_sync_clk_stage1) begin
            c <= 8'd0;"
        ),
        "{sv}"
    );
    assert!(
        sv.contains("if (!(!rst_sync_clk_stage1)) if (!(count < 8'd10)) volt_contract_fail(\"RstCnt.inv_0\");"),
        "{sv}"
    );
    assert!(
        sv.contains("if (!(!rst_sync_clk_stage1)) if (count == 8'd3) volt_hits_cov_0 <= volt_hits_cov_0 + 1;"),
        "{sv}"
    );
    assert!(
        !sv.contains("volt_contract_fail") || !sv.contains("if (!(!rst_n))"),
        "{sv}"
    );
    // Testbench ham portu sürer, bırakmadan sonra zincir kadar bekler,
    // çevrim sayacını ondan SONRA sıfırlar; reset veri gibi sıfırlanmaz.
    let reset = tb.find("static void apply_reset").expect("apply_reset");
    let body = &tb[reset
        ..tb[reset..]
            .find(
                "}
",
            )
            .map_or(tb.len(), |i| reset + i)];
    assert!(body.contains("dut->rst_n = 0;"), "{body}");
    assert!(
        body.contains(
            "dut->rst_n = 1;
    // reset synchronizer release (ADR-0065)
    run_cycle(dut, ctx);
    run_cycle(dut, ctx);"
        ),
        "{body}"
    );
    assert!(
        tb.contains(
            "    apply_reset(&dut, ctx);
    volt_contracts_begin();
"
        ),
        "{tb}"
    );
    assert!(!tb.contains("dut.rst_n = 0;"), "{tb}");
}

// ═══ Çalışma zamanı: gerçek Verilator ═════════════════════════════

fn have_verilator() -> bool {
    if std::env::var_os("VOLT_VERILATOR").is_some_and(|p| Path::new(&p).is_file()) {
        return true;
    }
    std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path).any(|dir| {
            ["verilator", "verilator.exe"]
                .iter()
                .any(|name| dir.join(name).is_file())
        })
    })
}

/// `volt <args>` koşturur (cwd: tasarımın dizini); Verilator yoksa None.
fn run_volt(tag: &str, tests: &str, args: &[&str]) -> Option<(Option<i32>, String)> {
    if !have_verilator() {
        eprintln!("atlandı: Verilator yok ({tag})");
        return None;
    }
    let dir = temp_dir(tag);
    write_design(&dir, tests);
    let output: Output = volt()
        .current_dir(&dir)
        .args(args)
        .args(["--target-dir", "build"])
        .output()
        .expect("volt");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let _ = std::fs::remove_dir_all(&dir);
    Some((
        output.status.code(),
        format!("{stdout}\n--- stderr\n{stderr}"),
    ))
}

fn run_test(tag: &str, body: &str) -> Option<(Option<i32>, String)> {
    run_volt(tag, &test_block(tag, body), &["test", "cnt_test.volt"])
}

#[test]
fn invariant_violation_fails_the_test_at_its_source_line() {
    let Some((code, out)) = run_test(
        "inv",
        "    dut.en = true;\n    step(60);\n    assert_eq(dut.count, 60);\n",
    ) else {
        return;
    };
    assert_eq!(code, Some(5), "{out}");
    assert!(out.contains("test inv ... FAILED"), "{out}");
    assert!(
        out.contains("  contract violated: invariant (cnt_test.volt:15)\n    invariant: count < 50\n  at cycle 51\n  in instance: dut\n"),
        "{out}"
    );
    // Test ihlal çevriminde durur: sonraki assert_eq raporlanmaz.
    assert!(!out.contains("assert_eq failed"), "{out}");
}

#[test]
fn ensures_violation_is_a_contract_violation() {
    let Some((code, out)) = run_test(
        "ens",
        "    dut.lim = 9;\n    dut.en = true;\n    step(10);\n",
    ) else {
        return;
    };
    assert_eq!(code, Some(5), "{out}");
    assert!(
        out.contains("  contract violated: ensures (cnt_test.volt:16)\n    ensures: lim != 9 || count != 3\n  at cycle 4\n"),
        "{out}"
    );
}

#[test]
fn assume_violation_is_classified_as_stimulus_error() {
    let Some((code, out)) = run_test("asm", "    dut.lim = 7;\n    step(2);\n") else {
        return;
    };
    assert_eq!(code, Some(5), "{out}");
    assert!(
        out.contains("  assumption violated by test stimulus: assume (cnt_test.volt:17)\n    assume: lim != 7\n  at cycle 1\n"),
        "{out}"
    );
    assert!(out.contains("fix the stimulus"), "{out}");
    assert!(!out.contains("contract violated"), "{out}");
}

#[test]
fn never_hit_cover_is_listed_without_failing() {
    let Some((code, out)) = run_test(
        "cov",
        "    dut.en = true;\n    step(5);\n    assert_eq(dut.count, 5);\n",
    ) else {
        return;
    };
    assert_eq!(code, Some(0), "{out}");
    assert!(out.contains("cover summary:"), "{out}");
    assert!(
        out.contains("  Cnt.cov_0 (cnt_test.volt:19)  hit 1 time"),
        "{out}"
    );
    assert!(
        out.contains("  Cnt.cov_1 (cnt_test.volt:20)  NEVER HIT"),
        "{out}"
    );
    assert!(out.contains("test result: ok. 1 passed; 0 failed"), "{out}");
}

#[test]
fn violation_while_reset_is_held_is_not_reported() {
    // lim = 250 yalnız reset() süresince: Sub'ın `x < 200`'ü kapalıdır.
    let Some((code, out)) = run_test(
        "reset",
        "    dut.lim = 250;\n    reset();\n    dut.lim = 0;\n    step(3);\n",
    ) else {
        return;
    };
    assert_eq!(code, Some(0), "{out}");
    assert!(!out.contains("contract violated"), "{out}");
}

#[test]
fn prev_is_zero_in_the_first_cycle_after_reset_like_formal() {
    // en reset boyunca 1: `$past(en)` 1 olurdu ve `!prev(en) || count
    // != 0` ilk çevrimde (count = 0) yanlış alarm verirdi. ADR-0040:
    // yardımcı reg reset'te 0'lanır — formal ile aynı anlam.
    let Some((code, out)) = run_test(
        "prev",
        "    dut.en = true;\n    reset();\n    step(3);\n    assert_eq(dut.count, 3);\n",
    ) else {
        return;
    };
    assert_eq!(code, Some(0), "{out}");
}

#[test]
fn submodule_violation_names_the_instance_path() {
    let Some((code, out)) = run_test("sub", "    dut.lim = 250;\n    step(2);\n") else {
        return;
    };
    assert_eq!(code, Some(5), "{out}");
    assert!(
        out.contains("  contract violated: invariant (cnt_test.volt:5)\n    invariant: x < 200\n  at cycle 1\n  in instance: dut.u\n"),
        "{out}"
    );
}

#[test]
fn submodule_requires_broken_by_its_parent_is_a_design_violation() {
    // Sub'ın `requires: x != 13`'ünü test değil Cnt (x = lim) bozar.
    let Some((code, out)) = run_test("subreq", "    dut.lim = 13;\n    step(2);\n") else {
        return;
    };
    assert_eq!(code, Some(5), "{out}");
    assert!(
        out.contains("  contract violated: requires (cnt_test.volt:6)\n    requires: x != 13\n  at cycle 1\n  in instance: dut.u\n"),
        "{out}"
    );
    assert!(out.contains("the parent broke it"), "{out}");
    assert!(
        !out.contains("assumption violated by test stimulus"),
        "{out}"
    );
}

#[test]
fn violation_inside_a_for_loop_reports_the_iteration() {
    let Some((code, out)) = run_test(
        "loop",
        "    for i in 0..5 {\n        dut.lim = 196 + i;\n        step(1);\n    }\n",
    ) else {
        return;
    };
    assert_eq!(code, Some(5), "{out}");
    assert!(out.contains("  at cycle 5\n"), "{out}");
    assert!(out.contains("  loop: i = 4\n"), "{out}");
}

#[test]
fn no_contracts_flag_lets_a_violating_test_pass() {
    let Some((code, out)) = run_volt(
        "off",
        &test_block("off", "    dut.lim = 250;\n    step(2);\n"),
        &["test", "cnt_test.volt", "--no-contracts"],
    ) else {
        return;
    };
    assert_eq!(code, Some(0), "{out}");
    assert!(!out.contains("cover summary"), "{out}");
}

#[test]
fn other_tests_keep_running_after_a_violation() {
    let tests = format!(
        "{}{}",
        test_block("bad", "    dut.lim = 250;\n    step(1);\n"),
        test_block("good", "    step(2);\n    assert_eq(dut.count, 0);\n")
    );
    let Some((code, out)) = run_volt("two", &tests, &["test", "cnt_test.volt"]) else {
        return;
    };
    assert_eq!(code, Some(5), "{out}");
    assert!(out.contains("test bad ... FAILED"), "{out}");
    assert!(out.contains("test good ... ok"), "{out}");
}

#[test]
fn raw_reset_design_runs_monitors_after_the_synchronizer_release() {
    // ADR-0064 + ADR-0065 birlikte: apply_reset zinciri bıraktıktan sonra
    // sayaç sıfırlanır; 5 çevrimde count == 5 (bırakma sırasında sahte
    // ihlal yok), 20 çevrimde count 10'a ulaşır → 11. çevrimde ihlal.
    if !have_verilator() {
        eprintln!("atlandı: Verilator yok (raw-run)");
        return;
    }
    let dir = temp_dir("raw-run");
    write_raw_reset_design(&dir);
    let output = volt()
        .current_dir(&dir)
        .args(["test", "rstcnt_test.volt", "--target-dir", "build"])
        .output()
        .expect("volt");
    let _ = std::fs::remove_dir_all(&dir);
    let out = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(5), "{out}");
    assert!(out.contains("test holds_below_limit ... ok"), "{out}");
    assert!(out.contains("test reaches_limit ... FAILED"), "{out}");
    assert!(
        out.contains(
            "  contract violated: invariant (rstcnt_test.volt:9)
    invariant: count < 10
  at cycle 11
  in instance: dut
"
        ),
        "{out}"
    );
    assert!(
        out.contains("  RstCnt.cov_0 (rstcnt_test.volt:10)  hit 2 times"),
        "{out}"
    );
}

#[test]
fn deep_violation_beyond_formal_depth_is_caught_by_simulation() {
    // ADR-0064 kanıtı: `volt verify --mode bmc --depth 20` bu dosyada
    // geçer (CI formal işi); 200 çevrimlik test 101. çevrimde düşer.
    if !have_verilator() {
        eprintln!("atlandı: Verilator yok (deep)");
        return;
    }
    let dir = temp_dir("deep");
    let output = volt()
        .arg("test")
        .arg(repo("tests/fixtures/sim_contracts/deep_violation.volt"))
        .arg("--target-dir")
        .arg(dir.join("build"))
        .output()
        .expect("volt");
    let _ = std::fs::remove_dir_all(&dir);
    let out = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(5), "{out}");
    assert!(
        out.contains("  contract violated: invariant (deep_violation.volt:14)\n    invariant: count < 100\n  at cycle 101\n"),
        "{out}"
    );
}

#[test]
fn volt_run_with_contracts_reports_and_exits_five() {
    // Duman uyarıcısı girişleri 1'e sürer: en = 1, lim = 1 → count 49'u
    // geçince `count < 50` bozulur (çevrim 51).
    let Some((code, out)) = run_volt(
        "run",
        "",
        &[
            "run",
            "cnt_test.volt",
            "--top",
            "Cnt",
            "--cycles",
            "60",
            "--contracts",
        ],
    ) else {
        return;
    };
    assert_eq!(code, Some(5), "{out}");
    assert!(
        out.contains("  contract violated: invariant (cnt_test.volt:15)"),
        "{out}"
    );
    assert!(out.contains("  at cycle 51\n"), "{out}");
    assert!(!out.contains("VOLT-"), "tablo temiz olmalı: {out}");
}
