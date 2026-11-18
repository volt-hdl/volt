//! Tasarıma yazılmayan `match` kolu (#331, #332): HIR ile SV üretici aynı
//! kolu ölü sayar.
//!
//! Kural: ilk muhafızsız joker koldan sonraki kol ve önceki kolların
//! kapsadığı kol hiç seçilmez ve yazılmaz (sv-mapping.md §19.1, W2014);
//! kapsayıcı, `_`'sız enum match'inde son adlı kol geçersiz kodları da
//! alır (ADR-0074 Karar 4). Enum tipli `const` adı deseni değerinin
//! varyantını adlandırır (ADR-0085).
//!
//! Düzeltmeden önce kapsanan kolları HIR ve SV üretici ayrı ayrı
//! hesaplıyordu: HIR `START => …` desenini `S::B` sayıp sonraki `S::B`
//! koluna W2014 veriyor, SV üretici aynı deseni joker sayıp kolu yazıyordu.
//! Üretilen `case` iki `S_B` etiketi taşıyor ve `default`'u yoktu;
//! Verilator `CASEOVERLAP`, `CASEINCOMPLETE` ve `LATCH` uyarılarıyla
//! duruyordu (`volt test` düşüyordu).
//!
//! Beklenen değerler kuraldan elle yazıldı (her testin yorumunda).
//! Verilator yoksa simülasyon testi atlanır; `VOLT_REQUIRE_TOOLS=verilator`
//! ile (CI'ın araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

/// Dört biçim, aynı kollar: `START` (= `S::B`), kapsanan `S::B`, `S::A`,
/// `S::C`. Deyim biçimi `comb`'da ve `on`'da, ifade biçimi kök ve iç
/// konumda.
const DESIGN: &str = r#"enum S { A, B, C }
const START : S = S::B
module M {
    in  clk : clock
    in  s : S
    out stmt : u8
    out seq : u8
    out expr : u8
    out inner : u8
    reg r : u8 = 0
    comb {
        match s {
            START => { stmt = 1 }
            S::B => { stmt = 2 }
            S::A => { stmt = 3 }
            S::C => { stmt = 4 }
        }
    }
    on clk {
        match s {
            START => { r <= 1 }
            S::B => { r <= 2 }
            S::A => { r <= 3 }
            S::C => { r <= 4 }
        }
    }
    seq = r
    expr = match s { START => 1, S::B => 2, S::A => 3, S::C => 4 }
    inner = ~(match s { START => 1, S::B => 2, S::A => 3, S::C => 4 })
}
"#;

/// `seq` `on` bloğundaki register'ı okur: `step(1)` kenardan sonraki
/// değeri gösterir. `inner` = `~v` (u8): ~1 = 254, ~3 = 252, ~4 = 251.
const TESTS: &str = r#"
test "s = B: START names B and comes first" {
    let dut = M { };
    dut.s = S::B;
    step(1);
    // `START` is `S::B` and is the first arm: 1 in every form; the arm
    // `S::B` is never taken.
    assert_eq(dut.stmt, 1);
    assert_eq(dut.seq, 1);
    assert_eq(dut.expr, 1);
    assert_eq(dut.inner, 254);
}

test "s = A and C: their own arms" {
    let dut = M { };
    dut.s = S::A;
    step(1);
    assert_eq(dut.stmt, 3);
    assert_eq(dut.seq, 3);
    assert_eq(dut.expr, 3);
    assert_eq(dut.inner, 252);
    dut.s = S::C;
    step(1);
    assert_eq(dut.stmt, 4);
    assert_eq(dut.seq, 4);
    assert_eq(dut.expr, 4);
    assert_eq(dut.inner, 251);
}
"#;

fn temp_dir(tag: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("volt-match-dead-arm-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// Enum sabiti deseninin kapsadığı kol dört biçimde de yazılmaz; ilk
/// eşleşen kolun değeri gelir ve Verilator tasarımı uyarısız derler.
#[test]
fn enum_const_pattern_covers_its_variant_in_the_sv() {
    if tools::require(tools::Tool::Verilator).is_none() {
        return;
    }
    let dir = temp_dir("sim");
    std::fs::write(dir.join("case.volt"), format!("{DESIGN}{TESTS}")).expect("yaz");
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
    eprintln!("--- volt test (match_dead_arm_tests::sim)\n{text}");
    assert!(out.status.success(), "{text}");
    assert!(text.contains("2 passed; 0 failed"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `volt check --format json`: (kod, birincil konumun bayt başı) çiftleri.
fn check(tag: &str, src: &str) -> Vec<(String, u64)> {
    let dir = temp_dir(tag);
    std::fs::write(dir.join("case.volt"), src).expect("yaz");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .current_dir(&dir)
        .args(["--lang", "en", "check", "--format", "json", "case.volt"])
        .output()
        .expect("volt check");
    let _ = std::fs::remove_dir_all(&dir);
    let json: Value = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("JSON değil ({e}): {}", String::from_utf8_lossy(&out.stderr)));
    let mut found: Vec<(String, u64)> = json["diagnostics"]
        .as_array()
        .expect("diagnostics dizisi")
        .iter()
        .map(|d| {
            let start = d["spans"]
                .as_array()
                .and_then(|s| s.iter().find(|s| s["primary"] == true))
                .and_then(|s| s["start"]["byte"].as_u64())
                .expect("birincil konum");
            (d["code"].as_str().unwrap_or_default().to_string(), start)
        })
        .collect();
    found.sort();
    found
}

/// Dört biçimde kapsanan `S::B` kolu W2014 alır, desen konumunda; başka
/// tanı yoktur (bu, düzeltmeden önce de böyleydi: HIR kuralı doğru
/// uyguluyordu, SV üretici uygulamıyordu).
#[test]
fn the_covered_arm_is_w2014_in_every_form() {
    let mut expected: Vec<(String, u64)> = ["S::B => { stmt", "S::B => { r <=", "S::B => 2"]
        .iter()
        .flat_map(|n| DESIGN.match_indices(n).map(|(i, _)| i))
        .map(|b| ("W2014".to_string(), b as u64))
        .collect();
    expected.sort();
    assert_eq!(expected.len(), 4, "{expected:?}");
    assert_eq!(check("w2014", DESIGN), expected);
}

/// İki `on` bloğu: `r`'yi yalnız ilki sürer; ikincisinde `r`'ye yazan kol
/// tasarıma yazılmaz (jokerden sonra / kapsanan). Bir `always_ff`'in reset
/// dalı yalnız o bloğun yazdığı register'ları sıfırlar; ölü koldaki yazma
/// ikinci bloğa `r` için reset ataması eklememeli. Düzeltmeden önce eklenen atama `r`'yi iki `always_ff`'te
/// sürüyordu: Verilator `MULTIDRIVEN` ile duruyordu.
const RESET_DESIGN: &str = r#"module After {
    in  clk : clock
    in  s   : u2
    in  x   : u8
    out y   : u8
    reg r : u8 = 5
    reg q : u8 = 0
    on clk {
        r <= x
    }
    on clk {
        match s {
            1 => { q <= 1 }
            _ => { q <= 2 }
            2 => { r <= 0 }
        }
    }
    y = r + q
}

module Covered {
    in  clk : clock
    in  s   : u2
    in  x   : u8
    out y   : u8
    reg r : u8 = 5
    reg q : u8 = 0
    on clk {
        r <= x
    }
    on clk {
        match s {
            1 => { q <= 1 }
            1 => { r <= 0 }
            _ => { q <= 2 }
        }
    }
    y = r + q
}
"#;

/// Her kenarda `r <= x` (ilk blok) ve `q`, `s`'in canlı kolundan: s = 1
/// için 1, başka her değer için `_` kolunun 2'si. `y = r + q`.
const RESET_TESTS: &str = r#"
test "after the wildcard: r follows x" {
    let dut = After { };
    dut.s = 1;
    dut.x = 10;
    step(1);
    // r = 10, q = 1 (arm 1).
    assert_eq(dut.y, 11);
    dut.s = 2;
    step(1);
    // r = 10, q = 2: the wildcard comes before the arm 2.
    assert_eq(dut.y, 12);
}

test "covered: r follows x" {
    let dut = Covered { };
    dut.s = 1;
    dut.x = 10;
    step(1);
    // r = 10, q = 1: the first arm 1 is taken, the second never.
    assert_eq(dut.y, 11);
    dut.s = 2;
    step(1);
    // r = 10, q = 2 (`_`).
    assert_eq(dut.y, 12);
}
"#;

/// Tasarıma yazılmayan koldaki yazma reset dalına girmez; tasarım
/// Verilator'da uyarısız derlenir ve canlı kolların değerini verir.
#[test]
fn dead_arm_write_gets_no_reset_assignment() {
    if tools::require(tools::Tool::Verilator).is_none() {
        return;
    }
    let dir = temp_dir("reset");
    std::fs::write(
        dir.join("case.volt"),
        format!("{RESET_DESIGN}{RESET_TESTS}"),
    )
    .expect("yaz");
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
    eprintln!("--- volt test (match_dead_arm_tests::reset)\n{text}");
    assert!(out.status.success(), "{text}");
    assert!(text.contains("2 passed; 0 failed"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// İç konumdaki match ifadesi üçlü zincire iner; zincirin yüksekliği
/// yazılan kol sayısıdır (sv-mapping.md §19.1: 256'yı aşarsa E0018).
/// Jokerden sonraki 299 kol yazılmaz: zincir tek koldur, E0018 yok,
/// yalnız her ölü kolun W2014'ü. Düzeltmeden önce ölü kollar da sayılıyor,
/// sahte E0018 çıkıyordu.
#[test]
fn dead_arms_do_not_count_toward_the_chain_height() {
    let arms: Vec<String> = (1..300).map(|i| format!("{i} => {}", i % 7)).collect();
    let src = format!(
        "module H {{\n    in  s : u9\n    in  a : u8\n    out y : u8\n    y = a + match s {{ _ => 0, {} }}\n}}\n",
        arms.join(", ")
    );
    let found = check("height", &src);
    assert_eq!(found.len(), 299, "{found:?}");
    assert!(found.iter().all(|(c, _)| c == "W2014"), "{found:?}");
}

/// `@mmio`: veri yolunun sahip olduğu alana RTL yazamaz (E4006, ADR-0044).
/// Tasarıma yazılmayan koldaki yazma ikinci yazıcı değildir: E4006 yok,
/// yalnız kolun W2014'ü.
#[test]
fn bus_owned_field_written_only_in_a_dead_arm_is_not_e4006() {
    let src = r#"@mmio(base = 0x4000_0000, bus = AXI4Lite)
module Gpio {
    in  clk     : clock
    in  s       : u2
    in  pins_in : u8
    out pins_out : u8

    @reg(offset = 0x00, access = ReadWrite)
    output : {
        pins : u8,
        @reserved : bits<24>,
    }

    @reg(offset = 0x08, access = ReadOnly)
    input : {
        pins : u8,
        @reserved : bits<24>,
    }

    on clk {
        match s {
            1 => { }
            1 => { regs.input.pins <= pins_in }
            _ => { }
        }
    }
    pins_out = regs.output.pins
}
"#;
    let at = src.find("1 => { regs").expect("kol") as u64;
    assert_eq!(check("mmio", src), [("W2014".to_string(), at)]);
}
