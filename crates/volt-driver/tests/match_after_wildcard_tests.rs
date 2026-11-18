//! `match`'te ilk joker koldan sonraki kollar (#216): davranış olarak,
//! gerçek Verilator'da (`volt test`), ve W2014 olarak (`volt check`).
//!
//! Kural: ilk eşleşen kol kazanır (ADR-0083 Karar 5 ve const-eval.md:
//! "ilk eşleşen kol"; type-inference.md §3.9: match ifadesi çok kollu `if`
//! ifadesidir). Deyim biçimi ifade biçimiyle aynı kuralları izler
//! (grammar-full.ebnf `MatchExpr`); kök konumdaki match ifadesi deyim
//! biçimine iner, hedef her kolda tekrarlanır (sv-mapping.md §19.1), yani
//! iki biçim aynı donanımdır. İlk joker koldan sonraki kollar yazılmaz ve
//! W2014 alır (sv-mapping.md §19.1).
//!
//! Düzeltmeden önce deyim biçimi `_`'yı yerinde `default` yazıyor, sonraki
//! kolları da `case`'e ekliyordu. SystemVerilog `default`'u yalnız başka
//! hiçbir etiket eşleşmediğinde seçer, nerede yazılmış olursa olsun: s = 1
//! için deyim biçimi 9, ifade biçimi ve `const` 7 veriyordu; hiçbir biçimde
//! W2014 yoktu.
//!
//! Beklenen değerler kuraldan elle yazıldı (her testin yorumunda),
//! derleyicinin ya da simülatörün çıktısından alınmadı. Üretilen SV'nin
//! metni karşılaştırılmaz.
//!
//! Verilator yoksa simülasyon testi atlanır; `VOLT_REQUIRE_TOOLS=verilator`
//! ile (CI'ın araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

/// Yedi biçim, aynı kollar: `0 => 1, _ => 7, 1 => 9` (ya da enum'da
/// `E::A => 1, _ => 7, E::B => 9`). Deyim biçimi `comb`'da (sayısal ve
/// enum) ve `on`'da (register), ifade biçimi kök konumda (sayısal ve enum)
/// ve iç konumda (`~` işleneni, üçlü zincir), bir de `const`.
const DESIGN: &str = r#"const K : u8 = match 1 { 0 => 1, _ => 7, 1 => 9 }
enum E { A, B, C }
module M {
    in  clk : clock
    in  s : u2
    in  e : E
    in  a : u8
    out stmt : u8
    out estmt : u8
    out seq : u8
    out expr : u8
    out eexpr : u8
    out inner : u8
    out k : u8
    reg r : u8 = 0
    comb {
        match s {
            0 => { stmt = 1 }
            _ => { stmt = 7 }
            1 => { stmt = 9 }
        }
        match e {
            E::A => { estmt = 1 }
            _ => { estmt = 7 }
            E::B => { estmt = 9 }
        }
    }
    on clk {
        match s {
            0 => { r <= 1 }
            _ => { r <= 7 }
            1 => { r <= 9 }
        }
    }
    seq = r
    expr = match s { 0 => 1, _ => 7, 1 => 9 }
    eexpr = match e { E::A => 1, _ => 7, E::B => 9 }
    inner = ~(match s { 0 => a, _ => 7, 1 => 9 })
    k = K
}
"#;

/// `seq`, `on` bloğundaki register'ı okur: `step(1)` kenardan sonraki
/// değeri gösterir. `inner` = `~v` (u8): ~1 = 254, ~7 = 248.
const TESTS: &str = r#"
test "s = 0: the arm 0 is the first that matches" {
    let dut = M { };
    dut.s = 0;
    dut.e = E::A;
    dut.a = 1;
    step(1);
    // `0` / `E::A` is the first arm and matches: 1 in every form.
    assert_eq(dut.stmt, 1);
    assert_eq(dut.estmt, 1);
    assert_eq(dut.seq, 1);
    assert_eq(dut.expr, 1);
    assert_eq(dut.eexpr, 1);
    assert_eq(dut.inner, 254);
    // match 1: `0` does not match, `_` comes before `1`: 7.
    assert_eq(dut.k, 7);
}

test "s = 1: the wildcard comes before the arm 1 and wins" {
    let dut = M { };
    dut.s = 1;
    dut.e = E::B;
    dut.a = 1;
    step(1);
    // `0` / `E::A` does not match; `_` matches every value and comes
    // before `1` / `E::B`: 7 in every form (the bug gave 9 in the
    // statement forms).
    assert_eq(dut.stmt, 7);
    assert_eq(dut.estmt, 7);
    assert_eq(dut.seq, 7);
    assert_eq(dut.expr, 7);
    assert_eq(dut.eexpr, 7);
    assert_eq(dut.inner, 248);
    assert_eq(dut.k, 7);
}

test "s = 2 and 3, e = C: only the wildcard matches" {
    let dut = M { };
    dut.s = 2;
    dut.e = E::C;
    dut.a = 1;
    step(1);
    assert_eq(dut.stmt, 7);
    assert_eq(dut.estmt, 7);
    assert_eq(dut.seq, 7);
    assert_eq(dut.expr, 7);
    assert_eq(dut.eexpr, 7);
    assert_eq(dut.inner, 248);
    dut.s = 3;
    step(1);
    assert_eq(dut.stmt, 7);
    assert_eq(dut.seq, 7);
    assert_eq(dut.expr, 7);
    assert_eq(dut.inner, 248);
}
"#;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "volt-match-after-wildcard-{tag}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// #216: yedi biçim her girişte aynı değeri verir, ilk eşleşen kolun
/// değerini.
#[test]
fn issue_216_every_form_takes_the_first_matching_arm() {
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
    eprintln!("--- volt test (match_after_wildcard_tests::sim)\n{text}");
    assert!(out.status.success(), "{text}");
    assert!(text.contains("3 passed; 0 failed"), "{text}");
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

/// Her `needle`'ın her geçtiği yerin bayt başı + `offset`.
fn at(src: &str, needles: &[(&str, usize)]) -> Vec<(String, u64)> {
    let mut out: Vec<(String, u64)> = needles
        .iter()
        .flat_map(|&(n, off)| src.match_indices(n).map(move |(i, _)| i + off))
        .map(|b| ("W2014".to_string(), b as u64))
        .collect();
    out.sort();
    out
}

/// #216: jokerden sonraki kol yedi biçimin her birinde W2014 alır, desen
/// konumunda; başka tanı yoktur (sv-mapping.md §19.1).
#[test]
fn issue_216_the_arm_after_the_wildcard_is_w2014_in_every_form() {
    let expected = at(
        DESIGN,
        &[
            ("1 => { stmt = 9 }", 0),
            ("E::B => { estmt = 9 }", 0),
            ("1 => { r <= 9 }", 0),
            // `const`, kök ifade ve iç ifade: üç kez `_ => 7, 1 => 9`.
            ("_ => 7, 1 => 9", 8),
            ("_ => 7, E::B => 9", 8),
        ],
    );
    assert_eq!(expected.len(), 7, "{expected:?}");
    assert_eq!(check("w2014", DESIGN), expected);
}

/// Jokerden sonraki muhafızlı kol da hiç seçilmez: iki biçimde W2014 alır
/// ve yazılmadığı için muhafızın "henüz desteklenmiyor" tanısını (E0003)
/// almaz. Düzeltmeden önce ifade biçimi kolu sessizce atlıyor, deyim
/// biçimi E0003 veriyordu.
#[test]
fn a_guarded_arm_after_the_wildcard_is_w2014_in_both_forms() {
    let src = r#"module M {
    in  s : u2
    in  c : bool
    out x : u8
    out y : u8
    comb {
        match s {
            0 => { x = 1 }
            _ => { x = 7 }
            1 if c => { x = 9 }
        }
    }
    y = match s { 0 => 1, _ => 7, 1 if c => 9 }
}
"#;
    let expected = at(src, &[("1 if c => { x = 9 }", 0), ("1 if c => 9", 0)]);
    assert_eq!(expected.len(), 2, "{expected:?}");
    assert_eq!(check("guard", src), expected);
}
