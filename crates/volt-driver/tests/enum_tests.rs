//! Enum desteği uçtan uca (ADR-0074): ui/fail tanılarının kodu ve satırı
//! (tip denetimi, SV üretimi ve parser katmanları birlikte), üretilen SV
//! biçimi (`localparam`, `case`/`default`, kod dönüşümü) ve otomatik
//! kontratlar.

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

/// `//~ KOD` + `//~^ ERROR` satırı: kod, anotasyonun üstündeki satırda
/// (1 tabanlı) raporlanmalı — `volt check`, bütün katmanlar.
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
    let line = src
        .lines()
        .position(|l| l.trim_start().starts_with("//~^ ERROR"))
        .expect("//~^ ERROR satırı") as u64; // üst satır (1 tabanlı)
    let env = check_json(&file);
    let found: Vec<u64> = env["diagnostics"]
        .as_array()
        .expect("tanılar")
        .iter()
        .filter(|d| d["code"] == code.as_str())
        .filter_map(|d| d["spans"][0]["start"]["line"].as_u64())
        .collect();
    assert!(
        found.contains(&line),
        "{name}: {code} satır {line} bekleniyor, bulunan {found:?}\n{env:#}"
    );
}

#[test]
fn ui_fail_85_enum_match_missing_variant_e0014() {
    assert_ui_fail("85_enum_match_missing_variant.volt");
}

#[test]
fn ui_fail_86_enum_mixed_values_e2030() {
    assert_ui_fail("86_enum_mixed_values.volt");
}

#[test]
fn ui_fail_87_enum_ordering_e2003() {
    assert_ui_fail("87_enum_ordering.volt");
}

#[test]
fn ui_fail_88_uint_to_enum_cast_e2009() {
    assert_ui_fail("88_uint_to_enum_cast.volt");
}

#[test]
fn ui_fail_89_enum_unreachable_arm_w2014() {
    assert_ui_fail("89_enum_unreachable_arm.volt");
}

#[test]
fn ui_fail_90_enum_payload_signal_e0003() {
    assert_ui_fail("90_enum_payload_signal.volt");
}

#[test]
fn ui_fail_91_enum_value_too_wide_e2010() {
    assert_ui_fail("91_enum_value_too_wide.volt");
}

#[test]
fn ui_fail_92_enum_sv_name_clash_e1003() {
    assert_ui_fail("92_enum_sv_name_clash.volt");
}

// ═══ SV üretimi ═══════════════════════════════════════════════════════

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-enum-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("geçici dizin");
    dir
}

/// Kaynağı derler; (modül adı → SV) listesi.
fn build(tag: &str, src: &str, emit: Option<&str>) -> Vec<(String, String)> {
    let dir = temp_dir(tag);
    let file = dir.join(format!("{tag}.volt"));
    std::fs::write(&file, src).expect("yazılmalı");
    let target = dir.join("out");
    let mut args = vec!["build", "--target-dir", target.to_str().unwrap()];
    if let Some(e) = emit {
        args.push(e);
    }
    args.push(file.to_str().unwrap());
    let out = volt(&args);
    assert!(
        out.status.success(),
        "build başarısız:\n{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let mut files = Vec::new();
    for sub in ["rtl", "formal"] {
        let Ok(rd) = std::fs::read_dir(target.join(sub)) else {
            continue;
        };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            files.push((name, std::fs::read_to_string(e.path()).unwrap()));
        }
    }
    files.sort();
    files
}

fn sv_of<'a>(files: &'a [(String, String)], name: &str) -> &'a str {
    &files
        .iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| {
            panic!(
                "{name} yok: {:?}",
                files.iter().map(|f| &f.0).collect::<Vec<_>>()
            )
        })
        .1
}

const FSM: &str = "enum State { Idle, Run, Done }
enum Op : u4 { Add = 0, Sub = 1, Jal = 8 }

module Fsm {
    in  clk  : clock
    in  go   : bool
    out st   : State
    out code : u8

    reg s  : State = State::Idle
    reg op : Op    = Op::Add

    on clk {
        match s {
            State::Idle => { if go { s <= State::Run } }
            State::Run => { s <= State::Done  op <= Op::Jal }
            State::Done => { s <= State::Idle }
        }
    }

    st   = s
    code = op as u8
}

module Top {
    in  clk  : clock
    in  go   : bool
    out done : bool

    let f = Fsm { clk: clk, go: go }
    done = f.st == State::Done
}
";

#[test]
fn enum_signals_lower_to_logic_with_used_localparams() {
    let files = build("fsm", FSM, None);
    let fsm = sv_of(&files, "Fsm.sv");
    assert!(
        fsm.contains("    // enum State : Idle = 0, Run = 1, Done = 2\n"),
        "{fsm}"
    );
    assert!(
        fsm.contains("    localparam logic [1:0] State_Idle = 2'd0;"),
        "{fsm}"
    );
    assert!(
        fsm.contains("    localparam logic [1:0] State_Done = 2'd2;"),
        "{fsm}"
    );
    assert!(
        fsm.contains("    // enum Op : Add = 0, Sub = 1, Jal = 8\n"),
        "{fsm}"
    );
    assert!(
        fsm.contains("    localparam logic [3:0] Op_Jal = 4'd8;"),
        "{fsm}"
    );
    // Kullanılmayan varyant bildirilmez (Verilator UNUSEDPARAM).
    assert!(!fsm.contains("Op_Sub ="), "{fsm}");
    assert!(fsm.contains("output logic [1:0] st,  // State"), "{fsm}");
    assert!(fsm.contains("    logic [1:0] s;  // State"), "{fsm}");
    assert!(fsm.contains("    logic [3:0] op;  // Op"), "{fsm}");
}

#[test]
fn exhaustive_enum_match_makes_the_last_arm_default() {
    let files = build("case", FSM, None);
    let fsm = sv_of(&files, "Fsm.sv");
    assert!(fsm.contains("State_Idle: begin"), "{fsm}");
    assert!(fsm.contains("State_Run: begin"), "{fsm}");
    assert!(
        fsm.contains("default: begin // State_Done (and invalid codes)"),
        "{fsm}"
    );
    assert!(!fsm.contains("State_Done: begin"), "{fsm}");
}

#[test]
fn enum_to_uint_cast_zero_extends_with_a_size_cast() {
    let files = build("cast", FSM, None);
    let fsm = sv_of(&files, "Fsm.sv");
    assert!(fsm.contains("assign code = 8'(op);"), "{fsm}");
}

#[test]
fn every_module_file_declares_its_own_localparams() {
    let files = build("ports", FSM, None);
    let top = sv_of(&files, "Top.sv");
    assert!(
        top.contains("localparam logic [1:0] State_Done = 2'd2;"),
        "{top}"
    );
    assert!(!top.contains("State_Idle"), "{top}");
    assert!(top.contains("assign done = f_st == State_Done;"), "{top}");
}

#[test]
fn separate_sva_file_carries_its_own_localparams() {
    let files = build("sva", FSM, Some("--emit=sva"));
    let sva = sv_of(&files, "fsm.sva");
    assert!(
        sva.contains("localparam logic [1:0] State_Idle = 2'd0;"),
        "{sva}"
    );
    assert!(
        sva.contains("state valid: s == State::Idle || s == State::Run || s == State::Done"),
        "{sva}"
    );
    assert!(
        sva.contains("s == State_Idle || s == State_Run || s == State_Done;"),
        "{sva}"
    );
}

#[test]
fn duplicate_arm_is_not_emitted() {
    let src = FSM.replace(
        "State::Done => { s <= State::Idle }",
        "State::Done => { s <= State::Idle }\n            State::Run => { s <= State::Idle }",
    );
    let files = build("dup", &src, None);
    let fsm = sv_of(&files, "Fsm.sv");
    assert_eq!(fsm.matches("State_Run: begin").count(), 1, "{fsm}");
}

#[test]
fn enum_const_lowers_to_the_variant_localparam() {
    let src = "enum State { Idle, Run }
const START : State = State::Run
module M {
    in  clk : clock
    out y   : bool
    reg s : State = START
    on clk { s <= State::Idle }
    y = s == START
}
";
    let files = build("const", src, None);
    let m = sv_of(&files, "M.sv");
    assert!(m.contains("s <= State_Run;"), "{m}");
    assert!(m.contains("assign y = s == State_Run;"), "{m}");
}

#[test]
fn multibit_enum_through_sync_warns_w3003() {
    let src = "enum State { Idle, Run, Done }
domain Fast { clock = posedge }
domain Slow { clock = posedge }
module M {
    in  fclk : clock @Fast
    in  sclk : clock @Slow
    in  a : State @Fast
    out y : State @Slow
    y = sync(a, sclk)
}
";
    let dir = temp_dir("w3003");
    let file = dir.join("w.volt");
    std::fs::write(&file, src).unwrap();
    let env = check_json(&file);
    let codes: Vec<&str> = env["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|d| d["code"].as_str())
        .collect();
    assert!(codes.contains(&"W3003"), "{codes:?}");
    // İki varyantlı enum tek bittir: uyarı yok.
    std::fs::write(&file, src.replace("{ Idle, Run, Done }", "{ Idle, Run }")).unwrap();
    let env = check_json(&file);
    assert!(
        !env["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "W3003"),
        "{env:#}"
    );
}
