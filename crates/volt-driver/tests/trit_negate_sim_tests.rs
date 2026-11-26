//! Trit'in tersi `-t`'dir (#434, ADR-0142): `~t` reddedilir (E2003) ve
//! yardımı `-t`'yi gösterir. Bu test yardımın kodunun gerçek Verilator'da
//! doğru değeri verdiğini sınar (ADR-0125 §4: yardımdaki kod sınanır).
//!
//! Değerler spec'ten elle hesaplandı: `-Trit` Trit'tir (type-inference.md
//! §3.4) ve -(+1) = -1, -0 = 0, -(-1) = +1 (ADR-0003). Trit × iN ternary
//! MAC'tir (§3.3): x = 5 için -t * x sırayla -5, 0, 5. Test dili bir çıkış
//! portunu bit deseniyle okur: i2'de -1 = 3, +1 = 1; i8'de -5 = 251.
//! Düzeltmeden önce `~t` tanısız derleniyordu ve t = +1 için `10` kodunu
//! (i2'de 2, yani -2) veriyordu.
//!
//! Verilator yoksa test atlanır; `VOLT_REQUIRE_TOOLS=verilator` ile (CI'ın
//! araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::PathBuf;
use std::process::Command;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-tritneg-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// `volt test` çıktısı (stdout + stderr).
fn volt_test(tag: &str, src: &str) -> String {
    let dir = temp_dir(tag);
    std::fs::write(dir.join("trit_negate_test.volt"), src).expect("yaz");
    let output = Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("VOLT_TOOL_BACKEND", "local")
        .env("VOLT_LANG", "en")
        .env("NO_COLOR", "1")
        .current_dir(&dir)
        .args(["test", "trit_negate_test.volt", "--target-dir", "build"])
        .output()
        .expect("volt test");
    let _ = std::fs::remove_dir_all(&dir);
    format!(
        "{}\n--- stderr\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// `up`/`down` bir Trit seçer; `n` onun tersi, `m` tersiyle x'in çarpımı.
const NEGATE: &str = "\
module TritNegate {
    in  up   : bool
    in  down : bool
    in  x    : i8
    out n    : i2
    out m    : i8

    let t : Trit = if up { 1 } else if down { -1 } else { 0 }

    n = (-t) as i2
    m = (-t) * x
}

test \"minus plus one is minus one\" {
    let dut = TritNegate { };
    dut.up = true;
    dut.down = false;
    dut.x = 5;
    step(1);
    // -(+1) = -1: i2 bit pattern 11 = 3; -1 * 5 = -5: i8 pattern 251.
    assert_eq(dut.n, 3);
    assert_eq(dut.m, 251);
}

test \"minus zero is zero\" {
    let dut = TritNegate { };
    dut.up = false;
    dut.down = false;
    dut.x = 5;
    step(1);
    assert_eq(dut.n, 0);
    assert_eq(dut.m, 0);
}

test \"minus minus one is plus one\" {
    let dut = TritNegate { };
    dut.up = false;
    dut.down = true;
    dut.x = 5;
    step(1);
    // -(-1) = +1: i2 pattern 01 = 1; +1 * 5 = 5.
    assert_eq(dut.n, 1);
    assert_eq(dut.m, 5);
}
";

#[test]
fn minus_t_negates_every_trit_value_in_verilator() {
    if tools::require(tools::Tool::Verilator).is_none() {
        return;
    }
    let out = volt_test("negate", NEGATE);
    assert!(out.contains("test result: ok. 3 passed; 0 failed"), "{out}");
}

/// Aynı tasarım `~t` ile yazılınca derlenmez: E2003 ve yardımı `-t`.
/// Araç gerektirmez (`volt check`).
#[test]
fn the_same_design_with_bit_not_is_e2003_with_minus_t_in_the_help() {
    let dir = temp_dir("check");
    let src = NEGATE.replace("(-t)", "(~t)");
    std::fs::write(dir.join("trit_not.volt"), &src).expect("yaz");
    let output = Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("VOLT_LANG", "en")
        .env("NO_COLOR", "1")
        .current_dir(&dir)
        .args(["check", "trit_not.volt"])
        .output()
        .expect("volt check");
    let _ = std::fs::remove_dir_all(&dir);
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!output.status.success(), "{text}");
    assert_eq!(
        text.matches("error[E2003]: operator '~' is not defined for 'Trit'")
            .count(),
        2,
        "{text}"
    );
    assert!(text.contains("-t"), "{text}");
}

/// `volt check`'in çıktısı (çıkış başarılı mı, stdout + stderr).
fn volt_check(tag: &str, src: &str) -> (bool, String) {
    let dir = temp_dir(tag);
    std::fs::write(dir.join("design.volt"), src).expect("yaz");
    let output = Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("VOLT_LANG", "en")
        .env("NO_COLOR", "1")
        .current_dir(&dir)
        .args(["check", "design.volt"])
        .output()
        .expect("volt check");
    let _ = std::fs::remove_dir_all(&dir);
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output.status.success(), text)
}

/// Trit dizisinde yardım her elemanı dizinin gerçek uzunluğuyla çeviren
/// döngüyü yazar (ADR-0142 §2). Yardımdaki kod `volt check`'in kendi
/// çıktısından alınır, `y = ~a`'nın yerine konur ve tasarım bütün
/// denetimlerden (SV eşlemesi dahil) tanısız geçer (ADR-0125 §4).
#[test]
fn the_array_help_code_from_volt_check_compiles_clean() {
    let design = |body: &str| {
        format!("module M {{\n    in  a : [Trit; 3]\n    out y : [Trit; 3]\n    {body}\n}}\n")
    };
    let (ok, text) = volt_check("array-not", &design("y = ~a"));
    assert!(!ok, "{text}");
    let help = text
        .lines()
        .find_map(|l| {
            l.trim()
                .strip_prefix("= help: negate each element with '-': ")
        })
        .unwrap_or_else(|| panic!("dizi yardımı yok:\n{text}"));
    let code = help.split("; ").next().expect("yardımda kod");
    assert_eq!(code, "for i in 0..3 { y[i] = -a[i] }", "{text}");
    let (ok, text) = volt_check("array-help", &design(code));
    assert!(ok, "{text}");
    assert!(text.contains("Result 0 error(s), 0 warning(s)"), "{text}");
}
