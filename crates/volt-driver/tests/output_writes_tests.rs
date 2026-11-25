//! Çıktı yazımının izin listesi (#221; biçim ADR-0137 §4'ünki).
//!
//! Volt'un yazdığı her dosya `crates/volt-driver/src/output.rs`'teki
//! `OutputRoot`'tan geçer: dosya yazımı, dizin oluşturma, kopyalama, yeniden
//! adlandırma ve silme. Yardımcı yolun kökün altında kaldığını ve yolda ya
//! da hedefte bağlantı olmadığını denetler. Test `volt-driver/src`'deki her
//! dosyayı tarar: yardımcı dışında bu işlemleri doğrudan yapan bir çağrı
//! testi düşürür, [`ALLOWED`]'daki istisna hariç. Listede olup artık
//! çağırmayan girdi de düşürür, böylece liste yalnız küçülür. Satır
//! numarası değil, dosya ve fonksiyon eşlenir (ADR-0133'teki gerekçe).
//! `#[cfg(test)]` öğeleri taranmaz: testler kendi geçici dizinlerine yazar.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// İzinli bir doğrudan çağrı: dosya (depo köküne göre), fonksiyon, neden.
struct Allowed {
    file: &'static str,
    function: &'static str,
    /// Neden yardımcıdan geçemiyor.
    reason: &'static str,
}

/// Bugün istisna yok: `volt-driver`'ın kendi geçici dizini yoktur, araçların
/// (Verilator, sby) çalışma dizinleri de çıktı kökünün altındadır.
const ALLOWED: [Allowed; 0] = [];

/// Yardımcının kendisi taranmaz.
const DEFINITION: &str = "crates/volt-driver/src/output.rs";

/// Dosya sistemini değiştiren çağrılar (yorum ve dizge dışında).
const FORBIDDEN: &[&str] = &[
    "fs::write",
    "fs::create_dir",
    "fs::copy",
    "fs::rename",
    "fs::hard_link",
    "fs::soft_link",
    "fs::remove_file",
    "fs::remove_dir",
    "fs::set_permissions",
    "File::create",
    "File::options",
    "OpenOptions",
    "fs::symlink(",
    "symlink_file(",
    "symlink_dir(",
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Kaynağın kod kısmı: yorumlar ve dizge/karakter sabitlerinin içi boşluk
/// olur, satır sonları kalır (satırlar ve parantezler kodunkidir).
fn code_only(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let blank = |c: char| if c == '\n' { '\n' } else { ' ' };
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                out.push(' ');
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            let mut depth = 0;
            while i < chars.len() {
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    depth += 1;
                    out.push_str("  ");
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    out.push_str("  ");
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    out.push(blank(chars[i]));
                    i += 1;
                }
            }
        } else if c == 'r' && (next == Some('"') || next == Some('#')) && !ident_before(&chars, i) {
            // Ham dizge: r"..." ya da r#"..."#.
            let mut j = i + 1;
            let mut hashes = 0;
            while chars.get(j) == Some(&'#') {
                hashes += 1;
                j += 1;
            }
            if chars.get(j) != Some(&'"') {
                out.push(c);
                i += 1;
                continue;
            }
            out.push('"');
            for _ in i + 1..=j {
                out.push(' ');
            }
            j += 1;
            while j < chars.len() {
                if chars[j] == '"' && (0..hashes).all(|k| chars.get(j + 1 + k) == Some(&'#')) {
                    out.push('"');
                    for _ in 0..hashes {
                        out.push(' ');
                    }
                    j += 1 + hashes;
                    break;
                }
                out.push(blank(chars[j]));
                j += 1;
            }
            i = j;
        } else if c == '"' {
            out.push('"');
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' {
                    out.push(' ');
                    i += 1;
                }
                if i < chars.len() {
                    out.push(blank(chars[i]));
                    i += 1;
                }
            }
            out.push('"');
            i += 1;
        } else if c == '\'' {
            // Karakter sabiti ('x', '\n', '\u{..}') ya da yaşam süresi ('a).
            let close = if next == Some('\\') {
                (i + 2..chars.len().min(i + 12)).find(|&k| chars[k] == '\'')
            } else if chars.get(i + 2) == Some(&'\'') {
                Some(i + 2)
            } else {
                None
            };
            match close {
                Some(k) => {
                    out.push('\'');
                    for _ in i + 1..k {
                        out.push(' ');
                    }
                    out.push('\'');
                    i = k + 1;
                }
                None => {
                    out.push(c);
                    i += 1;
                }
            }
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

fn ident_before(chars: &[char], i: usize) -> bool {
    i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_')
}

/// `#[cfg(test)]` öğelerinin satırları (öğe `{ ... }` bloğuysa bloğun sonuna
/// dek, `;` ile bitiyorsa o satıra dek).
fn test_lines(code: &str) -> BTreeSet<usize> {
    let mut lines = BTreeSet::new();
    let bytes: Vec<char> = code.chars().collect();
    let line_of = |pos: usize| bytes[..pos].iter().filter(|&&c| c == '\n').count();
    let mut search = 0;
    while let Some(found) = code[search..].find("#[cfg(test)]") {
        let start_byte = search + found;
        let start = code[..start_byte].chars().count();
        let mut i = start + "#[cfg(test)]".len();
        let mut depth = 0usize;
        let mut end = bytes.len().saturating_sub(1);
        while i < bytes.len() {
            match bytes[i] {
                '{' => depth += 1,
                '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        end = i;
                        break;
                    }
                }
                ';' if depth == 0 => {
                    end = i;
                    break;
                }
                _ => {}
            }
            i += 1;
        }
        for line in line_of(start)..=line_of(end) {
            lines.insert(line);
        }
        search = start_byte + "#[cfg(test)]".len();
    }
    lines
}

/// `fn ad` satırıysa fonksiyonun adı.
fn fn_name(line: &str) -> Option<&str> {
    let mut rest = line.trim_start();
    for prefix in ["pub(crate) ", "pub(super) ", "pub ", "const ", "async "] {
        rest = rest.strip_prefix(prefix).unwrap_or(rest);
    }
    let rest = rest.strip_prefix("fn ")?;
    let end = rest
        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    (end > 0).then(|| &rest[..end])
}

/// Bir kaynak metindeki doğrudan çağrılar (test öğeleri dışında), içinde
/// bulundukları fonksiyonla: (fonksiyon, çağrı). Çağrıyı gizleyen içe
/// aktarmalar (`use std::fs::*`, `use std::fs::write`, ...) ayrı döner.
fn writes(text: &str) -> (Vec<(String, String)>, Vec<String>) {
    let code = code_only(text);
    let tests = test_lines(&code);
    let mut found = Vec::new();
    let mut imports = Vec::new();
    let mut current: Option<String> = None;
    for (n, line) in code.lines().enumerate() {
        if tests.contains(&n) {
            continue;
        }
        if let Some(name) = fn_name(line) {
            current = Some(name.to_string());
        }
        let trimmed = line.trim_start();
        if (trimmed.starts_with("use ") || trimmed.starts_with("pub use "))
            && line.contains("fs::")
            && (line.contains("fs::*") || hides_a_write(line))
        {
            imports.push(trimmed.trim_end().to_string());
            continue;
        }
        for pattern in FORBIDDEN {
            if line.contains(pattern) {
                found.push((
                    current
                        .clone()
                        .unwrap_or_else(|| "<fonksiyon dışı>".to_string()),
                    pattern.trim_end_matches('(').to_string(),
                ));
            }
        }
    }
    (found, imports)
}

/// İçe aktarma bir yazma fonksiyonunu adıyla getiriyor mu
/// (`use std::fs::{write, ...}`): çağrı `fs::` öneki olmadan görünürdü.
fn hides_a_write(line: &str) -> bool {
    let names = [
        "write",
        "create_dir",
        "create_dir_all",
        "copy",
        "rename",
        "hard_link",
        "soft_link",
        "remove_file",
        "remove_dir",
        "remove_dir_all",
        "set_permissions",
        "OpenOptions",
    ];
    let after = line.split("fs::").nth(1).unwrap_or("");
    after
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .any(|word| names.contains(&word))
}

/// `volt-driver/src`'deki (dosya, fonksiyon, çağrı) üçlüleri ve kural dışı
/// içe aktarmalar.
fn scan() -> (BTreeSet<(String, String, String)>, Vec<String>, usize) {
    let root = repo_root();
    let mut files = Vec::new();
    rust_files(&root.join("crates/volt-driver/src"), &mut files);
    let mut found = BTreeSet::new();
    let mut bad = Vec::new();
    let mut definition_calls = 0;
    for path in files {
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .display()
            .to_string()
            .replace('\\', "/");
        let text = std::fs::read_to_string(&path).expect("kaynak okunmalı");
        let (calls, imports) = writes(&text);
        if rel == DEFINITION {
            definition_calls = calls.len();
            continue;
        }
        for (function, call) in calls {
            found.insert((rel.clone(), function, call));
        }
        bad.extend(imports.into_iter().map(|l| format!("{rel}: {l}")));
    }
    (found, bad, definition_calls)
}

/// Taramanın sonucunu izin listesiyle karşılaştırır; ADR-0125 biçiminde
/// hata metni.
fn verdict(
    found: &BTreeSet<(String, String, String)>,
    bad_imports: &[String],
    allowed: &[Allowed],
) -> Result<(), String> {
    let allowed_set: BTreeSet<(String, String)> = allowed
        .iter()
        .map(|a| (a.file.to_string(), a.function.to_string()))
        .collect();
    let callers: BTreeSet<(String, String)> = found
        .iter()
        .map(|(f, func, _)| (f.clone(), func.clone()))
        .collect();
    let new: Vec<_> = found
        .iter()
        .filter(|(f, func, _)| !allowed_set.contains(&(f.clone(), func.clone())))
        .collect();
    let stale: Vec<_> = allowed_set.difference(&callers).collect();
    if new.is_empty() && stale.is_empty() && bad_imports.is_empty() {
        return Ok(());
    }
    let mut m = String::new();
    let _ = writeln!(
        m,
        "volt-driver'da çıktı yardımcısından (`output::OutputRoot`) geçmeyen dosya yazımı"
    );
    let _ = writeln!(m, "ne oldu:");
    for (file, function, call) in &new {
        let _ = writeln!(m, "  doğrudan çağrı: {file} `{function}`: `{call}`");
    }
    for (file, function) in &stale {
        let _ = writeln!(m, "  listede ama artık çağırmıyor: {file} `{function}`");
    }
    for line in bad_imports {
        let _ = writeln!(m, "  çağrıyı gizleyen içe aktarma: {line}");
    }
    let _ = writeln!(
        m,
        "neden: Volt'un yazdığı her dosya çıktı kökünün altında kalır ve yolunda ya da \
         kendisinde bir bağlantı varsa yazılmaz (#221). Bunu tek yer denetler: \
         `crates/volt-driver/src/output.rs`. Doğrudan bir `std::fs` yazımı, klonlanan bir \
         depodaki bağlantının Volt'a başka bir yerdeki dosyanın üzerine yazdırmasına yol açar."
    );
    let _ = writeln!(
        m,
        "ne yapabilirsin: yazımı komutun `OutputRoot`'u üzerinden yap (`write`, \
         `create_dir_all`, `copy`, `remove_file`, `remove_dir_all`; araca verilen dizin için \
         `check_tree`). Yazım gerçekten çıktı kökünün dışında olmalıysa (ör. bir geçici \
         dizin) nedenini yazarak `ALLOWED`'a ekle. Artık çağırmayan girdiyi listeden sil; \
         içe aktarmayı modülüyle (`std::fs`) yaz."
    );
    Err(m)
}

#[test]
fn every_output_write_goes_through_the_helper() {
    let (found, bad, definition_calls) = scan();
    let mut report = String::new();
    for a in &ALLOWED {
        let _ = writeln!(report, "  {} `{}`: {}", a.file, a.function, a.reason);
    }
    println!("izin listesi ({} istisna):\n{report}", ALLOWED.len());
    // Tarayıcı yazımları görüyor mu: yardımcının kendisi onları yapar.
    assert!(
        definition_calls >= 5,
        "tarama yardımcının yazımlarını bulmadı ({definition_calls}): yol ya da tarayıcı yanlış"
    );
    if let Err(m) = verdict(&found, &bad, &ALLOWED) {
        panic!("{m}");
    }
}

/// Listede olmayan çağrı ve artık çağırmayan girdi testi düşürür; mesaj
/// ADR-0125'in üç parçasını taşır.
#[test]
fn a_direct_write_or_a_stale_entry_fails_with_what_why_and_how() {
    let allowed = [Allowed {
        file: "crates/volt-driver/src/doctor/mod.rs",
        function: "gone",
        reason: "örnek",
    }];
    let mut found = BTreeSet::new();
    found.insert((
        "crates/volt-driver/src/sim/run_cmd.rs".to_string(),
        "run_inner".to_string(),
        "fs::write".to_string(),
    ));
    let message = verdict(&found, &["x.rs: use std::fs::*;".to_string()], &allowed)
        .expect_err("doğrudan yazım testi düşürmeli");
    for part in [
        "ne oldu:",
        "neden:",
        "ne yapabilirsin:",
        "doğrudan çağrı: crates/volt-driver/src/sim/run_cmd.rs `run_inner`: `fs::write`",
        "listede ama artık çağırmıyor: crates/volt-driver/src/doctor/mod.rs `gone`",
        "çağrıyı gizleyen içe aktarma: x.rs: use std::fs::*;",
    ] {
        assert!(message.contains(part), "`{part}` yok:\n{message}");
    }
    assert!(verdict(&BTreeSet::new(), &[], &[]).is_ok());
}

/// Tarayıcı çağrıyı fonksiyonuna bağlar; yorumdaki, dizgedeki ve test
/// modülündeki çağrıyı saymaz (dizgedeki sütun 0'daki `}` test modülünü
/// erken kapatmaz); adıyla getiren içe aktarmayı kural dışı sayar.
#[test]
fn the_scanner_skips_comments_strings_and_test_items() {
    let text = "use std::fs;\n\
                use std::fs::{read_to_string, write};\n\
                /// `fs::write` belgede\n\
                pub(crate) fn emit(p: &Path) {\n    \
                    // std::fs::write(p, \"yorum\")\n    \
                    let s = \"fs::write içinde }\";\n    \
                    let r = r#\"fs::copy \"tırnak\" \"#;\n    \
                    let c = '{';\n    \
                    std::fs::write(p, s).ok();\n\
                }\n\
                fn second<'a>(x: &'a str) {\n    \
                    let _ = std::fs::OpenOptions::new();\n\
                }\n\
                #[cfg(test)]\n\
                use std::fs::remove_file;\n\
                #[cfg(test)]\n\
                mod tests {\n    \
                    const SRC: &str = \"module A {\n\
                }\n\";\n    \
                    fn t() { std::fs::create_dir_all(\"x\").unwrap(); }\n\
                }\n\
                fn after() { fs::rename(\"a\", \"b\").ok(); }\n";
    let (calls, imports) = writes(text);
    assert_eq!(
        calls,
        vec![
            ("emit".to_string(), "fs::write".to_string()),
            ("second".to_string(), "OpenOptions".to_string()),
            ("after".to_string(), "fs::rename".to_string()),
        ]
    );
    assert_eq!(
        imports,
        vec!["use std::fs::{read_to_string, write};".to_string()]
    );
}
