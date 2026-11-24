//! Sözdizimsel saat/reset sınıflamasının izin listesi (#412, ADR-0137).
//!
//! `volt_ast::alias::signal_class` bir portun saat, reset ya da veri
//! olduğunu takma ad zincirinden okur. Derleyici çıktısını etkileyen
//! anlamsal karar tip denetiminden ya da ad çözümlemeden gelir
//! (`volt_hir::port_class::DefTypes`); zincir yalnız tip denetiminin
//! sonucuna ulaşamayan yerlerde, nedeniyle birlikte [`ALLOWED`]'da durur.
//! Test her `crates/*/src/**/*.rs` dosyasını tarar: listede olmayan yeni
//! bir çağıran ya da artık çağırmayan bir girdi testi düşürür (ADR-0125
//! biçiminde). Satır numarası değil, dosya ve fonksiyon eşlenir: kod
//! taşınınca liste değişmez (ADR-0133'teki gerekçe).

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// İzinli bir çağıran: dosya (depo köküne göre), fonksiyon ve neden.
struct Allowed {
    file: &'static str,
    function: &'static str,
    /// Neden tip denetiminin sonucunu okuyamıyor.
    reason: &'static str,
    /// ADR-0137'nin geçiş sırasında girdiyi kaldıracak adım.
    until: &'static str,
}

/// Bugünkü çağıranlar (#234'ün düzeltmesi, PR #416).
const ALLOWED: [Allowed; 7] = [
    Allowed {
        file: "crates/volt-syntax/src/parser/desugar.rs",
        function: "single_clock",
        reason: "pipeline saati (E5011): ayrıştırma sırasında, ad çözümlemeden önce koşar",
        until: "orta vade: geçit ad çözümlemenin sonucunu okur (#415)",
    },
    Allowed {
        file: "crates/volt-syntax/src/parser/register_assign.rs",
        function: "reg_clocks",
        reason: "E0019 önerisinin saati: ayrıştırma sırasında, ad çözümlemeden önce koşar",
        until: "orta vade: geçit ad çözümlemenin sonucunu okur (#415)",
    },
    Allowed {
        file: "crates/volt-syntax/src/parser/mmio.rs",
        function: "module_clock",
        reason: "`@mmio` açılımının saati: ayrıştırıcıda, ad çözümlemeden önce koşar",
        until: "orta vade: geçit ad çözümlemenin sonucunu okur",
    },
    Allowed {
        file: "crates/volt-syntax/src/parser/auto_contract/scan.rs",
        function: "scan_module",
        reason: "otomatik kontratların saati: ayrıştırıcıda, ad çözümlemeden önce koşar",
        until: "orta vade: geçit ad çözümlemenin sonucunu okur",
    },
    Allowed {
        file: "crates/volt-hir/src/domain/rdc/mod.rs",
        function: "without_raw_reset_unused",
        reason: "W1001 süzgeci (ham reset portu): tip denetiminden önce, çözümleme \
                 hatalıyken de koşar",
        until: "kısa vade: süzgeç tip denetiminin sonucunu okur",
    },
    Allowed {
        file: "crates/volt-hir/src/sim.rs",
        function: "check_set_port",
        reason: "test blokları (E8503): birim düzeyinde tip denetimi olmadan denetlenir",
        until: "kısa vade: test blokları birimin tip denetimini okur",
    },
    Allowed {
        file: "crates/volt-lsp/src/analysis.rs",
        function: "clock_ports",
        reason: "LSP `on ` tamamlaması: yarım kodda tip denetimi yok (editör yardımcısı)",
        until: "kalıcı: editör yardımcısı (ADR-0137 Karar 1)",
    },
];

/// Tanımın kendisi taranmaz.
const DEFINITION: &str = "crates/volt-ast/src/alias.rs";

const NAME: &str = "signal_class";

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

/// Satırın yorum dışı kısmı (`//` sonrası atılır).
fn code_of(line: &str) -> &str {
    line.find("//").map_or(line, |i| &line[..i])
}

/// `ad` kelime olarak geçiyor mu (`signal_class_x` ya da `x_signal_class`
/// değil).
fn mentions(code: &str, name: &str) -> bool {
    let is_ident = |c: char| c.is_alphanumeric() || c == '_';
    code.match_indices(name).any(|(i, _)| {
        let before = code[..i].chars().next_back();
        let after = code[i + name.len()..].chars().next();
        !before.is_some_and(is_ident) && !after.is_some_and(is_ident)
    })
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

/// Bir kaynak metindeki başvurular: içe aktarma satırları ayrı, gerisi
/// içinde bulunduğu fonksiyonla. Dönüş: (fonksiyon başvuruları, kural dışı
/// içe aktarmalar).
fn references(text: &str) -> (Vec<String>, Vec<String>) {
    let mut calls = Vec::new();
    let mut bad_imports = Vec::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        if let Some(name) = fn_name(line) {
            current = Some(name.to_string());
        }
        let code = code_of(line);
        let trimmed = code.trim_start();
        let import = trimmed.starts_with("use ") || trimmed.starts_with("pub use ");
        // `alias::*` içe aktarması çağrıları adsız bırakır; başka adla içe
        // aktarma da öyle.
        if import && code.contains("alias::*") {
            bad_imports.push(line.trim().to_string());
        }
        if !mentions(code, NAME) {
            continue;
        }
        if import {
            if code.contains(&format!("{NAME} as ")) {
                bad_imports.push(line.trim().to_string());
            }
            continue;
        }
        calls.push(
            current
                .clone()
                .unwrap_or_else(|| "<fonksiyon dışı>".to_string()),
        );
    }
    (calls, bad_imports)
}

/// Bütün `src/` dosyalarındaki (dosya, fonksiyon) başvuruları ve kural dışı
/// içe aktarmalar.
fn scan() -> (BTreeSet<(String, String)>, Vec<String>) {
    let root = repo_root();
    let mut files = Vec::new();
    let crates = std::fs::read_dir(root.join("crates")).expect("crates/ okunmalı");
    let mut dirs: Vec<PathBuf> = crates.filter_map(|e| e.ok().map(|e| e.path())).collect();
    dirs.sort();
    for dir in dirs {
        rust_files(&dir.join("src"), &mut files);
    }
    let mut found = BTreeSet::new();
    let mut bad = Vec::new();
    for path in files {
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .display()
            .to_string()
            .replace('\\', "/");
        if rel == DEFINITION {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("kaynak okunmalı");
        let (calls, imports) = references(&text);
        for f in calls {
            found.insert((rel.clone(), f));
        }
        bad.extend(imports.into_iter().map(|l| format!("{rel}: {l}")));
    }
    (found, bad)
}

/// Taramanın sonucunu izin listesiyle karşılaştırır; ADR-0125 biçiminde
/// hata metni.
fn verdict(found: &BTreeSet<(String, String)>, bad_imports: &[String]) -> Result<(), String> {
    let allowed: BTreeSet<(String, String)> = ALLOWED
        .iter()
        .map(|a| (a.file.to_string(), a.function.to_string()))
        .collect();
    let new: Vec<_> = found.difference(&allowed).collect();
    let stale: Vec<_> = allowed.difference(found).collect();
    if new.is_empty() && stale.is_empty() && bad_imports.is_empty() {
        return Ok(());
    }
    let mut m = String::new();
    let _ = writeln!(m, "sözdizimsel sınıflamanın (`volt_ast::alias::{NAME}`) çağıranları izin listesiyle uyuşmuyor");
    let _ = writeln!(m, "ne oldu:");
    for (file, function) in &new {
        let _ = writeln!(m, "  listede olmayan çağıran: {file} `{function}`");
    }
    for (file, function) in &stale {
        let _ = writeln!(m, "  listede ama artık çağırmıyor: {file} `{function}`");
    }
    for line in bad_imports {
        let _ = writeln!(m, "  çağrıyı gizleyen içe aktarma: {line}");
    }
    let _ = writeln!(
        m,
        "neden: derleyici çıktısını (üretilen dosyalar, tanılar, test sonuçları) etkileyen \
         saat/reset kararı tip denetiminden ya da ad çözümlemeden gelir; takma ad zinciri \
         aynı bilginin ikinci hesabıdır ve ondan ayrışabilir (#415). Zincir yalnız tip \
         denetimine ulaşamayan yerlerde, nedeni yazılı olarak kalır (ADR-0137)."
    );
    let _ = writeln!(
        m,
        "ne yapabilirsin: yeni kod portun sınıfını `volt_hir::port_class::DefTypes`'tan okusun \
         (`class`, `port`, `is_clock_port`, `is_raw_reset`). Yer tip denetiminden gerçekten önce \
         koşuyorsa (ayrıştırıcı, editör yardımcısı) nedenini ve onu kaldıracak adımı yazarak \
         `ALLOWED`'a ekle; ADR-0137'nin geçiş sırasına uymuyorsa önce bir ADR. Artık çağırmayan \
         girdiyi listeden sil; içe aktarmayı adıyla (`use volt_ast::alias::{NAME}`) yaz."
    );
    Err(m)
}

#[test]
fn every_caller_of_the_syntactic_class_is_on_the_allowlist() {
    let (found, bad) = scan();
    let mut report = String::new();
    for a in &ALLOWED {
        let _ = writeln!(
            report,
            "  {} `{}`: {} | {}",
            a.file, a.function, a.reason, a.until
        );
    }
    println!("izin listesi ({} çağıran):\n{report}", ALLOWED.len());
    assert!(
        !found.is_empty(),
        "tarama hiç çağıran bulmadı: yol yanlış mı?"
    );
    if let Err(m) = verdict(&found, &bad) {
        panic!("{m}");
    }
}

/// Listede olmayan çağıran ve artık çağırmayan girdi testi düşürür; mesaj
/// ADR-0125'in üç parçasını taşır.
#[test]
fn a_new_caller_or_a_stale_entry_fails_with_what_why_and_how() {
    let mut found: BTreeSet<(String, String)> = ALLOWED
        .iter()
        .map(|a| (a.file.to_string(), a.function.to_string()))
        .collect();
    assert!(verdict(&found, &[]).is_ok());
    found.insert((
        "crates/volt-sv-emit/src/lib.rs".to_string(),
        "clock_ports_of".to_string(),
    ));
    let first = found.iter().next().cloned().expect("girdi");
    found.remove(&first);
    let message = verdict(&found, &["x.rs: use volt_ast::alias::*;".to_string()])
        .expect_err("yeni çağıran testi düşürmeli");
    for part in [
        "ne oldu:",
        "neden:",
        "ne yapabilirsin:",
        "listede olmayan çağıran: crates/volt-sv-emit/src/lib.rs `clock_ports_of`",
        &format!("listede ama artık çağırmıyor: {} `{}`", first.0, first.1),
        "çağrıyı gizleyen içe aktarma: x.rs: use volt_ast::alias::*;",
    ] {
        assert!(message.contains(part), "`{part}` yok:\n{message}");
    }
}

/// Tarayıcı çağrıyı içinde bulunduğu fonksiyona bağlar; yorumdaki ad,
/// başka bir adın parçası ve adıyla içe aktarma sayılmaz; başka adla içe
/// aktarma kural dışıdır.
#[test]
fn the_scanner_finds_calls_by_function_and_ignores_comments() {
    let text = "use volt_ast::alias::{signal_class, SignalClass};\n\
                /// `signal_class` belgede\n\
                pub(super) fn first(ast: &A) -> bool {\n    \
                    // signal_class yorumda\n    \
                    let x = my_signal_class(ast);\n    \
                    signal_class(ast, t) == SignalClass::Clock\n\
                }\n\
                fn second() {\n    \
                    ports.iter().map(|p| volt_ast::alias::signal_class(a, p.ty))\n\
                }\n\
                use volt_ast::alias::signal_class as sc;\n";
    let (calls, bad) = references(text);
    assert_eq!(calls, vec!["first".to_string(), "second".to_string()]);
    assert_eq!(
        bad,
        vec!["use volt_ast::alias::signal_class as sc;".to_string()]
    );
}
