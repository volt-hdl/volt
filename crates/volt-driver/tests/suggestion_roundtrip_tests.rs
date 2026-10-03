//! Öneri gidiş-dönüş denetimi: derleyicinin önerdiği her düzeltme
//! uygulanınca özgün tanı GİDER ve YENİ hata ya da uyarı ÇIKMAZ.
//!
//! * Fikstürler `tests/suggestions/<ad>.volt`; ilk satır
//!   `// suggestion: KOD`. Fikstür o kodu üretir ve düzeltme dışında
//!   temizdir (başka tanı varsa düzeltmeden sonra da aynen kalır).
//! * Kapsama birebirdir: kaynakta öneri kuran her yer
//!   (`Suggestion::replace` / `Suggestion::line_above` ya da
//!   `// suggestion-helper: AD` ile işaretli yardımcı çağrısı) üstündeki
//!   satırlarda `// suggestion: ad1, ad2` işaretini taşır; her ad bir
//!   fikstürdür ve her fikstürü en az bir işaret anar. İşaretsiz yeni bir
//!   öneri yeri ya da fikstürü olmayan bir işaret bu testi düşürür.
//! * Çalışma zamanı güvencesi: `tests/ui` derlemindeki her önerinin kodu
//!   bir fikstürün başlığında geçer.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn repo(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

fn check_json(file: &Path) -> Vec<Value> {
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["check", "--format=json"])
        .arg(file)
        .env("VOLT_LANG", "en")
        .env("VOLT_TOOL_BACKEND", "local")
        .output()
        .expect("volt çalışmalı");
    // `volt check` dosya başına bir zarf yazar (ardışık JSON değerleri).
    let text = String::from_utf8_lossy(&out.stdout);
    serde_json::Deserializer::from_str(&text)
        .into_iter::<Value>()
        .map(|v| v.expect("json zarfı"))
        .flat_map(|env| env["diagnostics"].as_array().cloned().unwrap_or_default())
        .collect()
}

/// (kod, önem, mesaj): konumdan bağımsız kimlik. Düzeltme satır ekleyip
/// sonraki konumları kaydırabilir.
fn identities(diags: &[Value]) -> Vec<(String, String, String)> {
    let mut ids: Vec<_> = diags
        .iter()
        .map(|d| {
            (
                d["code"].as_str().unwrap_or_default().to_string(),
                d["severity"].as_str().unwrap_or_default().to_string(),
                d["message"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    ids.sort();
    ids
}

/// Önerinin tüm düzenlemeleri (birincil + `additional_edits`), bayt
/// aralığı ve metin olarak.
fn edits(s: &Value) -> Vec<(usize, usize, String)> {
    let one = |e: &Value| {
        (
            e["span"]["start"]["byte"].as_u64().expect("start") as usize,
            e["span"]["end"]["byte"].as_u64().expect("end") as usize,
            e["replacement"].as_str().expect("replacement").to_string(),
        )
    };
    let mut all = vec![one(s)];
    if let Some(more) = s["additional_edits"].as_array() {
        all.extend(more.iter().map(one));
    }
    all
}

/// Düzenlemeleri sondan başa uygular; aynı konumdaki eklemeler öneri
/// sırasıyla yazılır (volt_diagnostics::apply_edits ile aynı kural).
fn apply(src: &str, edits: &[(usize, usize, String)]) -> String {
    let mut order: Vec<usize> = (0..edits.len()).collect();
    order.sort_by(|&a, &b| (edits[b].0, edits[b].1, b).cmp(&(edits[a].0, edits[a].1, a)));
    let mut out = src.to_string();
    for i in order {
        let (start, end, text) = &edits[i];
        out.replace_range(*start..*end, text);
    }
    out
}

fn header_code(src: &str) -> Option<String> {
    src.lines()
        .next()?
        .strip_prefix("// suggestion: ")
        .map(|c| c.trim().to_string())
}

fn fixtures() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(repo("tests/suggestions"))
        .expect("tests/suggestions")
        .map(|e| e.expect("girdi").path())
        .filter(|p| p.extension().is_some_and(|e| e == "volt"))
        .collect();
    files.sort();
    files
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt_{name}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("geçici dizin");
    dir
}

/// Bir fikstürün her önerisini ayrı ayrı uygular ve yeniden denetler.
/// Hata metni döndürür (boşsa geçti).
fn roundtrip(file: &Path, dir: &Path) -> Vec<String> {
    let name = file.file_name().expect("ad").to_string_lossy().to_string();
    let src = std::fs::read_to_string(file).expect("okunmalı");
    let Some(code) = header_code(&src) else {
        return vec![format!("{name}: başlık '// suggestion: KOD' değil")];
    };
    let before = check_json(file);
    let targets: Vec<&Value> = before.iter().filter(|d| d["code"] == code).collect();
    let Some(target) = targets
        .iter()
        .find(|d| d["suggestions"].as_array().is_some_and(|a| !a.is_empty()))
    else {
        return vec![format!(
            "{name}: önerili {code} yok; tanılar: {:?}",
            identities(&before)
        )];
    };
    let mut failures = Vec::new();
    for (i, s) in target["suggestions"]
        .as_array()
        .expect("öneriler")
        .iter()
        .enumerate()
    {
        let fixed_src = apply(&src, &edits(s));
        let case_dir = dir.join(format!("{}_{i}", name.trim_end_matches(".volt")));
        std::fs::create_dir_all(&case_dir).expect("dizin");
        let fixed = case_dir.join(&name);
        std::fs::write(&fixed, &fixed_src).expect("yazılmalı");
        let after = check_json(&fixed);
        let count = |ds: &[Value]| ds.iter().filter(|d| d["code"] == code).count();
        if count(&after) >= count(&before) {
            failures.push(format!(
                "{name} öneri {i}: {code} gitmedi\n--- düzeltilmiş ---\n{fixed_src}"
            ));
        }
        // Hatasız kalan düzeltme SystemVerilog'a da inmeli (üretici
        // denetimleri `volt check`te koşmayabilir).
        if !after.iter().any(|d| d["severity"] == "error") {
            let out = Command::new(env!("CARGO_BIN_EXE_volt"))
                .args(["build", "--target-dir"])
                .arg(case_dir.join("build"))
                .arg(&fixed)
                .env("VOLT_LANG", "en")
                .output()
                .expect("volt build çalışmalı");
            if !out.status.success() {
                failures.push(format!(
                    "{name} öneri {i}: düzeltilmiş dosya derlenmedi:\n{}",
                    String::from_utf8_lossy(&out.stderr)
                ));
            }
        }
        // Özgün kod dışındaki her tanı önceden de vardı (çoklu küme).
        let mut remaining = identities(&before);
        for id in identities(&after) {
            match remaining.iter().position(|r| *r == id) {
                Some(at) => {
                    remaining.remove(at);
                }
                None => failures.push(format!(
                    "{name} öneri {i}: düzeltme YENİ tanı üretti: {id:?}\n--- düzeltilmiş ---\n{fixed_src}"
                )),
            }
        }
    }
    failures
}

#[test]
fn every_suggestion_removes_its_diagnostic_without_new_errors_or_warnings() {
    let dir = temp_dir("suggestion_roundtrip");
    let failures: Vec<String> = fixtures().iter().flat_map(|f| roundtrip(f, &dir)).collect();
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

// ─── Kapsama ────────────────────────────────────────────────────────

/// Kaynak taraması: öneri kuran her yerin işaretindeki fikstür adları.
/// `#[cfg(test)]` sonrası (birim test modülleri) taranmaz.
fn marked_sites() -> (BTreeMap<String, Vec<String>>, Vec<String>) {
    let mut sources = Vec::new();
    collect_rs(&repo("crates"), &mut sources);
    sources.sort();
    let texts: Vec<(PathBuf, String)> = sources
        .into_iter()
        .map(|p| {
            let t = std::fs::read_to_string(&p).expect("okunmalı");
            (p, t)
        })
        .collect();
    // Yardımcılar: `// suggestion-helper: AD` → `AD(` çağrısı da bir yerdir.
    let mut helpers: Vec<String> = Vec::new();
    for (_, text) in &texts {
        for line in text.lines() {
            if let Some(h) = line.trim().strip_prefix("// suggestion-helper: ") {
                helpers.push(h.trim().to_string());
            }
        }
    }
    let mut names: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut unmarked = Vec::new();
    for (path, text) in &texts {
        let lines: Vec<&str> = text.lines().collect();
        let end = lines
            .iter()
            .position(|l| l.trim() == "#[cfg(test)]")
            .unwrap_or(lines.len());
        for (i, line) in lines[..end].iter().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            let is_site = code.contains("Suggestion::replace(")
                || code.contains("Suggestion::line_above(")
                || helpers
                    .iter()
                    .any(|h| code.contains(&format!("{h}(")) && !code.contains("fn "));
            if !is_site {
                continue;
            }
            let at = format!(
                "{}:{}",
                path.strip_prefix(repo("")).unwrap_or(path).display(),
                i + 1
            );
            let marker = lines[i.saturating_sub(12)..=i].iter().rev().find_map(|l| {
                let t = l.trim();
                t.strip_prefix("// suggestion: ")
                    .map(|m| Some(m.to_string()))
                    .or_else(|| t.strip_prefix("// suggestion-helper: ").map(|_| None))
            });
            match marker {
                Some(Some(list)) => {
                    for n in list.split(',').map(str::trim).filter(|n| !n.is_empty()) {
                        names.entry(n.to_string()).or_default().push(at.clone());
                    }
                }
                Some(None) => {} // yardımcının kendi gövdesi
                None => unmarked.push(at),
            }
        }
    }
    (names, unmarked)
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).expect("dizin") {
        let p = e.expect("girdi").path();
        if p.is_dir() {
            // Entegrasyon testleri öneri kurmaz; kurarsa da tanı değildir.
            if p.file_name().is_some_and(|n| n == "tests" || n == "target") {
                continue;
            }
            collect_rs(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

#[test]
fn every_suggestion_site_has_exactly_matching_fixtures() {
    let (sites, unmarked) = marked_sites();
    assert!(
        unmarked.is_empty(),
        "işaretsiz öneri yeri (üstüne '// suggestion: fikstür_adı' yazın ve \
         tests/suggestions/fikstür_adı.volt ekleyin): {unmarked:#?}"
    );
    let fixture_names: BTreeSet<String> = fixtures()
        .iter()
        .map(|p| p.file_stem().expect("ad").to_string_lossy().into_owned())
        .collect();
    let marked: BTreeSet<String> = sites.keys().cloned().collect();
    let missing: Vec<_> = marked.difference(&fixture_names).collect();
    let orphan: Vec<_> = fixture_names.difference(&marked).collect();
    assert!(
        missing.is_empty(),
        "fikstürü olmayan işaret: {missing:?} ({sites:#?})"
    );
    assert!(
        orphan.is_empty(),
        "hiçbir yerin anmadığı fikstür: {orphan:?}"
    );
}

/// `tests/ui` derlemindeki öneriler de fikstürlü kodlardandır: işaret
/// atlanmış bir yardımcı zinciri burada yakalanır.
#[test]
fn every_suggestion_in_the_ui_corpus_has_a_fixture_code() {
    let covered: BTreeSet<String> = fixtures()
        .iter()
        .filter_map(|p| header_code(&std::fs::read_to_string(p).expect("okunmalı")))
        .collect();
    let mut files = Vec::new();
    for sub in ["tests/ui/fail", "tests/ui/pass"] {
        for e in std::fs::read_dir(repo(sub)).expect("dizin") {
            let p = e.expect("girdi").path();
            if p.extension().is_some_and(|x| x == "volt") {
                files.push(p);
            }
        }
    }
    files.sort();
    let mut uncovered = BTreeSet::new();
    for f in files {
        for d in check_json(&f) {
            let code = d["code"].as_str().unwrap_or_default().to_string();
            if d["suggestions"].as_array().is_some_and(|a| !a.is_empty())
                && !covered.contains(&code)
            {
                uncovered.insert(format!("{code} ({})", f.display()));
            }
        }
    }
    assert!(uncovered.is_empty(), "fikstürsüz öneri: {uncovered:#?}");
}
