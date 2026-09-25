//! Struct desteği uçtan uca (ADR-0077): ui/fail tanılarının kodu ve satırı
//! (parser, tip denetimi, sürücü analizi ve SV üretimi birlikte), üretilen
//! SV'nin B eşlemesi (yaprak sinyalleri, düzen yorumu, birleştirme,
//! dilim, örnek bağlantısı, lint susturması), parite ve golden ilkesi
//! (struct kullanmayan tasarımın çıktısı değişmez).

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
        .expect("//~^ ERROR satırı") as u64;
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
fn ui_fail_102_struct_empty_e2013() {
    assert_ui_fail("102_struct_empty.volt");
}

#[test]
fn ui_fail_103_struct_field_domain_e2013() {
    assert_ui_fail("103_struct_field_domain.volt");
}

#[test]
fn ui_fail_104_struct_bundle_field_e2013() {
    assert_ui_fail("104_struct_bundle_field.volt");
}

#[test]
fn ui_fail_105_struct_duplicate_field_e1003() {
    assert_ui_fail("105_struct_duplicate_field.volt");
}

#[test]
fn ui_fail_106_struct_literal_missing_field_e2014() {
    assert_ui_fail("106_struct_literal_missing_field.volt");
}

#[test]
fn ui_fail_107_struct_literal_duplicate_field_e2014() {
    assert_ui_fail("107_struct_literal_duplicate_field.volt");
}

#[test]
fn ui_fail_108_struct_unknown_field_access_e1008() {
    assert_ui_fail("108_struct_unknown_field_access.volt");
}

#[test]
fn ui_fail_109_struct_ordering_e2003() {
    assert_ui_fail("109_struct_ordering.volt");
}

#[test]
fn ui_fail_110_struct_narrowing_cast_e2009() {
    assert_ui_fail("110_struct_narrowing_cast.volt");
}

#[test]
fn ui_fail_111_bits_to_struct_with_enum_e2009() {
    assert_ui_fail("111_bits_to_struct_with_enum.volt");
}

#[test]
fn ui_fail_112_struct_field_double_driver_e4001() {
    assert_ui_fail("112_struct_field_double_driver.volt");
}

#[test]
fn ui_fail_113_struct_field_undriven_e4012() {
    assert_ui_fail("113_struct_field_undriven.volt");
}

#[test]
fn ui_fail_114_vector_bits_undriven_e4012() {
    assert_ui_fail("114_vector_bits_undriven.volt");
}

#[test]
fn ui_fail_115_struct_array_e0003() {
    assert_ui_fail("115_struct_array.volt");
}

#[test]
fn ui_fail_116_handshake_whole_payload_e0003() {
    assert_ui_fail("116_handshake_whole_payload.volt");
}

#[test]
fn ui_fail_117_struct_literal_unparenthesized_e0001() {
    assert_ui_fail("117_struct_literal_unparenthesized.volt");
}

#[test]
fn ui_fail_118_struct_sv_name_clash_e1003() {
    assert_ui_fail("118_struct_sv_name_clash.volt");
}

#[test]
fn ui_fail_119_struct_reset_not_constant_e2021() {
    assert_ui_fail("119_struct_reset_not_constant.volt");
}

#[test]
fn ui_fail_120_struct_field_direction_e0001() {
    assert_ui_fail("120_struct_field_direction.volt");
}

#[test]
fn ui_fail_121_struct_nesting_budget_e4010() {
    assert_ui_fail("121_struct_nesting_budget.volt");
}

// ═══ SV üretimi (Karar 5 — B eşlemesi) ═══════════════════════════════

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-struct-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("geçici dizin");
    dir
}

/// Kaynağı derler; (dosya adı → içerik) listesi.
fn build(tag: &str, src: &str, emit: &[&str]) -> Vec<(String, String)> {
    let dir = temp_dir(tag);
    let file = dir.join(format!("{tag}.volt"));
    std::fs::write(&file, src).expect("yazılmalı");
    let target = dir.join("out");
    let mut args = vec!["build", "--target-dir", target.to_str().unwrap()];
    args.extend_from_slice(emit);
    args.push(file.to_str().unwrap());
    let out = volt(&args);
    assert!(
        out.status.success(),
        "build başarısız:\n{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let mut files = Vec::new();
    for sub in ["rtl", "formal", "constraints"] {
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

/// ADR-0077 Karar 3 tablosunun tasarımı: a[11:8] s[7:6] b[5] i.x[4:2] i.y[1:0].
const STAGE: &str = "enum St { Idle, Run, Done }
struct Inner {
    x : u3
    y : i2
}
struct P {
    a : u4
    s : St
    b : bool
    i : Inner
}

module Stage {
    in  clk : clock
    in  ld  : bool
    in  d   : P
    out q   : P
    out hit : bool
    out raw : u12

    reg p : P = P { a: 0, s: St::Idle, b: false, i: Inner { x: 0, y: 0 } }
    wire w : P
    w.a = d.a + 1
    w.s = d.s
    w.b = !d.b
    w.i = d.i

    on clk {
        if ld {
            p <= w
        } else {
            p.i.x <= p.i.x + 1
        }
    }

    q = p
    hit = p == d
    raw = p as u12
}

module Top {
    in  clk : clock
    in  ld  : bool
    in  d   : P
    out raw : u12
    out neg : bool

    let s0 = Stage { clk: clk, ld: ld, d: d }
    let s1 = Stage { clk: clk, ld: ld, d: s0.q }
    raw = s1.raw
    neg = s1.q.i.y < 0
}
";

#[test]
fn struct_signals_become_leaf_signals_with_a_layout_comment() {
    let files = build("stage", STAGE, &[]);
    let stage = sv_of(&files, "Stage.sv");
    // Kural 1-3: yaprak adları, bildirim sırası, işaretli/enum yaprak.
    assert!(
        stage.contains("    // struct P d : a[11:8] s[7:6] b[5] i.x[4:2] i.y[1:0]\n"),
        "düzen yorumu (ilk alan MSB):\n{stage}"
    );
    for decl in [
        "input  logic [3:0]        d_a,",
        "input  logic [1:0]        d_s,  // St",
        "input  logic              d_b,",
        "input  logic [2:0]        d_i_x,",
        "input  logic signed [1:0] d_i_y,",
        "output logic signed [1:0] q_i_y,",
    ] {
        assert!(stage.contains(decl), "{decl} beklenir:\n{stage}");
    }
    // Kural 6: bütün atama yaprak başına, aynı blokta.
    assert!(
        stage.contains("p_a <= w_a;\n") && stage.contains("p_i_y <= w_i_y;\n"),
        "{stage}"
    );
    // Kural 8 ve 9: eşitlik ve `as uN` birleştirmesi, bildirim sırasıyla.
    assert!(
        stage.contains(
            "assign hit = {p_a, p_s, p_b, p_i_x, p_i_y} == {d_a, d_s, d_b, d_i_x, d_i_y};"
        ),
        "{stage}"
    );
    assert!(
        stage.contains("assign raw = {p_a, p_s, p_b, p_i_x, p_i_y};"),
        "{stage}"
    );
    // Alt struct bütün ataması: `w.i = d.i` → iki yaprak.
    assert!(
        stage.contains("assign w_i_x = d_i_x;") && stage.contains("assign w_i_y = d_i_y;"),
        "{stage}"
    );
}

#[test]
fn struct_ports_connect_leaf_by_leaf_across_modules() {
    let files = build("top", STAGE, &[]);
    let top = sv_of(&files, "Top.sv");
    // Kural 11: örnek bağlantısı yaprak başına; çıkış `s0.q` → `s0_q_a` ...
    assert!(top.contains(".d_i_y(d_i_y)"), "{top}");
    assert!(
        top.contains(".d_a  (s0_q_a)") || top.contains(".d_a(s0_q_a)"),
        "{top}"
    );
    assert!(top.contains("assign neg = s1_q_i_y < 2'sd0;"), "{top}");
}

#[test]
fn bits_to_struct_slices_by_the_layout() {
    let src = "struct RType {
    funct7 : u7
    rs2    : u5
    rs1    : u5
    funct3 : u3
    rd     : u5
    opcode : u7
}
module D {
    in  instr : u32
    out rd    : u5
    out op    : u7
    out f3_is5 : bool
    out same  : u32
    let r : RType = instr as RType
    rd = r.rd
    op = r.opcode
    f3_is5 = (instr as RType).funct3 == 5
    same = r as u32
}
";
    let files = build("rtype", src, &[]);
    let d = sv_of(&files, "D.sv");
    // Kural 9: RISC-V R-tipi şeması birebir (funct7 bit 31'de).
    assert!(d.contains("wire [6:0] r_funct7 = instr[31:25];"), "{d}");
    assert!(d.contains("wire [4:0] r_rd = instr[11:7];"), "{d}");
    assert!(d.contains("wire [6:0] r_opcode = instr[6:0];"), "{d}");
    assert!(d.contains("assign f3_is5 = instr[14:12] == 3'd5;"), "{d}");
}

#[test]
fn unread_struct_leaves_are_silenced_one_by_one() {
    let src = "struct Cmd {
    op : u2
    a  : u4
}
module M {
    in  cmd : Cmd
    out y   : u4
    y = cmd.a
}
";
    let files = build("unused", src, &[]);
    let m = sv_of(&files, "M.sv");
    // Kural 13: yalnız okunmayan yaprak susturulur, alan başına.
    assert!(
        m.contains(
            "    // struct field unused in this module\n    // verilator lint_off UNUSEDSIGNAL\n    input  logic [1:0] cmd_op,\n    // verilator lint_on UNUSEDSIGNAL\n    input  logic [3:0] cmd_a,"
        ),
        "{m}"
    );
}

#[test]
fn struct_register_resets_every_field_of_a_partly_written_register() {
    let src = "struct Cfg {
    en  : bool
    div : u4
    v   : [u2; 2]
}
module M {
    in  clk : clock
    in  d   : u4
    out y   : u4
    out z   : u9
    reg c : Cfg = Cfg { en: true, div: 3, v: [1, 2] }
    on clk { c.div <= d }
    y = c.div
    z = c as u9
}
";
    let files = build("reset", src, &[]);
    let m = sv_of(&files, "M.sv");
    // Yazılmayan yaprak reset değerini korur (Karar 4); dizi yaprağı
    // paketlenmiş vektör, eleman 0 en düşük bitlerde (ADR-0056).
    assert!(m.contains("c_en <= 1'b1;"), "{m}");
    assert!(m.contains("c_v <= {2'd2, 2'd1};"), "{m}");
    assert!(m.contains("logic [3:0] c_v;"), "{m}");
    assert!(m.contains("assign z = {c_en, c_div, c_v};"), "{m}");
}

#[test]
fn struct_contracts_lower_to_leaf_properties() {
    let src = "struct Pair {
    lo : u2
    hi : u2
}
module M {
    in  clk : clock
    in  lo  : u2
    out y   : u2
    reg p : Pair = Pair { lo: 0, hi: 0 }
    on clk {
        p.lo <= lo
        p.hi <= p.lo
    }
    invariant: p.hi <= 3
    cover: p == (Pair { lo: 1, hi: 1 })
    assert: prev(p).lo == p.hi
    y = p.hi
}
";
    let files = build("contracts", src, &["--emit=sva", "--sva", "inline"]);
    let m = sv_of(&files, "M.sv");
    assert!(m.contains("{p_lo, p_hi} == {2'd1, 2'd1};"), "{m}");
    assert!(m.contains("$past(p_lo) == p_hi;"), "{m}");
}

#[test]
fn struct_sync_constrains_every_leaf_synchronizer() {
    let src = "domain Fast {
    clock = posedge
    reset = sync active_high
    frequency = 100_000_000
}
domain Slow {
    clock = posedge
    reset = sync active_high
    frequency = 25_000_000
}
struct Pair {
    lo : u2
    hi : bool
}
module M {
    in  fast : clock @Fast
    in  slow : clock @Slow
    in  x    : u2 @Fast
    out y    : bool @Slow
    reg p : Pair = Pair { lo: 0, hi: false }
    on fast { p.lo <= x }
    wire s : Pair
    s = sync(p, slow)
    y = s.hi
}
";
    let files = build("sync", src, &["--emit=sdc"]);
    let m = sv_of(&files, "M.sv");
    assert!(m.contains("// CDC synchronizer: p_lo -> slow"), "{m}");
    assert!(m.contains("// CDC synchronizer: p_hi -> slow"), "{m}");
    let sdc = sv_of(&files, "M.sdc");
    assert!(sdc.contains("sync_p_lo_stage0_reg*"), "{sdc}");
    assert!(sdc.contains("sync_p_hi_stage0_reg*"), "{sdc}");
}

// ═══ Parite ve golden ════════════════════════════════════════════════

/// Karar 5 kural 15: struct kullanmayan (Handshake payload'lı dahil)
/// tasarımın indirgemesi yoktur — aynı kaynağın SV'si struct bildirimi
/// eklenince değişmez.
#[test]
fn designs_without_struct_signals_emit_the_same_sv() {
    let base = "module M {
    in  clk : clock
    in  a   : u8
    out y   : u8
    reg r : u8 = 0
    on clk { r <= a }
    y = r
}
";
    let with_decl = format!("struct Unused {{\n    a : u4\n}}\n{base}");
    let plain = build("plain", base, &[]);
    let decl = build("decl", &with_decl, &[]);
    let body = |files: &[(String, String)]| {
        sv_of(files, "M.sv")
            .lines()
            .filter(|l| !l.starts_with("// Source:"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(body(&plain), body(&decl));
}

// ═══ Simülasyon (Karar 6 — test dili; gerçek Verilator) ═══════════════

fn have_verilator() -> bool {
    // ADR-0079 §3: VOLT_REQUIRE_TOOLS=verilator ise yokluk atlama değil hata.
    tools::require(tools::Tool::Verilator).is_some()
}

const ALU: &str = "enum Op { Add, Sub, Pass }
struct Inner {
    x : u3
    y : i2
}
struct Cmd {
    op  : Op
    a   : u4
    en  : bool
    in2 : Inner
}
module Alu {
    in  clk : clock
    in  cmd : Cmd
    out res : Cmd
    out sum : u5
    reg r : Cmd = Cmd { op: Op::Pass, a: 0, en: false, in2: Inner { x: 0, y: 0 } }
    on clk {
        if cmd.en {
            r <= cmd
        } else {
            r.a <= r.a + 1
        }
    }
    res = r
    sum = (r.a as u5) + (r.in2.x as u5)
}
";

const ALU_TEST: &str = "test \"whole and field\" {
    let dut = Alu { };
    dut.cmd = Cmd { op: Op::Sub, a: 5, en: true, in2: Inner { x: 3, y: 0 - 1 } };
    step(1);
    assert_eq(dut.res, Cmd { op: Op::Sub, a: 5, en: true, in2: Inner { x: 3, y: 0 - 1 } });
    assert_eq(dut.res.a, 5);
    assert_eq(dut.res.op, Op::Sub);
    dut.cmd.en = false;
    step(2);
    assert_eq(dut.res.a, 7);
    assert_eq(dut.sum, 10);
}

test \"field report\" {
    let dut = Alu { };
    dut.cmd = Cmd { op: Op::Add, a: 1, en: true, in2: Inner { x: 0, y: 1 } };
    step(1);
    assert_eq(dut.res, Cmd { op: Op::Add, a: 2, en: true, in2: Inner { x: 0, y: 1 } });
}
";

/// `dut.res.a` okuma, `dut.cmd.en = …` alan yazma, `dut.cmd = Cmd { … }`
/// bütün yazma ve `assert_eq(dut.res, Cmd { … })` bütün karşılaştırma;
/// düşen iddianın raporu alan adlarını ve farklı alanı basar.
#[test]
fn struct_ports_in_the_test_language_simulate_with_field_names() {
    if !have_verilator() {
        eprintln!("atlandı: Verilator yok");
        return;
    }
    let dir = temp_dir("sim");
    std::fs::write(dir.join("alu.volt"), ALU).expect("tasarım");
    std::fs::write(dir.join("alu_test.volt"), ALU_TEST).expect("test");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .current_dir(&dir)
        .args([
            "--lang",
            "en",
            "test",
            "alu_test.volt",
            "--target-dir",
            "build",
        ])
        .output()
        .expect("volt");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("test whole_and_field ... ok"), "{stdout}");
    assert!(stdout.contains("test field_report ... FAILED"), "{stdout}");
    assert!(
        stdout.contains("left:  Cmd { op: Op::Add, a: 1, en: true, in2.x: 0, in2.y: 1 }"),
        "{stdout}"
    );
    assert!(stdout.contains("differs: a"), "{stdout}");
    let _ = std::fs::remove_dir_all(&dir);
}
