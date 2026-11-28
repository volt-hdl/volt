//! Bir Trit indeks olamaz (#465, ADR-0146): `x[t]` reddedilir (E2003) ve
//! yardımı Trit'i açıkça işaretsiz indekse çevirmeyi gösterir:
//! `x[(t as i3 + 1) as u3]`. Bu test yardımın kodunun gerçek Verilator'da
//! doğru konumu okuduğunu sınar (ADR-0125 §4: yardımdaki kod sınanır).
//!
//! Değerler spec'ten elle hesaplandı: `t as i3` Trit'in değeridir
//! (-1, 0, +1), `+ 1` onu 0, 1, 2 yapar, `as u3` işaretsiz indekstir.
//! x = 5 = 0b0000_0101 için x[0] = 1, x[1] = 0, x[2] = 1, x[3] = 0;
//! TABLE = [10, 20, ..., 80] için TABLE[0..2] = 10, 20, 30. Düzeltmeden
//! önce `x[t]` tanısız derleniyordu ve t = -1 için x[3]'ü (0) ve
//! TABLE[3]'ü (40) okuyordu. İki tabanın adresi de 3 bittir (`u3`
//! indeksle aynı): adresten geniş değişken indeks const dizide bugün
//! Verilator'da WIDTHTRUNC ile durur, Trit'ten bağımsız.
//!
//! Verilator yoksa test atlanır; `VOLT_REQUIRE_TOOLS=verilator` ile (CI'ın
//! araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::PathBuf;
use std::process::Command;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-tritidx-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// `volt <args>` çıktısı (çıkış başarılı mı, stdout + stderr); kaynak
/// `design.volt` olarak yazılır.
fn volt(tag: &str, src: &str, args: &[&str]) -> (bool, String) {
    let dir = temp_dir(tag);
    std::fs::write(dir.join("design.volt"), src).expect("yaz");
    let output = Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("VOLT_TOOL_BACKEND", "local")
        .env("VOLT_LANG", "en")
        .env("NO_COLOR", "1")
        .current_dir(&dir)
        .args(args)
        .output()
        .expect("volt");
    let _ = std::fs::remove_dir_all(&dir);
    let text = format!(
        "{}\n--- stderr\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output.status.success(), text)
}

/// Tasarım; `INDEX` indeksin yazılışıyla değiştirilir. `up`/`down` bir
/// Trit seçer; `b` x'in, `e` TABLE'ın o konumdaki değeri.
const DESIGN: &str = "\
const TABLE : [u8; 8] = [10, 20, 30, 40, 50, 60, 70, 80]

module TritIndex {
    in  up   : bool
    in  down : bool
    in  x    : u8
    out b    : bool
    out e    : u8

    let t : Trit = if up { 1 } else if down { -1 } else { 0 }

    b = x[INDEX]
    e = TABLE[INDEX]
}
";

const TESTS: &str = "
test \"minus one reads position zero\" {
    let dut = TritIndex { };
    dut.up = false;
    dut.down = true;
    dut.x = 5;
    step(1);
    // t = -1: (-1 + 1) = 0; x[0] = 1, TABLE[0] = 10.
    assert_eq(dut.b, 1);
    assert_eq(dut.e, 10);
}

test \"zero reads position one\" {
    let dut = TritIndex { };
    dut.up = false;
    dut.down = false;
    dut.x = 5;
    step(1);
    // t = 0: 1; x[1] = 0, TABLE[1] = 20.
    assert_eq(dut.b, 0);
    assert_eq(dut.e, 20);
}

test \"plus one reads position two\" {
    let dut = TritIndex { };
    dut.up = true;
    dut.down = false;
    dut.x = 5;
    step(1);
    // t = +1: 2; x[2] = 1, TABLE[2] = 30.
    assert_eq(dut.b, 1);
    assert_eq(dut.e, 30);
}
";

const HELP_PREFIX: &str = "= help: convert the Trit to an unsigned index explicitly: ";

/// Tasarım `x[t]` ile reddedilir; yardımın kodu `volt check`'in kendi
/// çıktısından alınır, `x[t]`'nin yerine konur, tasarım bütün
/// denetimlerden tanısız geçer (ADR-0125 §4) ve Verilator'da her Trit
/// değeri için doğru konumu okur.
#[test]
fn the_help_conversion_from_volt_check_reads_the_right_position_in_verilator() {
    let (ok, text) = volt(
        "check",
        &DESIGN.replace("INDEX", "t"),
        &["check", "design.volt"],
    );
    assert!(!ok, "{text}");
    assert_eq!(
        text.matches("error[E2003]: an index must not be a Trit, found 'Trit'")
            .count(),
        2,
        "{text}"
    );
    let help = text
        .lines()
        .find_map(|l| l.trim().strip_prefix(HELP_PREFIX))
        .unwrap_or_else(|| panic!("yardım yok:\n{text}"));
    // `x[(t as i3 + 1) as u3] reads ...`: indeksin kodu köşeli parantezin içi.
    let index = help
        .strip_prefix("x[")
        .and_then(|h| h.split("] reads").next())
        .unwrap_or_else(|| panic!("yardımda x[...] yok: {help}"));
    assert_eq!(index, "(t as i3 + 1) as u3", "{help}");

    let fixed = DESIGN.replace("INDEX", index);
    let (ok, text) = volt("help", &fixed, &["check", "design.volt"]);
    assert!(ok, "{text}");
    assert!(text.contains("Result 0 error(s), 0 warning(s)"), "{text}");

    if tools::require(tools::Tool::Verilator).is_none() {
        return;
    }
    let (ok, out) = volt(
        "sim",
        &format!("{fixed}{TESTS}"),
        &["test", "design.volt", "--target-dir", "build"],
    );
    assert!(ok, "{out}");
    assert!(out.contains("test result: ok. 3 passed; 0 failed"), "{out}");
}
