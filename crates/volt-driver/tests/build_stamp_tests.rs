//! Verilator derleme dizini damgası: başarılı derleme obj dizinine
//! `.volt-build-stamp` yazar (Volt sürümü, arka uç, Verilator sürümü,
//! imaj özeti). Damgası olmayan (yarıda kalmış) ya da başka bir araçla
//! kurulmuş dizin elle silinmeden temizden kurulur.
//!
//! Gerçek Verilator gerekir; yoksa atlanır, `VOLT_REQUIRE_TOOLS=verilator`
//! ile (CI'ın araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const DESIGN: &str = "\
module Echo {
    in  clk : clock
    in  d   : u4
    out q   : u4

    reg hold : u4 = 0

    on clk {
        hold <= d
    }

    q = hold
}

test \"echo\" {
    let dut = Echo { };
    dut.d = 5;
    step(1);
    assert_eq(dut.q, 5);
}
";

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-stamp-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

fn volt_test(dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_volt"))
        .current_dir(dir)
        .env("VOLT_LANG", "en")
        .env("VOLT_TOOL_BACKEND", "local")
        .args(["test", "echo_test.volt", "--target-dir", "build"])
        .output()
        .expect("volt test")
}

fn assert_ok(out: &Output) {
    let text = format!(
        "{}\n--- stderr\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(text.contains("test result: ok. 1 passed"), "{text}");
}

#[test]
fn a_half_built_or_foreign_obj_dir_is_rebuilt_without_manual_cleanup() {
    if tools::require(tools::Tool::Verilator).is_none() {
        return;
    }
    let dir = temp_dir("rebuild");
    std::fs::write(dir.join("echo_test.volt"), DESIGN).expect("yaz");
    let obj = dir.join("build/sim/echo_test/obj_echo");
    let stamp = obj.join(".volt-build-stamp");

    // 1. Temiz derleme damga yazar.
    assert_ok(&volt_test(&dir));
    let text = std::fs::read_to_string(&stamp).expect("damga yazılmalı");
    assert!(text.contains("backend local"), "{text}");
    assert!(text.contains("verilator Verilator"), "{text}");
    assert!(text.contains("image -"), "{text}");

    // 2. Yarıda kalmış derleme: damga yok, Makefile bozuk. Damgasız
    //    dizin kullanılsaydı make bozuk dosyayla düşerdi.
    std::fs::remove_file(&stamp).expect("damga silinir");
    std::fs::write(obj.join("VEcho.mk"), "this is not a makefile (\n").expect("boz");
    assert_ok(&volt_test(&dir));
    assert!(stamp.is_file(), "yeniden kurulan dizin damgalı");

    // 3. Başka araçla (Docker) kurulmuş dizin: damga uyuşmaz.
    std::fs::write(&stamp, "volt 0.1.0\nbackend docker\nverilator x\nimage y\n").expect("damga");
    std::fs::write(obj.join("VEcho.mk"), "this is not a makefile (\n").expect("boz");
    assert_ok(&volt_test(&dir));
    let text = std::fs::read_to_string(&stamp).expect("damga");
    assert!(text.contains("backend local"), "{text}");

    let _ = std::fs::remove_dir_all(&dir);
}
