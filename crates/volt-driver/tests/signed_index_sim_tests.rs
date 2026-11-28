//! Bir indeks işaretsiz olmalı (#475, ADR-0147): `x[i]` (i : iN)
//! reddedilir (E2003) ve yardımı indeksi tabanın adres genişliğinde
//! işaretsize açıkça çevirmeyi gösterir: `x : u8` için `x[i as u3]`,
//! `[u8; 4]` için `arr[i as u2]`. Bu test yardımın kodunu `volt check`'in
//! kendi çıktısından alır, indeksin yerine koyar ve gerçek Verilator'da
//! okunan konumu sınar (ADR-0125 §4: yardımdaki kod sınanır). Adres
//! genişliği WIDTHTRUNC'ı (#474) önler: `[u8; 4]`'ün const dizisi 2 bitlik
//! indeks ister.
//!
//! Değerler spec'ten elle hesaplandı (type-inference.md §3.6): `i as uN`
//! işaret değişimidir; i3'ten u3'e bitler aynı kalır (-1 = 111 → 7, -3 =
//! 101 → 5), u2'ye daraltma üst biti keser (111 → 11 = 3, 101 → 01 = 1).
//! x = 0b1000_0100 için x[7] = 1, x[5] = 0, x[2] = 1; TABLE = [10, 20, 30,
//! 40] için TABLE[3] = 40, TABLE[2] = 30, TABLE[1] = 20; `y[k] = true`
//! 0'dan başlayan y'de y = 2^k. Test dili portu bit deseni olarak yazar:
//! i = -1 için 7, i = -3 için 5.
//!
//! Verilator yoksa Verilator kısmı atlanır; `VOLT_REQUIRE_TOOLS=verilator`
//! ile (CI'ın araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::PathBuf;
use std::process::Command;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-signedidx-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// `volt <args>` çıktısı (çıkış başarılı mı, stdout + stderr); kaynak
/// `design.volt` olarak yazılır, dizin sonra silinir.
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

/// Tasarım; `XIDX` `x`'in ve `y`'nin, `TIDX` `TABLE`'ın indeksiyle
/// değiştirilir.
const DESIGN: &str = "\
const TABLE : [u8; 4] = [10, 20, 30, 40]

module SignedIndex {
    in  i : i3
    in  x : u8
    out b : bool
    out e : u8
    out y : u8

    b = x[XIDX]
    e = TABLE[TIDX]
    comb {
        y = 0
        y[XIDX] = true
    }
}
";

const TESTS: &str = "
test \"minus one reads the highest positions\" {
    let dut = SignedIndex { };
    dut.i = 7;
    dut.x = 132;
    step(1);
    // i = -1: as u3 = 7, as u2 = 3; x[7] = 1, TABLE[3] = 40, y = 2^7.
    assert_eq(dut.b, 1);
    assert_eq(dut.e, 40);
    assert_eq(dut.y, 128);
}

test \"two reads position two\" {
    let dut = SignedIndex { };
    dut.i = 2;
    dut.x = 132;
    step(1);
    // i = 2: as u3 = 2, as u2 = 2; x[2] = 1, TABLE[2] = 30, y = 2^2.
    assert_eq(dut.b, 1);
    assert_eq(dut.e, 30);
    assert_eq(dut.y, 4);
}

test \"minus three keeps the low bits\" {
    let dut = SignedIndex { };
    dut.i = 5;
    dut.x = 132;
    step(1);
    // i = -3 (101): as u3 = 5, as u2 = 1; x[5] = 0, TABLE[1] = 20, y = 2^5.
    assert_eq(dut.b, 0);
    assert_eq(dut.e, 20);
    assert_eq(dut.y, 32);
}
";

const HELP_PREFIX: &str =
    "= help: convert the index explicitly to an unsigned number of the address width: ";

/// Yardımların `x[i as u3] for x : u8` ve `arr[i as u2] for arr : [u8; 4]`
/// biçimindeki kodundan indeksin dönüşümü, tabanın adıyla.
fn conversions(text: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = text
        .lines()
        .filter_map(|l| l.trim().strip_prefix(HELP_PREFIX))
        .map(|help| {
            let (code, rest) = help
                .split_once(" for ")
                .unwrap_or_else(|| panic!("yardımda 'for' yok: {help}"));
            let base = rest.split("; a negative").next().unwrap_or("").to_string();
            let index = code
                .split_once('[')
                .and_then(|(_, r)| r.strip_suffix(']'))
                .unwrap_or_else(|| panic!("yardımda [...] yok: {help}"))
                .to_string();
            (base, index)
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Tasarım işaretli indeksle reddedilir (üç E2003); yardımın kodu
/// `volt check`'in kendi çıktısından alınır, indekslerin yerine konur,
/// tasarım bütün denetimlerden tanısız geçer (ADR-0125 §4) ve Verilator'da
/// her değer için hesaplanan konumu okur ve yazar.
#[test]
fn the_help_conversion_from_volt_check_reads_the_right_position_in_verilator() {
    let rejected = DESIGN.replace("XIDX", "i").replace("TIDX", "i");
    let (ok, text) = volt("check", &rejected, &["check", "design.volt"]);
    assert!(!ok, "{text}");
    assert_eq!(
        text.matches("error[E2003]: an index must be unsigned, found 'i3'")
            .count(),
        3,
        "{text}"
    );
    let conv = conversions(&text);
    assert_eq!(
        conv,
        [
            ("arr : [u8; 4]".to_string(), "i as u2".to_string()),
            ("x : u8".to_string(), "i as u3".to_string()),
        ],
        "{text}"
    );

    let fixed = DESIGN
        .replace("XIDX", &conv[1].1)
        .replace("TIDX", &conv[0].1);
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
