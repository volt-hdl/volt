//! E2031'in iki önerisi (ADR-0106) uygulanınca iki farklı anlam verir,
//! uçtan uca, gerçek Verilator'da. `a == b + 1` (a, b: u3):
//! - sarma önerisi `a == (b + 1)[2:0] as u3`: b = 7, a = 0 için eşit
//!   (7 + 1 = 8, 3 bitte 0);
//! - taşmayı tutma önerisi `(a as u4) == b + 1`: aynı girişte eşit değil.
//!
//! Öneriler `volt check --format=json` çıktısından okunup uygulanır, yani
//! sınanan metin derleyicinin önerdiği metindir. Verilator yoksa test
//! atlanır; `VOLT_REQUIRE_TOOLS=verilator` ile yokluk testi düşürür.

mod tools;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const DESIGN: &str = "\
module Cmp {
    in  a  : u3
    in  b  : u3
    out eq : bool

    eq = a == b + 1
}
";

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-cmpw-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

fn volt(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("VOLT_TOOL_BACKEND", "local")
        .env("VOLT_LANG", "en")
        .env("NO_COLOR", "1")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("volt çalışmalı")
}

/// E2031 tanısının önerileri: her biri (başlangıç, bitiş, metin) düzenlemeleri.
fn suggestions(dir: &Path) -> Vec<Vec<(usize, usize, String)>> {
    let out = volt(dir, &["check", "--format=json", "cmp.volt"]);
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let env: Value = serde_json::Deserializer::from_str(&text)
        .into_iter::<Value>()
        .next()
        .expect("json")
        .expect("zarf");
    let diag = env["diagnostics"]
        .as_array()
        .expect("tanılar")
        .iter()
        .find(|d| d["code"] == "E2031")
        .cloned()
        .expect("E2031 beklenir");
    let edit = |e: &Value| {
        (
            e["span"]["start"]["byte"].as_u64().expect("start") as usize,
            e["span"]["end"]["byte"].as_u64().expect("end") as usize,
            e["replacement"].as_str().expect("replacement").to_string(),
        )
    };
    diag["suggestions"]
        .as_array()
        .expect("öneriler")
        .iter()
        .map(|s| {
            let mut all = vec![edit(s)];
            if let Some(more) = s["additional_edits"].as_array() {
                all.extend(more.iter().map(edit));
            }
            all
        })
        .collect()
}

fn apply(src: &str, edits: &[(usize, usize, String)]) -> String {
    let mut sorted = edits.to_vec();
    sorted.sort_by_key(|e| std::cmp::Reverse((e.0, e.1)));
    let mut out = src.to_string();
    for (start, end, text) in sorted {
        out.replace_range(start..end, &text);
    }
    out
}

fn test_file(expect_equal: bool) -> String {
    let check = if expect_equal {
        "assert_true(dut.eq);"
    } else {
        "assert_false(dut.eq);"
    };
    format!(
        "test \"b = 7, a = 0\" {{\n    let dut = Cmp {{ }};\n    dut.a = 0;\n    dut.b = 7;\n    step(1);\n    {check}\n}}\n\n\
         test \"b = 4, a = 5\" {{\n    let dut = Cmp {{ }};\n    dut.a = 5;\n    dut.b = 4;\n    step(1);\n    assert_true(dut.eq);\n}}\n"
    )
}

#[test]
fn the_two_suggestions_are_written_as_the_help_line_shows() {
    let dir = temp_dir("text");
    std::fs::write(dir.join("cmp.volt"), DESIGN).expect("yaz");
    let fixed: Vec<String> = suggestions(&dir).iter().map(|e| apply(DESIGN, e)).collect();
    assert_eq!(fixed.len(), 2, "iki öneri beklenir");
    assert!(
        fixed[0].contains("eq = a == (b + 1)[2:0] as u3"),
        "{}",
        fixed[0]
    );
    assert!(fixed[1].contains("eq = (a as u4) == b + 1"), "{}", fixed[1]);
}

#[test]
fn wrapping_and_keeping_the_carry_differ_for_b_7_a_0() {
    if tools::require(tools::Tool::Verilator).is_none() {
        return;
    }
    let dir = temp_dir("sim");
    std::fs::write(dir.join("cmp.volt"), DESIGN).expect("yaz");
    let fixes = suggestions(&dir);
    // Öneri sırası: sarma (eşit), taşmayı tutma (eşit değil).
    for (i, (edits, equal)) in fixes.iter().zip([true, false]).enumerate() {
        let case = dir.join(format!("case{i}"));
        std::fs::create_dir_all(&case).expect("dizin");
        std::fs::write(case.join("cmp.volt"), apply(DESIGN, edits)).expect("yaz");
        std::fs::write(case.join("cmp_test.volt"), test_file(equal)).expect("yaz");
        let out = volt(&case, &["test", "cmp_test.volt", "--target-dir", "build"]);
        let text = format!(
            "{}\n--- stderr\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(out.status.success(), "öneri {i}:\n{text}");
        assert!(text.contains("2 passed; 0 failed"), "öneri {i}:\n{text}");
    }
}

/// ADR-0106 §6: kendi genişlikli alt ifade daha geniş bağlamda Volt
/// anlamını korur. `a << 4` u8'dir (a = 255 → 240), `~a` u8'dir
/// (a = 0 → 255); SV boyut dönüşümü yalıtmasaydı 4080 ve 4095 olurdu.
const SELF_SIZED: &str = "module Wide {
    in  a : u8
    in  n : u3
    out s : u12
    out t : u12

    let ws : u12 = (a << n) + 0
    let wt : u12 = (~a) + 0
    s = ws
    t = wt
}
";

#[test]
fn shift_and_not_keep_their_own_width_in_simulation() {
    if tools::require(tools::Tool::Verilator).is_none() {
        return;
    }
    let dir = temp_dir("selfsized");
    std::fs::write(dir.join("wide.volt"), SELF_SIZED).expect("yaz");
    std::fs::write(
        dir.join("wide_test.volt"),
        "test \"own widths\" {
    let dut = Wide { };
    dut.a = 255;
    dut.n = 4;
    step(1);
    assert_eq(dut.s, 240);
    dut.a = 0;
    step(1);
    assert_eq(dut.t, 255);
}
",
    )
    .expect("yaz");
    let out = volt(&dir, &["test", "wide_test.volt", "--target-dir", "build"]);
    let text = format!(
        "{}
--- stderr
{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "{text}");
    assert!(text.contains("1 passed; 0 failed"), "{text}");
}
