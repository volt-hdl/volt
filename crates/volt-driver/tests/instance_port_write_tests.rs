//! Bir örneğin portuna atama (#378) — uçtan uca, gerçek Verilator'da.
//!
//! Örneğin çıkışını örneğin kendisi sürer. Kullanıcı modülü örneğinde
//! `u.q = d` E4011 alıyordu; yerleşik primitifte (`Counter`, `SyncFifo`)
//! aynı yazım kabul ediliyor ve SV aynı ağı hem sayacın `always_ff`'inden
//! hem bir `assign`'dan sürüyordu: geçersiz SV (IEEE 1800-2017 §6.5).
//! Verilator `--lint-only` MULTIDRIVEN ile düşüyor, Yosys iki sürücülü
//! netlist üretip 0 ile çıkıyordu.
//!
//! Özellik: Volt bir tasarımı kabul ederse ürettiği SV Verilator
//! `--lint-only -Wall`'dan temiz geçer. Yazımlı tasarım reddedilir (E4011,
//! SV yazılmaz); yazımsız aynı tasarım kabul edilir ve temiz geçer.
//!
//! Verilator yoksa lint adımı atlanır; `VOLT_REQUIRE_TOOLS=verilator` ile
//! (CI'ın araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// #378'in tasarımı; `write` satırı `cnt.overflow`'a yazar.
fn design(write: &str) -> String {
    format!(
        "\
module M06 {{
    in  clk : clock
    in  en  : bool
    out q   : bits<8>
    out w   : bool
    let cnt = Counter<8> {{ clk: clk, enable: en, clear: false }}
{write}
    q = cnt.count
    w = cnt.overflow
}}
"
    )
}

const FIFO: &str = "\
module MF {
    in  clk  : clock
    in  d    : u8
    in  push : bool
    in  pop  : bool
    out q    : u8
    out e    : bool
    let f = SyncFifo<u8, 4> { clk: clk, wr_data: d, wr_en: push, rd_en: pop }
    f.rd_data = d
    q = f.rd_data
    e = f.empty || f.full
}
";

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-inst-write-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

fn build(dir: &Path, src: &str) -> (Output, String) {
    let file = dir.join("m.volt");
    std::fs::write(&file, src).expect("tasarım");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["build", "--color", "never", "--target-dir"])
        .arg(dir.join("build"))
        .arg(&file)
        .current_dir(dir)
        .env("VOLT_LANG", "en")
        .output()
        .expect("volt çalışmalı");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out, text)
}

/// `top`'un SV'si Verilator `--lint-only -Wall`'dan temiz geçiyor mu
/// (Verilator yoksa `None`).
fn lints_clean(dir: &Path, top: &str) -> Option<(bool, String)> {
    let verilator = tools::require(tools::Tool::Verilator)?;
    let rtl = dir.join("build").join("rtl");
    let out = Command::new(&verilator)
        .args(["--lint-only", "-Wall", "--top-module", top])
        .arg(format!("{top}.sv"))
        .current_dir(&rtl)
        .output()
        .expect("verilator çalışmalı");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let findings = text.lines().filter(|l| l.starts_with('%')).count();
    Some((out.status.success() && findings == 0, text))
}

/// Volt kabul ettiyse SV temiz geçmeli; reddettiyse SV yazılmamış olmalı.
fn accepted_means_clean_sv(tag: &str, src: &str, top: &str) -> (bool, String) {
    let dir = temp_dir(tag);
    let (out, text) = build(&dir, src);
    let accepted = out.status.success();
    if accepted {
        if let Some((clean, lint)) = lints_clean(&dir, top) {
            assert!(clean, "{tag}: Volt kabul etti, SV temiz değil:\n{lint}");
        }
    } else {
        assert!(
            !dir.join("build")
                .join("rtl")
                .join(format!("{top}.sv"))
                .exists(),
            "{tag}: reddedilen tasarımın SV'si yazıldı"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
    (accepted, text)
}

#[test]
fn issue_378_a_written_builtin_output_is_rejected_before_the_sv() {
    // Önce: `volt build` 0 ile çıkıyor, Verilator `MULTIDRIVEN`
    // ('cnt_overflow' hem always_ff'te hem assign'da yazılıyor).
    let (accepted, text) =
        accepted_means_clean_sv("counter", &design("    cnt.overflow = true"), "M06");
    assert!(!accepted, "{text}");
    assert!(text.contains("error[E4011]"), "{text}");
    assert!(
        text.contains("'cnt.overflow' is an output of instance 'cnt'"),
        "{text}"
    );
}

#[test]
fn a_written_fifo_output_is_rejected_before_the_sv() {
    let (accepted, text) = accepted_means_clean_sv("fifo", FIFO, "MF");
    assert!(!accepted, "{text}");
    assert!(text.contains("error[E4011]"), "{text}");
}

#[test]
fn the_same_design_reading_the_output_builds_and_lints_clean() {
    // Yazım yoksa tasarım geçerli: kabul edilir ve SV temizdir.
    let (accepted, text) = accepted_means_clean_sv("read-only", &design(""), "M06");
    assert!(accepted, "{text}");
}
