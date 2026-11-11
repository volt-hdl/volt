//! Yerleşik primitiflerin sabit generic argüman tanıları (E2025, E2008,
//! E2003) iki dilde: ileti parametrenin adını, yardım satırı kuralını ve
//! imza kalıbını taşır. Parametre adları ve aralıklar ADR-0029'dan ("Sabit
//! generic kuralları"): `DEPTH` iki kuvveti 2..=65536, `WIDTH` (Counter)
//! 1..=64, `LEN` (ShiftRegister) ve `N` (arbiterler) 2..=64; imza
//! kalıpları ADR-0027, ADR-0029 ve ADR-0049'un primitif tablolarından
//! (`docs/stdlib.md`'de de aynı biçimde). Tanı metni
//! kullanıcıya görünen davranıştır (ADR-0109 §4).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

/// Testin kendi geçici dizini (checkout'a yazılmaz).
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-builtin-param-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("geçici dizin");
    dir
}

/// `inst` örneğini taşıyan modülün hataları: (kod, ileti, yardım).
fn errors(dir: &Path, lang: &str, inst: &str) -> Vec<(String, String, String)> {
    let file = dir.join("m.volt");
    let src = format!(
        "const W : u32 = 4\n\nmodule M {{\n    in  clk : clock\n    let x = {inst} {{}}\n}}\n"
    );
    std::fs::write(&file, src).expect("yazılmalı");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", lang, "check", "--format", "json"])
        .arg(&file)
        .current_dir(dir)
        .env("VOLT_TOOL_BACKEND", "local")
        .output()
        .expect("volt çalışmalı");
    let env: Value = serde_json::from_slice(&out.stdout).expect("json");
    env["diagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter(|d| d["severity"] == "error")
        .map(|d| {
            let s = |k: &str| d[k].as_str().unwrap_or_default().to_string();
            (s("code"), s("message"), s("help"))
        })
        .collect()
}

fn the_error(dir: &Path, lang: &str, inst: &str) -> (String, String, String) {
    let errs = errors(dir, lang, inst);
    assert_eq!(
        errs.len(),
        1,
        "{lang} {inst}: tek hata bekleniyor: {errs:?}"
    );
    errs[0].clone()
}

/// E2025: kural dışı değer. İleti primitifin ve parametrenin adını, yardım
/// satırı kuralın aralığını söyler.
#[test]
fn an_invalid_size_names_the_parameter_and_its_rule_in_both_languages() {
    let dir = temp_dir("rule");
    const POW2_EN: &str = "use a power of two: 2, 4, 8, 16, ...";
    const POW2_TR: &str = "iki kuvveti kullanın: 2, 4, 8, 16, ...";
    const R1_EN: &str = "use a value between 1 and 64";
    const R1_TR: &str = "1 ile 64 arasında bir değer kullanın";
    const R2_EN: &str = "use a value between 2 and 64";
    const R2_TR: &str = "2 ile 64 arasında bir değer kullanın";
    // (örnek, ad, parametre, değer, iki kuvveti mi, yardım EN, yardım TR)
    let cases: &[(&str, &str, &str, u32, bool, &str, &str)] = &[
        (
            "AsyncFifo<u8, 3>",
            "AsyncFifo",
            "DEPTH",
            3,
            true,
            POW2_EN,
            POW2_TR,
        ),
        (
            "SyncFifo<u8, 10>",
            "SyncFifo",
            "DEPTH",
            10,
            true,
            POW2_EN,
            POW2_TR,
        ),
        ("Ram<u8, 1>", "Ram", "DEPTH", 1, true, POW2_EN, POW2_TR),
        (
            "DualPortRam<u8, 6>",
            "DualPortRam",
            "DEPTH",
            6,
            true,
            POW2_EN,
            POW2_TR,
        ),
        (
            "AsyncDualPortRam<u8, 12>",
            "AsyncDualPortRam",
            "DEPTH",
            12,
            true,
            POW2_EN,
            POW2_TR,
        ),
        ("Counter<0>", "Counter", "WIDTH", 0, false, R1_EN, R1_TR),
        ("Counter<65>", "Counter", "WIDTH", 65, false, R1_EN, R1_TR),
        (
            "ShiftRegister<u8, 1>",
            "ShiftRegister",
            "LEN",
            1,
            false,
            R2_EN,
            R2_TR,
        ),
        (
            "RoundRobinArbiter<1>",
            "RoundRobinArbiter",
            "N",
            1,
            false,
            R2_EN,
            R2_TR,
        ),
        (
            "PriorityArbiter<65>",
            "PriorityArbiter",
            "N",
            65,
            false,
            R2_EN,
            R2_TR,
        ),
    ];
    for &(inst, name, param, value, pow2, help_en, help_tr) in cases {
        let (code, msg, help) = the_error(&dir, "en", inst);
        assert_eq!(code, "E2025", "{inst}");
        let want = if pow2 {
            format!("{name} {param} must be a power of two, got {value}")
        } else {
            format!("{name} {param} is out of range, got {value}")
        };
        assert_eq!(
            (msg.as_str(), help.as_str()),
            (want.as_str(), help_en),
            "{inst}"
        );
        let (code, msg, help) = the_error(&dir, "tr", inst);
        assert_eq!(code, "E2025", "{inst}");
        let want = if pow2 {
            format!("{name} {param} iki kuvveti olmalı, {value} verildi")
        } else {
            format!("{name} {param} aralık dışı, {value} verildi")
        };
        assert_eq!(
            (msg.as_str(), help.as_str()),
            (want.as_str(), help_tr),
            "{inst}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// E2008: literal olmayan sabit argüman; E2003: yanlış generic argüman
/// sayısı. Yardım satırı primitifin imza kalıbını gösterir.
#[test]
fn a_non_literal_size_or_a_wrong_arity_shows_the_signature_in_both_languages() {
    let dir = temp_dir("shape");
    // (literal olmayan argümanlı örnek, ad, parametre, imza kalıbı)
    let non_literal: &[(&str, &str, &str, &str)] = &[
        (
            "AsyncFifo<u8, W>",
            "AsyncFifo",
            "DEPTH",
            "AsyncFifo<T, DEPTH>",
        ),
        ("SyncFifo<u8, W>", "SyncFifo", "DEPTH", "SyncFifo<T, DEPTH>"),
        ("Ram<u8, W>", "Ram", "DEPTH", "Ram<T, DEPTH>"),
        (
            "DualPortRam<u8, W>",
            "DualPortRam",
            "DEPTH",
            "DualPortRam<T, DEPTH>",
        ),
        (
            "AsyncDualPortRam<u8, W>",
            "AsyncDualPortRam",
            "DEPTH",
            "AsyncDualPortRam<T, DEPTH>",
        ),
        ("Counter<W>", "Counter", "WIDTH", "Counter<WIDTH>"),
        (
            "ShiftRegister<u8, W>",
            "ShiftRegister",
            "LEN",
            "ShiftRegister<T, LEN>",
        ),
        (
            "RoundRobinArbiter<W>",
            "RoundRobinArbiter",
            "N",
            "RoundRobinArbiter<N>",
        ),
        (
            "PriorityArbiter<W>",
            "PriorityArbiter",
            "N",
            "PriorityArbiter<N>",
        ),
    ];
    for &(inst, name, param, shape) in non_literal {
        let got = the_error(&dir, "en", inst);
        let want = (
            "E2008".to_string(),
            format!("{name} {param} must be a compile-time integer literal"),
            format!("write the size directly, e.g. {shape}"),
        );
        assert_eq!(got, want, "{inst}");
        let got = the_error(&dir, "tr", inst);
        let want = (
            "E2008".to_string(),
            format!("{name} {param} derleme zamanı tam sayı literali olmalı"),
            format!("boyutu doğrudan yazın, ör. {shape}"),
        );
        assert_eq!(got, want, "{inst}");
    }
    // (yanlış sayıda argümanlı örnek, imza kalıbı)
    let arity: &[(&str, &str)] = &[
        ("AsyncFifo<u8>", "AsyncFifo<T, DEPTH>"),
        ("HandshakeSync<u8, 4>", "HandshakeSync<T>"),
        ("PulseSync<4>", "PulseSync"),
        ("SyncFifo<u8>", "SyncFifo<T, DEPTH>"),
        ("Ram<u8>", "Ram<T, DEPTH>"),
        ("DualPortRam<u8>", "DualPortRam<T, DEPTH>"),
        ("Counter<u8, 4>", "Counter<WIDTH>"),
        ("ShiftRegister<u8>", "ShiftRegister<T, LEN>"),
        ("RoundRobinArbiter<u8, 4>", "RoundRobinArbiter<N>"),
        ("PriorityArbiter<u8, 4>", "PriorityArbiter<N>"),
        ("EdgeDetect<4>", "EdgeDetect"),
        ("AsyncDualPortRam<u8>", "AsyncDualPortRam<T, DEPTH>"),
    ];
    for &(inst, shape) in arity {
        let (code, _, help) = the_error(&dir, "en", inst);
        assert_eq!(code, "E2003", "{inst}");
        assert_eq!(help, format!("write it as {shape} {{ ... }}"), "{inst}");
        let (code, _, help) = the_error(&dir, "tr", inst);
        assert_eq!(code, "E2003", "{inst}");
        assert_eq!(help, format!("{shape} {{ ... }} biçiminde yazın"), "{inst}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
