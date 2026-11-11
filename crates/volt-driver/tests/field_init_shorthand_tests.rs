//! Alan ilklendirme kısayolu (#204): `S { x }` ve `Inst { x }`,
//! `S { x: x }` ve `Inst { x: x }` demektir (grammar-full.ebnf
//! `FieldInit = Ident ":" Expr | Ident`). Önce her denetleyici değeri
//! olmayan alanı kendi başına atlıyordu: kısayol tip, CDC, güven,
//! el sıkışma ve zamanlama denetimlerinden hiç geçmiyordu. Ayrıştırıcı
//! artık kısayolu `x: x`'e açar; buradaki testler iki biçimin aynı
//! tanıları verdiğini denetler. Beklenen kodlar şartnameden, elle
//! (her fikstürün başlığı kuralı adlandırır).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Testin kendi geçici dizini (checkout'a yazılmaz).
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-shorthand-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("geçici dizin");
    dir
}

fn check_json(dir: &Path, file: &Path) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "check", "--format", "json"])
        .arg(file)
        .current_dir(dir)
        .env("VOLT_TOOL_BACKEND", "local")
        .output()
        .expect("volt çalışmalı");
    serde_json::from_slice(&out.stdout).expect("json")
}

fn diagnostics(env: &Value) -> Vec<Value> {
    env["diagnostics"].as_array().cloned().unwrap_or_default()
}

fn primary_line(d: &Value) -> Option<u64> {
    d["spans"]
        .as_array()
        .and_then(|s| s.iter().find(|s| s["primary"] == true))
        .and_then(|s| s["start"]["line"].as_u64())
}

/// `//~ KOD` + `//~^ ERROR <parça>`: kod, anotasyonun üstündeki satırda
/// (birincil etiket) ve iletisi parçayı içererek raporlanmalı; başka hata
/// olmamalı.
fn assert_ui_fail(name: &str) {
    let file = root().join("tests/ui/fail").join(name);
    let src = std::fs::read_to_string(&file).expect("okunmalı");
    let code = src
        .lines()
        .next()
        .and_then(|l| l.strip_prefix("//~ "))
        .expect("ilk satır //~ KOD")
        .trim()
        .to_string();
    let (line, part) = src
        .lines()
        .enumerate()
        .find_map(|(i, l)| {
            l.trim_start()
                .strip_prefix("//~^ ERROR ")
                .map(|p| (i as u64, p.trim().to_string()))
        })
        .expect("//~^ ERROR satırı");
    let dir = temp_dir(name.trim_end_matches(".volt"));
    let env = check_json(&dir, &file);
    let _ = std::fs::remove_dir_all(&dir);
    let errors: Vec<Value> = diagnostics(&env)
        .into_iter()
        .filter(|d| d["severity"] == "error")
        .collect();
    assert_eq!(errors.len(), 1, "{name}: tek hata bekleniyor: {env}");
    let d = &errors[0];
    assert_eq!(d["code"], code.as_str(), "{name}: {env}");
    assert_eq!(primary_line(d), Some(line), "{name}: {d}");
    assert!(
        d["message"].as_str().is_some_and(|m| m.contains(&part)),
        "{name}: '{part}' bekleniyor: {d}"
    );
}

#[test]
fn ui_fail_256_shorthand_field_type_e2001() {
    assert_ui_fail("256_shorthand_field_type.volt");
}

#[test]
fn ui_fail_257_shorthand_instance_type_e2001() {
    assert_ui_fail("257_shorthand_instance_type.volt");
}

#[test]
fn ui_fail_258_shorthand_field_cdc_e3001() {
    assert_ui_fail("258_shorthand_field_cdc.volt");
}

#[test]
fn ui_fail_259_shorthand_field_trust_e3009() {
    assert_ui_fail("259_shorthand_field_trust.volt");
}

#[test]
fn ui_fail_260_shorthand_field_handshake_e4007() {
    assert_ui_fail("260_shorthand_field_handshake.volt");
}

#[test]
fn ui_fail_261_shorthand_field_timing_e5010() {
    assert_ui_fail("261_shorthand_field_timing.volt");
}

/// Kısa biçimli fikstür ve onun uzun biçimi: `(dosya, kısa, uzun)`.
const PAIRS: &[(&str, &str, &str)] = &[
    ("256_shorthand_field_type.volt", "S { a }", "S { a: a }"),
    (
        "257_shorthand_instance_type.volt",
        "Child { x }",
        "Child { x: x }",
    ),
    (
        "258_shorthand_field_cdc.volt",
        "Flag { fast_flag }",
        "Flag { fast_flag: fast_flag }",
    ),
    (
        "259_shorthand_field_trust.volt",
        "Wrap { key }",
        "Wrap { key: key }",
    ),
    (
        "260_shorthand_field_handshake.volt",
        "Gate { ready }",
        "Gate { ready: ready }",
    ),
    (
        "261_shorthand_field_timing.volt",
        "Pair { a, b }",
        "Pair { a: a, b: b }",
    ),
];

/// Bir tanının biçimden bağımsız özü: kod, önem, birincil satır, ileti.
/// Sütun karşılaştırılmaz: uzun biçim satırı uzatır.
fn essence(env: &Value) -> Vec<(String, String, Option<u64>, String)> {
    let mut v: Vec<_> = diagnostics(env)
        .iter()
        .map(|d| {
            (
                d["code"].as_str().unwrap_or_default().to_string(),
                d["severity"].as_str().unwrap_or_default().to_string(),
                primary_line(d),
                d["message"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    v.sort();
    v
}

#[test]
fn shorthand_and_long_form_give_the_same_diagnostics() {
    let dir = temp_dir("pairs");
    let mut bad = Vec::new();
    for (name, short, long) in PAIRS {
        let src = std::fs::read_to_string(root().join("tests/ui/fail").join(name)).expect("oku");
        // Yalnız kod satırları: başlık yorumu iki biçimi de anar. Satır
        // sayısı değişmez, tanılar aynı satırlarda karşılaştırılır.
        let is_code = |l: &str| !l.trim_start().starts_with("//");
        let sites = src
            .lines()
            .filter(|l| is_code(l))
            .map(|l| l.matches(short).count())
            .sum::<usize>();
        assert_eq!(sites, 1, "{name}: '{short}' kodda bir kez geçmeli");
        let long_src: String = src
            .lines()
            .map(|l| {
                if is_code(l) {
                    l.replace(short, long)
                } else {
                    l.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let short_file = dir.join(format!("short_{name}"));
        let long_file = dir.join(format!("long_{name}"));
        std::fs::write(&short_file, &src).expect("yaz");
        std::fs::write(&long_file, long_src).expect("yaz");
        let s = essence(&check_json(&dir, &short_file));
        let l = essence(&check_json(&dir, &long_file));
        assert!(!l.is_empty(), "{name}: uzun biçim tanı vermeli");
        if s != l {
            bad.push(format!("{name}:\n  kısa: {s:?}\n  uzun: {l:?}"));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Temiz bir tasarımda iki biçim aynı SV'yi üretir: kısayolun kendi
/// emit yolu kalmadı, uzun biçimle aynı yoldan iner.
#[test]
fn shorthand_and_long_form_build_the_same_systemverilog() {
    const SHORT: &str = "struct P { a : u8, b : bool }

module Child {
    in  a : u8
    in  b : bool
    out y : u8
    y = if b { a } else { 0 }
}

module Top {
    in  clk : clock
    in  a   : u8
    in  b   : bool
    out p   : P
    out y   : u8

    reg r : P = P { a: 0, b: false }
    on clk {
        r <= P { a, b }
    }
    p = r
    let c = Child { a, b }
    y = c.y
}
";
    let long = SHORT
        .replace("r <= P { a, b }", "r <= P { a: a, b: b }")
        .replace("Child { a, b }", "Child { a: a, b: b }");
    assert_ne!(SHORT, long);
    let build = |tag: &str, src: &str| -> String {
        let dir = temp_dir(tag);
        let file = dir.join("top.volt");
        std::fs::write(&file, src).expect("yaz");
        let out = Command::new(env!("CARGO_BIN_EXE_volt"))
            .args(["--lang", "en", "build", "--target-dir"])
            .arg(dir.join("out"))
            .arg(&file)
            .current_dir(&dir)
            .env("VOLT_TOOL_BACKEND", "local")
            .output()
            .expect("volt çalışmalı");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            out.status.success() && !stderr.contains("warning["),
            "{tag}: {stderr}"
        );
        let mut sv = String::new();
        for m in ["Top", "Child"] {
            sv += &std::fs::read_to_string(dir.join(format!("out/rtl/{m}.sv"))).expect("SV");
        }
        let _ = std::fs::remove_dir_all(&dir);
        sv
    };
    assert_eq!(build("sv-short", SHORT), build("sv-long", &long));
}
