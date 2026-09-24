//! Tanı paritesi (ADR-0070): `volt build`'in varsayılan emit'inde
//! ürettiği her tanı `volt check`'te de üretilir; editör (LSP) `check`
//! ile aynı tanıları görür. Külliyat: `tests/ui`, `examples`,
//! `tests/fixtures/parity` (sv-emit'in her tanı sınıfı için sonda).
//!
//! İleride sv-emit'e yeni bir tanı eklenirse ve `check` onu görmezse
//! (örn. emit doğrulaması `check`'ten çıkarılırsa) bu testler düşer.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn volt_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            volt_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "volt") {
            out.push(path);
        }
    }
}

fn corpus() -> Vec<PathBuf> {
    let mut files = Vec::new();
    for dir in [
        "tests/ui/pass",
        "tests/ui/fail",
        "examples",
        "tests/fixtures/parity",
    ] {
        volt_files(&root().join(dir), &mut files);
    }
    assert!(files.len() > 200, "külliyat bulunamadı: {}", files.len());
    files
}

fn parity_fixtures() -> Vec<PathBuf> {
    let mut files = Vec::new();
    volt_files(&root().join("tests/fixtures/parity"), &mut files);
    assert!(files.len() >= 40, "parite sondaları: {}", files.len());
    files
}

/// Bir tanının kimliği: kod + mesaj + birincil konum (dosya adı, satır,
/// sütun). Dosya adı yol biçiminden bağımsız olsun diye yalnız son bileşen.
type Key = (String, String, String, u64, u64);

fn keys(env: &Value) -> Vec<Key> {
    env["diagnostics"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .map(|d| {
            let prim = d["spans"]
                .as_array()
                .and_then(|s| s.iter().find(|s| s["primary"] == true))
                .cloned()
                .unwrap_or(Value::Null);
            let file = prim["file"].as_str().unwrap_or_default().replace('\\', "/");
            let file = file.rsplit('/').next().unwrap_or_default().to_string();
            (
                d["code"].as_str().unwrap_or_default().to_string(),
                d["message"].as_str().unwrap_or_default().to_string(),
                file,
                prim["start"]["line"].as_u64().unwrap_or(0),
                prim["start"]["col"].as_u64().unwrap_or(0),
            )
        })
        .collect()
}

fn run_json(args: &[&str], file: &Path) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en"])
        .args(args)
        .args(["--format", "json"])
        .arg(file)
        .output()
        .expect("volt çalışmalı");
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "{}: JSON değil ({e}): {}",
            file.display(),
            String::from_utf8_lossy(&out.stderr)
        )
    })
}

fn check_json(file: &Path) -> Value {
    run_json(&["check"], file)
}

fn build_json(file: &Path, target: &Path, emit: Option<&str>) -> Value {
    let target = target.to_string_lossy().into_owned();
    let mut args = vec!["build", "--target-dir", target.as_str()];
    if let Some(e) = emit {
        args.push(e);
    }
    run_json(&args, file)
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-parity-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// ADR-0070 A sınıfı: yalnız belirli bir emit kipinde bilinebilen
/// tanılar (`check` o kipin çıktısını üretmediği için görmez). Varsayılan
/// `volt build` için liste BOŞTUR.
///
/// * `--emit=sva`: SVA'ya inemeyen (E0003) ya da boyutlandırılamayan
///   (E2005, bağlamsız literal) kontrat ifadesi. Kontratlar varsayılan
///   build'de üretilmez; `volt test`'te aynı ifade W5001 ile izlenmez
///   (ADR-0064). SVA istenmediğinde hata değildir.
const A_CLASS_SVA: &[&str] = &["E0003", "E2005"];

// ═══ build ⊆ check ═══════════════════════════════════════════════════

/// Varsayılan `volt build`'in her tanısı `volt check`'te de var.
#[test]
fn every_default_build_diagnostic_is_reported_by_check() {
    let target = temp_dir("build");
    let mut violations = Vec::new();
    for file in corpus() {
        let check: BTreeSet<Key> = keys(&check_json(&file)).into_iter().collect();
        for k in keys(&build_json(&file, &target, None)) {
            if !check.contains(&k) {
                violations.push(format!("{}: {k:?}", file.display()));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "yalnız build'de görünen tanılar (A sınıfı listesi boş):\n{}",
        violations.join("\n")
    );
}

/// `--emit=sva` farkları yalnız A sınıfı listesindeki kodlar olabilir.
#[test]
fn sva_only_diagnostics_are_listed_a_class() {
    let target = temp_dir("sva");
    let mut violations = Vec::new();
    for file in corpus() {
        let check: BTreeSet<Key> = keys(&check_json(&file)).into_iter().collect();
        for k in keys(&build_json(&file, &target, Some("--emit=sva"))) {
            if !check.contains(&k) && !A_CLASS_SVA.contains(&k.0.as_str()) {
                violations.push(format!("{}: {k:?}", file.display()));
            }
        }
    }
    assert!(violations.is_empty(), "{}", violations.join("\n"));
}

// ═══ Parite sondaları ═══════════════════════════════════════════════

/// Sondanın ilk satırı `// parity: E0003 E2005` (beklenen `check` hata
/// kodları, sırasız) ya da `// parity: ok` (temiz derlenmeli).
fn expected_codes(file: &Path) -> Option<BTreeSet<String>> {
    let text = std::fs::read_to_string(file).expect("okunmalı");
    let first = text.lines().next()?;
    let spec = first.strip_prefix("// parity:")?.trim();
    if spec == "ok" {
        return Some(BTreeSet::new());
    }
    Some(spec.split_whitespace().map(str::to_string).collect())
}

/// Her sonda `check`'te beklenen hatayı verir (B/C düzeltmelerinin
/// fixture'ı); `ok` sondaları hem check hem build'den temiz geçer.
#[test]
fn parity_fixtures_report_expected_errors_in_check() {
    let target = temp_dir("fixtures");
    let mut bad = Vec::new();
    for file in parity_fixtures() {
        let expected = expected_codes(&file)
            .unwrap_or_else(|| panic!("{}: '// parity:' başlığı yok", file.display()));
        let env = check_json(&file);
        let errors: BTreeSet<String> = env["diagnostics"]
            .as_array()
            .map(|a| a.as_slice())
            .unwrap_or_default()
            .iter()
            .filter(|d| d["severity"] == "error")
            .map(|d| d["code"].as_str().unwrap_or_default().to_string())
            .collect();
        if errors != expected {
            bad.push(format!(
                "{}: beklenen {expected:?}, check {errors:?}",
                file.display()
            ));
        }
        if expected.is_empty() && build_json(&file, &target, None)["success"] != true {
            bad.push(format!(
                "{}: 'ok' sondası build'den geçmedi",
                file.display()
            ));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

// ═══ LSP = check ════════════════════════════════════════════════════

/// Editör tanıları `volt check` ile aynı (kod, mesaj, satır, sütun) —
/// ana dosyaya düşenler. İstisna: test veri dosyası içeriği (E8508,
/// E8510) editörde okunmaz.
#[test]
fn lsp_reports_the_same_diagnostics_as_check() {
    const LSP_EXEMPT: &[&str] = &["E8508", "E8510"];
    let mut bad = Vec::new();
    for file in corpus() {
        let name = file.file_name().unwrap().to_string_lossy().into_owned();
        let check: BTreeSet<Key> = keys(&check_json(&file))
            .into_iter()
            .filter(|k| k.2 == name && !LSP_EXEMPT.contains(&k.0.as_str()))
            .collect();
        let text = std::fs::read_to_string(&file).expect("okunmalı");
        volt_diagnostics::set_lang(volt_diagnostics::Lang::En);
        let analysis = volt_lsp::analysis::analyze(&file.display().to_string(), &text);
        let lsp: BTreeSet<Key> = analysis
            .diagnostics
            .iter()
            .filter(|d| !LSP_EXEMPT.contains(&d.code.as_str()))
            .map(|d| {
                let span = d.primary_span().expect("birincil span").span;
                let (line, col) = analysis.map.line_col(span);
                (
                    d.code.as_str().to_string(),
                    d.message.clone(),
                    name.clone(),
                    u64::from(line),
                    u64::from(col),
                )
            })
            .collect();
        if lsp != check {
            let only_check: Vec<_> = check.difference(&lsp).collect();
            let only_lsp: Vec<_> = lsp.difference(&check).collect();
            bad.push(format!(
                "{}:\n  yalnız check: {only_check:?}\n  yalnız LSP: {only_lsp:?}",
                file.display()
            ));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
