//! #206: bir uyarı ayrıştırıcının kaskad bastırma penceresini
//! (error-recovery.md §5) açıyordu; hemen ardından gelen hata ve
//! dosyanın son deyimindeki bir uyarıdan sonra koşan desugar
//! geçitlerinin (`@mmio`, pipeline) bütün hataları yutuluyordu. Kaynak
//! çıkış 0 ile kabul ediliyor, kontratlar, register'lar ve stall mantığı
//! sessizce düşüyordu.
//!
//! Beklenen kodlar şartnameden, elle: her fikstürün başlığı kuralı
//! adlandırır. Kontrat sayımı ADR-0011'in eşlemesine dayanır:
//! `invariant`/`ensures`/`assert` kontratı `.sva`'da bir
//! `assert property` olur.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Testin kendi geçici dizini (checkout'a yazılmaz).
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-cascade-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("geçici dizin");
    dir
}

/// `volt check --format json`: (çıkış kodu, JSON zarfı).
fn check_json(dir: &Path, file: &Path) -> (i32, Value) {
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "check", "--format", "json"])
        .arg(file)
        .current_dir(dir)
        .env("VOLT_TOOL_BACKEND", "local")
        .output()
        .expect("volt çalışmalı");
    let env = serde_json::from_slice(&out.stdout).expect("json");
    (out.status.code().unwrap_or(-1), env)
}

fn primary_line(d: &Value) -> Option<u64> {
    d["spans"]
        .as_array()
        .and_then(|s| s.iter().find(|s| s["primary"] == true))
        .and_then(|s| s["start"]["line"].as_u64())
}

/// `//~ KOD` + bir ya da daha çok `//~^ ERROR <ileti parçası>`: dosya
/// reddedilmeli (çıkış 0 değil), her anotasyonun üstündeki satırda o
/// kodla ve iletiyi içeren bir hata olmalı, başka koddan hata olmamalı.
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
    let expected: Vec<(u64, String)> = src
        .lines()
        .enumerate()
        .filter_map(|(i, l)| {
            l.trim_start()
                .strip_prefix("//~^ ERROR ")
                .map(|p| (i as u64, p.trim().to_string()))
        })
        .collect();
    assert!(!expected.is_empty(), "{name}: //~^ ERROR satırı yok");

    let dir = temp_dir(name.trim_end_matches(".volt"));
    let (status, env) = check_json(&dir, &file);
    let _ = std::fs::remove_dir_all(&dir);
    assert_ne!(status, 0, "{name}: dosya reddedilmeli: {env}");
    let errors: Vec<Value> = env["diagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|d| d["severity"] == "error")
        .collect();
    for d in &errors {
        assert_eq!(d["code"], code.as_str(), "{name}: beklenmeyen hata: {d}");
    }
    for (line, part) in &expected {
        assert!(
            errors.iter().any(|d| primary_line(d) == Some(*line)
                && d["message"]
                    .as_str()
                    .is_some_and(|m| m.contains(part.as_str()))),
            "{name}: satır {line}'de '{part}' iletili {code} bekleniyor: {env}"
        );
    }
    assert_eq!(
        errors.len(),
        expected.len(),
        "{name}: anotasyon başına bir hata bekleniyor: {env}"
    );
}

#[test]
fn ui_fail_262_warning_then_stray_paren_e0001() {
    assert_ui_fail("262_warning_then_stray_paren.volt");
}

#[test]
fn ui_fail_263_mmio_warning_in_last_statement_e0015() {
    assert_ui_fail("263_mmio_warning_in_last_statement.volt");
}

#[test]
fn ui_fail_264_pipeline_stall_after_warning_e5013() {
    assert_ui_fail("264_pipeline_stall_after_warning.volt");
}

// ─── Kaynaktaki her kontrat .sva'ya ulaşır ───

/// Kaynakta yazılan `invariant`/`ensures`/`assert` kontratları: satır bu
/// anahtar sözcükle ve ardından ':' ya da boşlukla başlar. Bozuk yazılmış
/// kontrat da sayılır (`invariant x < 1`): reddedilmediyse kaybolmamalı.
fn written_contracts(src: &str) -> usize {
    src.lines()
        .map(str::trim_start)
        .filter(|l| {
            ["invariant", "ensures", "assert"].iter().any(|kw| {
                l.strip_prefix(kw).is_some_and(|rest| {
                    rest.starts_with(':') || rest.starts_with(char::is_whitespace)
                })
            })
        })
        .count()
}

/// `build/formal/*.sva` içindeki `assert property (...)` deyimleri: formal
/// aracın denetleyeceği iddialar (ADR-0011).
fn sva_asserts(dir: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir.join("build/formal")) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "sva"))
        .map(|e| std::fs::read_to_string(e.path()).expect("sva okunmalı"))
        .map(|t| {
            t.lines()
                .filter(|l| l.trim_start().starts_with("assert property ("))
                .count()
        })
        .sum()
}

/// Kabul edilen kaynakta yazılan her kontrat üretilen `.sva`'da bir
/// iddiadır; aksi halde kaynak reddedilmelidir. Önce bir uyarının hemen
/// ardından gelen hata yutuluyor, kaynak çıkış 0 ile kabul ediliyor ve
/// kontrat `.sva`'dan sessizce düşüyordu.
#[test]
fn every_written_contract_reaches_the_sva_or_the_source_is_rejected() {
    let issue_example =
        std::fs::read_to_string(root().join("tests/ui/fail/262_warning_then_stray_paren.volt"))
            .expect("okunmalı");
    let cases: Vec<(&str, String)> = vec![
        // #206'nın örneği: W0010, ardından fazladan ')' ve bir invariant.
        ("stray_paren", issue_example.clone()),
        // Aynı dosya ')' olmadan: geçerli; iki invariant da .sva'da.
        ("w0010_valid", issue_example.replace("c )", "c")),
        // W0010'dan bir token sonra ':' eksik: kontrat ya .sva'da ya da hata.
        (
            "w0010_missing_colon",
            "module A {\n    in  clk : clock\n    in  a : u8\n    in  b : u8\n    in  c : u8\n    \
             out r : u8\n    reg cnt : u8 = 0\n    r = a & b | c\n    invariant cnt < 200\n    \
             invariant: cnt != 7\n    on clk { cnt <= cnt + 1 }\n}\n"
                .to_string(),
        ),
        // W0020 (bilinmeyen nitelik) ardından geçerli kontratlar.
        (
            "w0020_valid",
            "@bogus\nmodule B {\n    in  clk : clock\n    in  a : u8\n    out r : u8\n    \
             reg cnt : u8 = 0\n    invariant: cnt < 200\n    ensures: cnt != 9\n    \
             r = a\n    assert: cnt != 250\n    on clk { cnt <= cnt + 1 }\n}\n"
                .to_string(),
        ),
    ];

    let mut built = 0;
    for (tag, src) in &cases {
        let dir = temp_dir(&format!("sva-{tag}"));
        let file = dir.join(format!("{tag}.volt"));
        std::fs::write(&file, src).expect("yazılmalı");
        let out = Command::new(env!("CARGO_BIN_EXE_volt"))
            .args(["--lang", "en", "build", "--emit", "sva"])
            .arg(&file)
            .current_dir(&dir)
            .env("VOLT_TOOL_BACKEND", "local")
            .output()
            .expect("volt çalışmalı");
        if out.status.success() {
            built += 1;
            let (written, emitted) = (written_contracts(src), sva_asserts(&dir));
            assert_eq!(
                emitted,
                written,
                "{tag}: kaynakta {written} kontrat, .sva'da {emitted} assert property; \
                 kabul edilen kaynak kontrat kaybetmemeli.\nstderr:\n{}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
    // Sayım boşa koşmasın: geçerli örnekler gerçekten derlenmeli.
    assert!(built >= 2, "geçerli örnekler derlenmeli ({built} derlendi)");
}
