//! Tanı metinleri okura göre yazılır: `tests/ui` derleminin her tanısında
//! (iki dilde; ileti, etiket, not, çözüm) iç belge numarası (ADR-) ve
//! alınmamış alan adı yoktur, sayıdan önceki İngilizce artikel doğrudur
//! ("an 8-bit", "a 16-bit").

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use volt_diagnostics::a_an;

fn repo(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

fn corpus() -> Vec<PathBuf> {
    let mut files = Vec::new();
    for sub in ["tests/ui/fail", "tests/ui/pass", "tests/suggestions"] {
        for e in std::fs::read_dir(repo(sub)).expect("dizin") {
            let p = e.expect("girdi").path();
            if p.extension().is_some_and(|x| x == "volt") {
                files.push(p);
            }
        }
    }
    files.sort();
    files
}

/// Tanının okura görünen metinleri.
fn texts(d: &Value) -> Vec<String> {
    let mut out = vec![d["message"].as_str().unwrap_or_default().to_string()];
    out.extend(d["help"].as_str().map(str::to_string));
    for key in ["spans", "notes"] {
        for item in d[key].as_array().into_iter().flatten() {
            for field in ["label", "text"] {
                out.extend(item[field].as_str().map(str::to_string));
            }
        }
    }
    out
}

fn diagnostics(file: &Path, lang: &str) -> Vec<Value> {
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["check", "--format=json"])
        .arg(file)
        .env("VOLT_LANG", lang)
        .env("VOLT_TOOL_BACKEND", "local")
        .output()
        .expect("volt çalışmalı");
    let text = String::from_utf8_lossy(&out.stdout);
    serde_json::Deserializer::from_str(&text)
        .into_iter::<Value>()
        .map(|v| v.expect("json zarfı"))
        .flat_map(|env| env["diagnostics"].as_array().cloned().unwrap_or_default())
        .collect()
}

/// "a 8" / "an 16" gibi yanlış artikeller (sayı okunuşuna göre).
fn wrong_articles(text: &str) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    words
        .windows(2)
        .filter_map(|w| {
            let article = w[0].to_ascii_lowercase();
            if article != "a" && article != "an" {
                return None;
            }
            let digits: String = w[1].chars().take_while(char::is_ascii_digit).collect();
            let n: u64 = digits.parse().ok()?;
            (a_an(n) != article).then(|| format!("'{} {}'", w[0], w[1]))
        })
        .collect()
}

#[test]
fn article_check_flags_only_wrong_articles() {
    assert_eq!(wrong_articles("a 8-bit value"), ["'a 8-bit'"]);
    assert!(wrong_articles("an 8-bit value does not fit in a 4-bit target").is_empty());
    assert_eq!(wrong_articles("an 16-bit"), ["'an 16-bit'"]);
}

#[test]
fn ui_corpus_diagnostics_name_no_adr_and_use_correct_articles() {
    let mut bad = Vec::new();
    for file in corpus() {
        for lang in ["en", "tr"] {
            for d in diagnostics(&file, lang) {
                for t in texts(&d) {
                    if t.contains("ADR-") || t.contains(concat!("volthdl", ".org")) {
                        bad.push(format!("{} [{lang}] {}: {t}", file.display(), d["code"]));
                    }
                    if lang == "en" {
                        for w in wrong_articles(&t) {
                            bad.push(format!("{} {}: {w} in {t}", file.display(), d["code"]));
                        }
                    }
                }
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
