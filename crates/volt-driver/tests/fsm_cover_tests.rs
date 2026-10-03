//! Otomatik FSM geçiş cover'ı testin son kenarındaki geçişi de sayar
//! (ADR-0064 eki). Cover izleyicisi kenardan önceki değerlerle örnekler
//! (formal ile aynı); son kenardan sonraki durum bir sonraki kenarda
//! görülecekti ama test orada biter — önceden son geçiş "NEVER HIT"
//! görünüyordu (kitap, Tur: trafik ışığı).
//!
//! Gerçek Verilator ister; yoksa atlanır, `VOLT_REQUIRE_TOOLS=verilator`
//! ile (CI'ın araçlı işi) düşer (ADR-0079 §3).

mod tools;

use std::path::PathBuf;
use std::process::Command;

/// Satırlar sabit: geçişler 14 (Red→Green), 19 (Green→Yellow),
/// 25 (Yellow→Red); sayaç sarması 17.
const LIGHT: &str = "\
enum Light { Red, Green, Yellow }

pub module TrafficLight {
    in  clk   : clock
    in  go    : bool
    out red   : bool

    reg state_r : Light = Light::Red
    reg timer_r : u2    = 0

    on clk {
        match state_r {
            Light::Red => {
                if go { state_r <= Light::Green }
            }
            Light::Green => {
                if timer_r == 3 {
                    timer_r <= 0
                    state_r <= Light::Yellow
                } else {
                    timer_r <= timer_r + 1
                }
            }
            Light::Yellow => {
                state_r <= Light::Red
            }
        }
    }

    red = state_r == Light::Red
}
";

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-fsmcov-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// `volt test` çıktısı; Verilator yoksa `None` (test atlanır).
fn run(tag: &str, body: &str) -> Option<String> {
    tools::require(tools::Tool::Verilator)?;
    let dir = temp_dir(tag);
    let test = format!("test \"{tag}\" {{\n    let dut = TrafficLight {{ }};\n{body}}}\n");
    std::fs::write(dir.join("light_test.volt"), format!("{LIGHT}\n{test}")).expect("yaz");
    let output = Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("VOLT_TOOL_BACKEND", "local")
        .current_dir(&dir)
        .env("VOLT_LANG", "en")
        .args(["test", "light_test.volt", "--target-dir", "build"])
        .output()
        .expect("volt test");
    let out = format!(
        "{}\n--- stderr\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(output.status.code(), Some(0), "{out}");
    Some(out)
}

/// Cover özet satırı: `TrafficLight.cov_N (light_test.volt:L, ...)  <sonuç>`.
fn cover_line(out: &str, line: u32) -> &str {
    let key = format!("(light_test.volt:{line}, auto FSM transition)");
    out.lines()
        .find(|l| l.contains(&key))
        .unwrap_or_else(|| panic!("cover satırı yok ({key}):\n{out}"))
}

#[test]
fn transition_on_the_last_edge_of_a_test_is_hit() {
    // Red→Green 1. kenarda, Green→Yellow 5. (son) kenarda.
    let Some(out) = run(
        "last-edge",
        "    dut.go = true;\n    step(1);\n    dut.go = false;\n    step(4);\n    assert_false(dut.red);\n",
    ) else {
        return;
    };
    assert!(cover_line(&out, 14).ends_with("hit 1 time"), "{out}");
    assert!(cover_line(&out, 19).ends_with("hit 1 time"), "{out}");
    // Yellow→Red hiç olmadı: yanlış pozitif yok.
    assert!(cover_line(&out, 25).ends_with("NEVER HIT"), "{out}");
}

#[test]
fn transition_that_never_fires_stays_never_hit() {
    // `go` hiç gelmez: ışık kırmızıda kalır, hiçbir geçiş olmaz.
    let Some(out) = run("idle", "    step(3);\n    assert_true(dut.red);\n") else {
        return;
    };
    for line in [14, 19, 25] {
        assert!(cover_line(&out, line).ends_with("NEVER HIT"), "{out}");
    }
}

#[test]
fn every_transition_is_counted_exactly_once() {
    // Tam tur (6 kenar) + bekleme: her geçiş bir kez; son durum (Red,
    // geçiş değil) sonda ek sayım üretmez.
    let Some(out) = run(
        "full",
        "    dut.go = true;\n    step(1);\n    dut.go = false;\n    step(5);\n    assert_true(dut.red);\n    step(2);\n",
    ) else {
        return;
    };
    for line in [14, 19, 25] {
        assert!(cover_line(&out, line).ends_with("hit 1 time"), "{out}");
    }
}
