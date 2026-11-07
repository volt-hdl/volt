//! Ayrıştırıcının ad eşleyen öteki adımları kapsam zincirinden geçer
//! (#136, #137, #138), davranış olarak, gerçek Verilator'da. Bundle
//! düzleştirmesi (ADR-0039, ADR-0050), çift yönlü port açılımı
//! (ADR-0051), `@mmio` tutamacı (ADR-0044), pipeline aşama değerleri
//! (ADR-0038) ve otomatik kontrat çıkarımı (ADR-0066) ad çözümünden (HIR)
//! önce koşar ve bir tabanı adıyla yeniden yazar ya da sabit sayar. Ad
//! yine de kullanıldığı yerin kapsam zincirinde çözülür
//! (name-resolution.md §2, §4, §6): blok `let`'i ya da döngü değişkeni
//! dıştaki portu, tutamacı, aşama değerini ya da kök const'u gölgeler,
//! kullanım iç bildirime bağlanır.
//!
//! Her test bir tasarımı `volt test` ile simüle eder; beklenen değerler
//! şartnameden elle hesaplandı (yorumlarda), derleyici çıktısından
//! alınmadı. Üretilen SV'nin metni karşılaştırılmaz: `volt test`
//! Verilator'ın WIDTH uyarılarında da durur, otomatik kontratlar
//! simülasyonda izlenir (ADR-0064, ADR-0066), yani geçen test lint temiz
//! ve kontrat ihlalsizdir de. Tanılar `parser_rewrite_scope_diag_tests`'te.
//!
//! Verilator yoksa testler atlanır; `VOLT_REQUIRE_TOOLS=verilator` ile
//! (CI'ın araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::PathBuf;
use std::process::Command;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-rwscope-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// `src`'yi (tasarım + test blokları) `volt test` ile koşar; `passed` test
/// geçmeli, hiçbiri düşmemeli. Çıktı (özet ve kapsam ölçüleri) döner;
/// Verilator yoksa `None`.
fn sim(tag: &str, src: &str, passed: usize) -> Option<String> {
    tools::require(tools::Tool::Verilator)?;
    let dir = temp_dir(tag);
    std::fs::write(dir.join("case.volt"), src).expect("yaz");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("VOLT_TOOL_BACKEND", "local")
        .env("VOLT_LANG", "en")
        .env("NO_COLOR", "1")
        .current_dir(&dir)
        .args(["test", "case.volt", "--target-dir", "build"])
        .output()
        .expect("volt test");
    let text = format!(
        "{}\n--- stderr\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    // CI günlüğünde testin Verilator'da koştuğu görünsün (atlanan test de
    // "ok" yazar).
    eprintln!("--- volt test (parser_rewrite_scope_sim_tests::{tag})\n{text}");
    assert!(out.status.success(), "{text}");
    assert!(
        text.contains(&format!("{passed} passed; 0 failed")),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
    Some(text)
}

// ═══ Bundle (ADR-0039, ADR-0050) ════════════════════════════════════════

/// #137: blok `let tx`'i `Handshake<u8>` çıkış portu `tx`'i gölgeler
/// (§2 blok kapsamı modülün içinde, §6). `tx.fired` yerel struct'ın
/// alanıdır (`s`), portun sanal alanı (`tx.valid && tx.ready`, ADR-0050)
/// değil. `r <= s` bir saat kenarında görünür: s = true → y = true,
/// s = false → y = false. Hatada `tx.valid` sabit false olduğundan y hep
/// false.
#[test]
fn a_block_let_shadows_the_virtual_field_of_a_handshake_port() {
    let src = r#"struct Flags {
    fired   : bool
    stalled : bool
}
module M {
    in  clk : clock
    out tx  : Handshake<u8>
    in  s   : bool
    out y   : bool
    reg r : bool = false
    tx.valid = false
    tx.data = 0
    on clk {
        let tx : Flags = Flags { fired: s, stalled: false }
        r <= tx.fired
    }
    y = r
}
test "the block let shadows the handshake port" {
    let dut = M { };
    dut.s = true;
    step(1);
    assert_eq(dut.y, true);
    dut.s = false;
    step(1);
    assert_eq(dut.y, false);
}
"#;
    sim("virtual", src, 1);
}

/// Bekçi: blok `let aw`'si (struct) bundle portu `aw`'yi gölgeler; `aw.addr`
/// yerelin alanıdır (`s`). r <= s: s = 6 → y = 6, s = 200 → y = 200.
/// Düzeltmeden önce de değer doğruydu, ama yanlış nedenle: ayrıştırıcı
/// `aw.addr`'ı porta (`aw_addr`) yazıyor, üretici yerelin düzleşmiş
/// alanını (`aw_addr`) portun önüne koyuyordu (#154, ayrı kök neden).
#[test]
fn a_block_struct_let_shadows_a_bundle_port() {
    let src = r#"struct port Req {
    out addr  : u8
    in  ack   : bool
}
struct Pair {
    addr : u8
    ack  : bool
}
module M {
    in  clk : clock
    in  aw  : Req
    in  s   : u8
    out y   : u8
    reg r : u8 = 0
    aw.ack = true
    on clk {
        let aw : Pair = Pair { addr: s, ack: false }
        r <= aw.addr
    }
    y = r
}
test "the struct let shadows the bundle port" {
    let dut = M { };
    dut.s = 6;
    step(1);
    assert_eq(dut.y, 6);
    dut.s = 200;
    step(1);
    assert_eq(dut.y, 200);
}
"#;
    sim("bundle_struct", src, 1);
}

// ═══ Çift yönlü port (ADR-0051) ═════════════════════════════════════════

/// #137: blok `let dq`'su `inout dq` portunu gölgeler; `dq.released`
/// yerel struct'ın alanıdır (`s`), sürücü register'ı (`!dq_oe`) değil.
/// `dq.drive(w)` `let`'ten önce olduğundan portu sürer (§4). s = false:
/// iki kenarda da y = false; s = true → y = true. Hatada ilk kenarda
/// `dq_oe` henüz 0 olduğundan y = !0 = true.
#[test]
fn a_block_let_shadows_the_virtual_field_of_an_inout_port() {
    let src = r#"struct St {
    released : bool
    driving  : bool
}
module M {
    in  clk : clock
    in  s   : bool
    in  w   : bits<8>
    inout dq : bits<8>
    out y   : bool
    reg r : bool = false
    on clk {
        dq.drive(w)
        let dq : St = St { released: s, driving: false }
        r <= dq.released
    }
    y = r
}
test "the block let shadows the inout port" {
    let dut = M { };
    dut.s = false;
    step(1);
    assert_eq(dut.y, false);
    step(1);
    assert_eq(dut.y, false);
    dut.s = true;
    step(1);
    assert_eq(dut.y, true);
}
"#;
    sim("bidir", src, 1);
}

// ═══ @mmio tutamacı (ADR-0044) ══════════════════════════════════════════

/// #137: `regs` her `@mmio` modülünün örtük tutamacıdır (modül kapsamı);
/// blok `let regs`'i onu gölgeler.
///
/// * `on` bloğunda: `r <= regs.control.enable` yerelin alanını (`a`) okur.
/// * `comb`'da `if`'ten önce `ec` tutamacı okur: bus hiç yazmadığından
///   `control` sıfırlamadaki 0'dır; `if c` dalındaki `let regs`'ten sonra
///   `ec` yereli (`a`) okur. Gölgeleme yalnız o dalda (§2, §4).
///
/// a = 1, c = 1: en = 1, ec = 1. c = 0: ec = 0 (tutamaç). a = 0, c = 1:
/// en = 0, ec = 0. Hatada en her zaman `control[0]` = 0.
#[test]
fn a_block_let_shadows_the_register_handle_only_in_its_block() {
    let src = r#"struct Ctl {
    enable : bool
    other  : bool
}
struct Regs {
    control : Ctl
    pad     : bool
}
@mmio(base = 0x4000_0000, bus = AXI4Lite)
module G {
    in  clk : clock
    in  a   : bool
    in  c   : bool
    out en  : bool
    out ec  : bool
    @reg(offset = 0x00, access = ReadWrite)
    control : {
        enable : bool,
        @reserved : bits<31>,
    }
    reg r : bool = false
    on clk {
        let regs : Regs = Regs { control: Ctl { enable: a, other: false }, pad: false }
        r <= regs.control.enable
    }
    en = r
    comb {
        ec = regs.control.enable
        if c {
            let regs : Regs = Regs { control: Ctl { enable: a, other: false }, pad: false }
            ec = regs.control.enable
        }
    }
}
test "a block let shadows the register handle" {
    let dut = G { };
    dut.a = true;
    dut.c = true;
    step(1);
    assert_eq(dut.en, true);
    assert_eq(dut.ec, true);
    dut.c = false;
    step(1);
    assert_eq(dut.ec, false);
    dut.a = false;
    dut.c = true;
    step(1);
    assert_eq(dut.en, false);
    assert_eq(dut.ec, false);
}
"#;
    sim("mmio", src, 1);
}

// ═══ Pipeline aşama değerleri (ADR-0038) ════════════════════════════════

/// Aşama A'nın değeri `a` (x + 1), B'de bir sınır register'ından okunur.
/// Testlerin ortak başı; `{B}` B aşamasının gövdesi, `{M}` modül düzeyi.
fn pipeline(stage_b: &str, module: &str, test: &str) -> String {
    format!(
        r#"pipeline(2) P {{
    in  clk : clock
    in  x   : u32
    in  c   : bool
    in  s   : u2
    out y0  : u32
    out y1  : u32
    out z   : u32
    reg r  : [u32; 2] = [0; 2]
    stage A {{
        let a : u32 = x + 1
    }}
    stage B {{
{stage_b}
    }}
{module}
    y0 = r[0]
    y1 = r[1]
}}
test "inner names shadow the stage value" {{
    let dut = P {{ }};
{test}
}}
"#
    )
}

/// #137: B aşamasındaki `for a in 0..2` aşama değeri `a`'yı gölgeler; `r[a]`
/// döngü değişkeniyle indekslenir: bir kenarda r = [x, x] = [10, 10].
/// Hatada iki yazma da `r[a_a_r]`'ye (aşama değerinin sınır register'ı)
/// gider; geri alma koşusunda y0 = 0.
#[test]
fn a_loop_variable_in_a_stage_shadows_the_stage_value() {
    let src = pipeline(
        "        for a in 0..2 {
            r[a] <= x
        }",
        "    z = 0",
        "    dut.x = 10;
    step(1);
    assert_eq(dut.y0, 10);
    assert_eq(dut.y1, 10);",
    );
    sim("pipe_loop", &src, 1);
}

/// #137: B'deki `if c { let a = 5 }` dalında `a` yereldir (5); `else`
/// dalında aşama değeridir (B'nin görünümü: bir kenar önceki x + 1).
/// x = 10, c = 1: bir kenarda r[0] = 5. c = 0: sınır register'ı ilk
/// kenarda 11 olmuştu, ikinci kenarda r[0] = 11. Hatada c = 1'de de sınır
/// register'ı okunur: ilk kenarda sıfırlama değeri 0.
#[test]
fn a_block_let_in_a_stage_branch_shadows_the_stage_value() {
    let src = pipeline(
        "        if c {
            let a : u32 = 5
            r[0] <= a
        } else {
            r[0] <= a
        }",
        "    z = 0",
        "    dut.x = 10;
    dut.c = true;
    step(1);
    assert_eq(dut.y0, 5);
    dut.c = false;
    step(1);
    assert_eq(dut.y0, 11);",
    );
    sim("pipe_if", &src, 1);
}

/// #137: `match` kolunun bloğundaki `let a` aşama değerini gölgeler; öteki
/// kolda `a` aşama değeridir. x = 10, s = 0: r[1] = 9; s = 1: r[1] = 11.
/// Hatada s = 0'da da sınır register'ı okunur: ilk kenarda 0.
#[test]
fn a_block_let_in_a_match_arm_of_a_stage_shadows_the_stage_value() {
    let src = pipeline(
        "        match s {
            0 => {
                let a : u32 = 9
                r[1] <= a
            }
            _ => {
                r[1] <= a
            }
        }",
        "    z = 0",
        "    dut.x = 10;
    dut.s = 0;
    step(1);
    assert_eq(dut.y1, 9);
    dut.s = 1;
    step(1);
    assert_eq(dut.y1, 11);",
    );
    sim("pipe_match", &src, 1);
}

/// #137: modül düzeyi `comb`'daki `let a` aşama değerini gölgeler; `z = a`
/// yereli okur (x + 7 = 17). Aşama değerine modül düzeyinde düz adla
/// erişim E5012'dir, ama burada `a` aşama değeri değil. Hatada E5012
/// (yanlış red) ve derleme düşer.
#[test]
fn a_block_let_at_module_level_shadows_the_stage_value() {
    let src = pipeline(
        "        r[0] <= a",
        "    comb {
        let a : u32 = x + 7
        z = a
    }",
        "    dut.x = 10;
    step(1);
    assert_eq(dut.z, 17);
    step(1);
    assert_eq(dut.y0, 11);",
    );
    sim("pipe_comb", &src, 1);
}

// ═══ Fonksiyon kapsamları (bekçi) ═══════════════════════════════════════

/// Fonksiyonun parametresi ve `let`'i kendi kapsamındadır (§2, ADR-0081);
/// modülün bundle portu, çift yönlü portu, `regs` tutamacı ve aşama değeri
/// fonksiyon gövdesine sızmaz. Yeniden yazan adımlar fonksiyon gövdesine
/// girmez; düzeltmeden önce de geçer.
///
/// * M1: pick(s) = s = 6; mk(s) = s ^ 1 = 7.
/// * M2: r <= drv(s): s = 1 → y = 1.
/// * G: en = f(a) = a = 1; e2 = g(a) = !a = 0; e3 = tutamaç = 0.
/// * P: B'de `a` = bir kenar önceki x + 1 = 11; r <= f(a) = 13,
///   q <= g(a) = 14 (ikinci kenarda).
#[test]
fn function_parameters_and_lets_are_not_rewritten() {
    let src = r#"struct port Req {
    out addr  : u8
    in  ack   : bool
}
struct Pair {
    addr : u8
    ack  : bool
}
struct St {
    released : bool
    driving  : bool
}
struct Ctl {
    enable : bool
    other  : bool
}
struct Regs {
    control : Ctl
    pad     : bool
}
fn pick(aw: Pair) -> u8 { aw.addr }
fn mk(s: u8) -> u8 {
    let aw = Pair { addr: s ^ 1, ack: false }
    aw.addr
}
fn drv(dq: St) -> bool { dq.driving }
fn f(regs: Regs) -> bool { regs.control.enable }
fn g(a: bool) -> bool {
    let regs = Regs { control: Ctl { enable: !a, other: a }, pad: a }
    regs.control.enable
}
fn fa(a: u32) -> u32 { a + 2 }
fn ga(x: u32) -> u32 {
    let a = x + 3
    a
}
module M1 {
    in  aw  : Req
    in  s   : u8
    out y   : u8
    out z   : u8
    aw.ack = true
    y = pick(Pair { addr: s, ack: false })
    z = mk(s)
}
module M2 {
    in  clk : clock
    in  s   : bool
    in  w   : bits<8>
    inout dq : bits<8>
    out y   : bool
    reg r : bool = false
    on clk {
        dq.drive(w)
        r <= drv(St { released: false, driving: s })
    }
    y = r
}
@mmio(base = 0x4000_0000, bus = AXI4Lite)
module G {
    in  clk : clock
    in  a   : bool
    out en  : bool
    out e2  : bool
    out e3  : bool
    @reg(offset = 0x00, access = ReadWrite)
    control : {
        enable : bool,
        @reserved : bits<31>,
    }
    en = f(Regs { control: Ctl { enable: a, other: a }, pad: a })
    e2 = g(a)
    e3 = regs.control.enable
}
pipeline(2) P {
    in  clk : clock
    in  x   : u32
    out y   : u32
    out z   : u32
    reg r : u32 = 0
    reg q : u32 = 0
    stage A {
        let a : u32 = x + 1
    }
    stage B {
        r <= fa(a)
        q <= ga(a)
    }
    y = r
    z = q
}
test "bundle" {
    let dut = M1 { };
    dut.s = 6;
    step(1);
    assert_eq(dut.y, 6);
    assert_eq(dut.z, 7);
}
test "bidir" {
    let dut = M2 { };
    dut.s = true;
    step(1);
    assert_eq(dut.y, true);
}
test "mmio" {
    let dut = G { };
    dut.a = true;
    step(1);
    assert_eq(dut.en, true);
    assert_eq(dut.e2, false);
    assert_eq(dut.e3, false);
}
test "pipeline" {
    let dut = P { };
    dut.x = 10;
    step(2);
    assert_eq(dut.y, 13);
    assert_eq(dut.z, 14);
}
"#;
    sim("fn_scopes", src, 4);
}

// ═══ Otomatik kontratlar (ADR-0066) ═════════════════════════════════════

/// #138: sınır `r == K`'deki `K` blok `let`'idir (`lim`), kök const (5)
/// değil: sayaç kontratı çıkarılmaz (adı taze olan aynı girdi gibi).
/// lim = 10: sekiz kenarda r = 8; üç kenar daha: 9, 10, sonra r == 10'da
/// 0. Hatada çıkarılan `invariant r <= 5` r = 6'da (7. çevrim) ihlal
/// edilir ve test düşer.
#[test]
fn a_counter_bound_named_by_a_block_let_gets_no_contract() {
    let src = r#"const K : u8 = 5
module M {
    in  clk : clock
    in  lim : u8
    out q   : u8
    reg r : u8 = 0
    on clk {
        let K : u8 = lim
        if r == K {
            r <= 0
        } else {
            r <= r + 1
        }
    }
    q = r
}
test "the bound is the block let" {
    let dut = M { };
    dut.lim = 10;
    step(8);
    assert_eq(dut.q, 8);
    step(3);
    assert_eq(dut.q, 0);
}
"#;
    let Some(out) = sim("cnt_bound", src, 1) else {
        return;
    };
    assert!(!out.contains("auto counter"), "{out}");
}

/// #138: sabit yazma `r <= K`'deki `K` blok `let`'idir (`v`): r her değeri
/// alabilir, kontrat çıkarılmaz. v = 50, load: bir kenarda r = 50; load
/// kalkınca 50 ≠ 9, r = 51. Hatada çıkarılan `invariant r <= 9` ilk
/// yüklemede ihlal edilir.
#[test]
fn a_counter_written_from_a_block_let_gets_no_contract() {
    let src = r#"const K : u8 = 0
module M {
    in  clk  : clock
    in  load : bool
    in  v    : u8
    out q    : u8
    reg r : u8 = 0
    on clk {
        let K : u8 = v
        if load {
            r <= K
        } else if r == 9 {
            r <= 0
        } else {
            r <= r + 1
        }
    }
    q = r
}
test "the loaded value is the block let" {
    let dut = M { };
    dut.v = 50;
    dut.load = true;
    step(1);
    assert_eq(dut.q, 50);
    dut.load = false;
    step(1);
    assert_eq(dut.q, 51);
}
"#;
    let Some(out) = sim("cnt_write", src, 1) else {
        return;
    };
    assert!(!out.contains("auto counter"), "{out}");
}

/// #138 (`fsm.rs` aynı süzgeç): `st <= S`'deki `S` blok `let`'idir (`v`);
/// durum register'ı sabit yazılmaz, FSM kontratı (geçiş kapsam ölçüleri)
/// çıkarılmaz. Hatada `S` kök const (3) sanılır ve 1 → 3 geçişinin
/// kapsam ölçüsü izlenir; tasarım o geçişi yapmaz (v = 2). v = 2: durum
/// 0 → 1 → 2 → 0.
#[test]
fn a_state_written_from_a_block_let_gets_no_fsm_contract() {
    let src = r#"const S : u2 = 3
module M {
    in  clk : clock
    in  v   : u2
    out q   : u2
    reg st : u2 = 0
    on clk {
        let S : u2 = v
        match st {
            0 => { st <= 1 }
            1 => { st <= S }
            _ => { st <= 0 }
        }
    }
    q = st
}
test "the written state is the block let" {
    let dut = M { };
    dut.v = 2;
    step(1);
    assert_eq(dut.q, 1);
    step(1);
    assert_eq(dut.q, 2);
    step(1);
    assert_eq(dut.q, 0);
}
"#;
    let Some(out) = sim("fsm_write", src, 1) else {
        return;
    };
    assert!(!out.contains("auto FSM"), "{out}");
}

/// Bekçi: koşul `r != K` `if`'in bulunduğu yerde çözülür; `K` orada kök
/// const'tur (5). Dalın içindeki `let K` koşuldan sonradır ve onu
/// etkilemez (§4): sayaç kontratı (`r <= 5` ve sarma kapsam ölçüsü)
/// çıkarılır ve tutar. r: beş kenarda 5, altıncıda 0; sarma bir kez
/// görülür.
#[test]
fn a_block_let_after_the_condition_keeps_the_counter_contract() {
    let src = r#"const K : u8 = 5
module M {
    in  clk : clock
    in  lim : u8
    out q   : u8
    reg r : u8 = 0
    on clk {
        if r != K {
            let K : u8 = lim
            r <= r + 1
        } else {
            r <= 0
        }
    }
    q = r
}
test "the contract holds" {
    let dut = M { };
    dut.lim = 200;
    step(5);
    assert_eq(dut.q, 5);
    step(1);
    assert_eq(dut.q, 0);
}
"#;
    let Some(out) = sim("cnt_cond", src, 1) else {
        return;
    };
    let wrap = out
        .lines()
        .find(|l| l.contains("auto counter wrap"))
        .unwrap_or_else(|| panic!("sarma kapsam ölçüsü yok\n{out}"));
    assert!(wrap.ends_with("hit 1 time"), "{out}");
}

/// Bekçiler: çıkarımın değişmediği üç hücre.
///
/// * C1: sayaç yazması `for K in 5..6` içinde; döngü içi yazma tanıyıcının
///   dışındadır (ADR-0066), kontrat yok. Döngü değişkeni K = 5: r beş
///   kenarda 5, altıncıda 0.
/// * F1: durum register'ı `st` sabitlerle yazılır, FSM kontratı (geçiş
///   kapsam ölçüleri) çıkarılır; `comb`'daki `let st` register'ı gölgeler ve
///   onun `match`'i register üzerinde değildir (o = x == 3). Durum 0 → 1 →
///   2 → 0.
/// * C2: modül `let K`'si `on` bloğundan SONRA bildirilir; blokta `K` kök
///   const'tur (5, §4, §5), sayaç beşte sarar. Kontrat çıkarılmaz: modül
///   kapsamında yaşayan kontratta `K` o `let`'e bağlanırdı.
#[test]
fn inferred_contracts_that_do_not_change() {
    let src = r#"const K : u8 = 5
module C1 {
    in  clk : clock
    out q   : u8
    reg r : u8 = 0
    on clk {
        for K in 5..6 {
            if r == K {
                r <= 0
            } else {
                r <= r + 1
            }
        }
    }
    q = r
}
module F1 {
    in  clk : clock
    in  x   : u2
    out q   : u2
    out o   : bool
    reg st : u2 = 0
    on clk {
        match st {
            0 => { st <= 1 }
            1 => { st <= 2 }
            _ => { st <= 0 }
        }
    }
    comb {
        let st : u2 = x
        match st {
            3 => { o = true }
            _ => { o = false }
        }
    }
    q = st
}
module C2 {
    in  clk : clock
    in  lim : u8
    out q   : u8
    reg r : u8 = 0
    on clk {
        if r == K {
            r <= 0
        } else {
            r <= r + 1
        }
    }
    let K : u8 = lim
    q = r
}
test "loop" {
    let dut = C1 { };
    step(5);
    assert_eq(dut.q, 5);
    step(1);
    assert_eq(dut.q, 0);
}
test "fsm" {
    let dut = F1 { };
    dut.x = 3;
    step(1);
    assert_eq(dut.q, 1);
    assert_eq(dut.o, true);
    dut.x = 2;
    step(1);
    assert_eq(dut.q, 2);
    assert_eq(dut.o, false);
    step(1);
    assert_eq(dut.q, 0);
}
test "later" {
    let dut = C2 { };
    dut.lim = 9;
    step(5);
    assert_eq(dut.q, 5);
    step(1);
    assert_eq(dut.q, 0);
}
"#;
    let Some(out) = sim("contracts_unchanged", src, 3) else {
        return;
    };
    assert!(
        out.contains("F1.cov_0") && out.contains("auto FSM transition"),
        "{out}"
    );
    assert!(!out.contains("auto counter"), "{out}");
}

/// Bekçi: pipeline'ın `stall` muhafızı modül düzeyi `let stall_decode`
/// olarak üretilir; `comb`'daki aynı adlı `let` onu gölgeler (çözücü W1002
/// verir) ve `stalled` yereli (q) okur. Muhafızın üretilmesi davranışı
/// değiştirmez.
#[test]
fn a_block_let_named_like_a_stall_guard_is_the_local() {
    let src = r#"pipeline(3) Basic {
    in  clk : clock
    in  x : u32
    in  q : bool
    out y : u32
    out stalled : bool
    stage Fetch {
        let a : u32 = x + 1
    }
    stage Decode {
        let b : u32 = a + 2
        stall when stage(Execute).c == 0
    }
    stage Execute {
        let c : u32 = b + stage(Fetch).a
    }
    y = stage(Execute).c
    comb {
        let stall_decode : bool = q
        stalled = stall_decode
    }
}
test "the local is read" {
    let dut = Basic { };
    dut.x = 1;
    dut.q = true;
    step(1);
    assert_eq(dut.stalled, true);
    dut.q = false;
    step(1);
    assert_eq(dut.stalled, false);
}
"#;
    sim("stall_guard", src, 1);
}

// ═══ Hedefli mutasyon koşusunun kör noktaları (ADR-0109 eki) ═══════════
//
// Değişen fonksiyonların önceki mantığı da koşuya girdi; missed'lerden
// davranışı değiştirenler (B) aşağıda. Her mutant elle uygulandı, girdi
// mutantlı derleyicide farklı davrandı (test düşer), mutant geri alındı.

/// Sınır ifadesi `K >> 0`: ikili işlemde bir yan bile yereldeyse ifade
/// sabit değildir (`is_const_expr`'ın `&&`'i; mutant `||` kök K'yı, 5'i,
/// okuyup `invariant r <= 5` üretir). Sayaç lim'e (10) kadar sayar: sekiz
/// kenarda 8, üç kenar daha: 9, 10, 0.
#[test]
fn a_binary_bound_with_one_local_operand_is_not_constant() {
    let src = r#"const K : u8 = 5
module M {
    in  clk : clock
    in  lim : u8
    out q   : u8
    reg r : u8 = 0
    on clk {
        let K : u8 = lim
        if r == K >> 0 {
            r <= 0
        } else {
            r <= r + 1
        }
    }
    q = r
}
test "the bound is the block let shifted by zero" {
    let dut = M { };
    dut.lim = 10;
    step(8);
    assert_eq(dut.q, 8);
    step(3);
    assert_eq(dut.q, 0);
}
"#;
    let Some(out) = sim("bound_shift", src, 1) else {
        return;
    };
    assert!(!out.contains("auto counter"), "{out}");
}

/// `-(-5)` sabittir (literal aritmetiği, #144): sayaç sözleşmesi çıkarılır.
/// Mutant (`is_const_expr`'ın eksi kolu silinir) sözleşmeyi düşürür. r beş
/// kenarda 5, altıncıda 0; sarma kapsam ölçüsü bir kez görülür.
#[test]
fn a_negated_constant_bound_keeps_the_counter_contract() {
    let src = r#"module M {
    in  clk : clock
    out q   : u8
    reg r : u8 = 0
    on clk {
        if r == -(-5) {
            r <= 0
        } else {
            r <= r + 1
        }
    }
    q = r
}
test "the negated bound is a constant" {
    let dut = M { };
    step(5);
    assert_eq(dut.q, 5);
    step(1);
    assert_eq(dut.q, 0);
}
"#;
    let Some(out) = sim("bound_neg", src, 1) else {
        return;
    };
    let wrap = out
        .lines()
        .find(|l| l.contains("auto counter wrap"))
        .unwrap_or_else(|| {
            panic!(
                "sarma kapsam ölçüsü yok
{out}"
            )
        });
    assert!(wrap.ends_with("hit 1 time"), "{out}");
}

/// Aynı sınırla korunan iki artış tek sayaçtır (`recognize`'ın `c.bound ==
/// v` kolu; mutant `false` sözleşmeyi düşürür). en = 1: beş kenarda 5;
/// en = 0: dört kenarda 9, bir kenarda 0.
#[test]
fn two_increments_under_the_same_bound_keep_the_counter_contract() {
    let src = r#"module M {
    in  clk : clock
    in  en  : bool
    out q   : u8
    reg r : u8 = 0
    on clk {
        if en {
            if r == 9 {
                r <= 0
            } else {
                r <= r + 1
            }
        } else {
            if r == 9 {
                r <= 0
            } else {
                r <= r + 1
            }
        }
    }
    q = r
}
test "two increments under the same bound" {
    let dut = M { };
    dut.en = true;
    step(5);
    assert_eq(dut.q, 5);
    dut.en = false;
    step(4);
    assert_eq(dut.q, 9);
    step(1);
    assert_eq(dut.q, 0);
}
"#;
    let Some(out) = sim("bound_twice", src, 1) else {
        return;
    };
    assert!(out.contains("auto counter wrap"), "{out}");
}

/// Koşul `x == 5` sayacı değil başka bir sinyali karşılaştırır: sınır
/// değildir (`bound_of`'un `&&`'i; mutant `||` 5'i sınır sayıp `invariant
/// r <= 5` üretir ve r = 6'da ihlal edilir). x = 0: sekiz kenarda 8;
/// x = 5: 0.
#[test]
fn a_guard_on_another_signal_is_not_a_counter_bound() {
    let src = r#"module M {
    in  clk : clock
    in  x   : u8
    out q   : u8
    reg r : u8 = 0
    on clk {
        if x == 5 {
            r <= 0
        } else {
            r <= r + 1
        }
    }
    q = r
}
test "the guard compares another signal" {
    let dut = M { };
    dut.x = 0;
    step(8);
    assert_eq(dut.q, 8);
    dut.x = 5;
    step(1);
    assert_eq(dut.q, 0);
}
"#;
    let Some(out) = sim("bound_other", src, 1) else {
        return;
    };
    assert!(!out.contains("auto counter"), "{out}");
}

/// Register üzerinde `match` yoksa FSM değildir (`recognize`'ın `&&`'i;
/// mutant `||` başka bir sinyalin `match`'ini alıp durum kapsam ölçüleri
/// üretir). c = 1: st = 1; c = 0: st = 2. `o = s == 0`.
#[test]
fn a_register_without_a_match_on_it_is_not_an_fsm() {
    let src = r#"module M {
    in  clk : clock
    in  c   : bool
    in  s   : u2
    out q   : u2
    out o   : bool
    reg st : u2 = 0
    on clk {
        if c {
            st <= 1
        } else {
            st <= 2
        }
    }
    comb {
        match s {
            0 => { o = true }
            _ => { o = false }
        }
    }
    q = st
}
test "no match on the register" {
    let dut = M { };
    dut.c = true;
    dut.s = 0;
    step(1);
    assert_eq(dut.q, 1);
    assert_eq(dut.o, true);
    dut.c = false;
    dut.s = 1;
    step(1);
    assert_eq(dut.q, 2);
    assert_eq(dut.o, false);
}
"#;
    let Some(out) = sim("no_fsm_match", src, 1) else {
        return;
    };
    assert!(!out.contains("auto FSM"), "{out}");
}

/// `@no_protocol_check` bundle dizisi portunda da protokol sözleşmesini
/// kapatır (ADR-0050; `flatten_bundles`'ın `||`'i; mutant `&&` öğe
/// düzeyinde nitelik yoksa varsayımları üretir). Uyarıcı `ready` = 0
/// iken `valid`'i düşürür: protokole aykırı, ama denetim kapalı; `y` veri
/// girişi 7'dir. Nitelik olmadan aynı test "assumption violated by test
/// stimulus" ile düşer.
#[test]
fn no_protocol_check_on_a_bundle_array_port_drops_its_assumptions() {
    let src = r#"module M {
    in  clk : clock
    @no_protocol_check
    in  rx : [Handshake<u8>; 2]
    out y  : u8
    rx[0].ready = false
    rx[1].ready = false
    y = rx[0].data
}
test "an unchecked array port" {
    let dut = M { };
    dut.rx_0_valid = true;
    dut.rx_0_data = 7;
    step(1);
    dut.rx_0_valid = false;
    step(1);
    assert_eq(dut.y, 7);
}
"#;
    sim("array_unchecked", src, 1);
}
