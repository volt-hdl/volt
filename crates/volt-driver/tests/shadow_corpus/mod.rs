//! Gölge testlerinin ortak girdisi (ADR-0136 Aşama 1, ADR-0137): derleyicinin
//! iki hesabını karşılaştıran testler aynı birimleri aynı yoldan yükler.
//!
//! Girdi: `tests/fixtures`, `tests/ui/pass`, `tests/ui/multifile`,
//! `tests/real_bugs`, `examples`, `templates`, `demo` altındaki her `.volt`
//! dosyası ve kitabın `volt` blokları (`book/tools/check_book.py` gibi).
//! Bir birim sürücünün yoluyla yüklenir (`load_unit`, `check_imports`,
//! `analyze_unit`); ayrıştırma, import ya da anlamsal hatası olan birim
//! derlenmemiş sayılır ve atlanır.

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use volt_diagnostics::Diagnostic;
use volt_hir::unit_load::LoadedUnit;
use volt_hir::AnalysisResult;

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Gölgelerin dosya girdisi, depo köküne göre.
pub const CORPUS_DIRS: [&str; 7] = [
    "tests/fixtures",
    "tests/ui/pass",
    "tests/ui/multifile",
    "tests/real_bugs",
    "examples",
    "templates",
    "demo",
];

/// [`CORPUS_DIRS`]'taki `.volt` dosyaları, dizin sırasıyla ve her dizinde
/// sıralı.
pub fn corpus_files() -> Vec<PathBuf> {
    let root = repo_root();
    let mut files = Vec::new();
    for dir in CORPUS_DIRS {
        volt_files(&root.join(dir), &mut files);
    }
    files
}

/// Dizindeki `.volt` dosyaları (alt dizinler dahil), sıralı.
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

/// Dosyanın depo köküne göre adı (`/` ayraçlı), raporlar için.
pub fn display_name(main: &Path) -> String {
    let root = repo_root();
    main.strip_prefix(&root)
        .unwrap_or(main)
        .display()
        .to_string()
        .replace('\\', "/")
}

/// Hata (uyarı değil) taşıyan tanı var mı.
pub fn has_error(d: &[Diagnostic]) -> bool {
    d.iter().any(|d| !d.code.is_warning())
}

/// Derlenen birim: ayrıştırma, import ve anlamsal analiz hatasız.
pub struct Analyzed {
    pub unit: LoadedUnit,
    pub analysis: AnalysisResult,
}

/// Atlanan birim: neden ve tanıları.
pub struct Skipped {
    pub reason: &'static str,
    pub diagnostics: Vec<Diagnostic>,
}

/// Birimi sürücünün yoluyla yükler ve analiz eder.
pub fn analyze(main: &Path) -> Result<Analyzed, Skipped> {
    let skipped = |reason, diagnostics: &[Diagnostic]| Skipped {
        reason,
        diagnostics: diagnostics.to_vec(),
    };
    let Ok(unit) = volt_hir::unit_load::load_unit(main) else {
        return Err(skipped("okunamadı", &[]));
    };
    if has_error(&unit.parsed.diagnostics) || has_error(&unit.diagnostics) {
        return Err(skipped("ayrıştırma hatası", &unit.parsed.diagnostics));
    }
    let imports = volt_hir::check_imports(&unit.parsed.ast, &unit.info);
    if has_error(&imports.diagnostics) {
        return Err(skipped("import hatası", &imports.diagnostics));
    }
    let analysis = volt_hir::analyze_unit(&unit.parsed.ast, &imports.scopes);
    if analysis.has_errors() {
        return Err(skipped("anlamsal hata", &analysis.diagnostics));
    }
    Ok(Analyzed { unit, analysis })
}

/// Kitabın `volt` blokları, `book/tools/check_book.py`'nin yaptığı gibi:
/// `SUMMARY.md` sırasıyla; adsız blok tek başına, `file=` adlı bloklar
/// kitabın dizini başına ortak bir çalışma dizininde; `should_fail` olan
/// atlanır. Yazılan her blok o anda `each`'e verilir. Dönüş: blok sayısı.
pub fn book_blocks(work: &Path, each: &mut dyn FnMut(&Path)) -> usize {
    let src = repo_root().join("book/src");
    let summary = std::fs::read_to_string(src.join("SUMMARY.md")).expect("SUMMARY.md okunmalı");
    let mut pages = vec![src.join("SUMMARY.md")];
    for part in summary.split("](").skip(1) {
        if let Some(end) = part.find(".md)") {
            pages.push(src.join(&part[..end + 3]));
        }
    }
    let mut workdirs: HashMap<PathBuf, PathBuf> = HashMap::new();
    let mut blocks = 0;
    for page in pages {
        let Ok(text) = std::fs::read_to_string(&page) else {
            continue;
        };
        let parent = page.parent().map(Path::to_path_buf).unwrap_or_default();
        let next = workdirs.len();
        let workdir = workdirs
            .entry(parent)
            .or_insert_with(|| work.join(format!("part-{next}")))
            .clone();
        std::fs::create_dir_all(&workdir).expect("çalışma dizini");
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            let trimmed = lines[i].trim_start();
            let fence: String = trimmed
                .chars()
                .take_while(|&c| c == '`' || c == '~')
                .collect();
            if fence.len() < 3 || lines[i].len() - trimmed.len() > 3 {
                i += 1;
                continue;
            }
            let info = trimmed[fence.len()..].trim();
            let mut body = String::new();
            i += 1;
            while i < lines.len() {
                let close = lines[i].trim();
                if close.starts_with(&fence)
                    && close.trim_matches(fence.as_bytes()[0] as char).is_empty()
                {
                    break;
                }
                body.push_str(lines[i]);
                body.push('\n');
                i += 1;
            }
            i += 1;
            let parts: Vec<&str> = info
                .split(',')
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .collect();
            if parts.first() != Some(&"volt") || parts.iter().any(|p| p.starts_with("should_fail"))
            {
                continue;
            }
            let named = parts.iter().find_map(|p| p.strip_prefix("file="));
            let path = match named {
                Some(name) => workdir.join(name),
                None => {
                    let dir = work.join(format!("unnamed-{blocks}"));
                    std::fs::create_dir_all(&dir).expect("blok dizini");
                    dir.join("block.volt")
                }
            };
            std::fs::write(&path, &body).expect("blok yazılmalı");
            blocks += 1;
            each(&path);
        }
    }
    blocks
}

/// Bütün girdi: dosyalar ve kitap blokları, her biri `each`'e. Kitap
/// blokları geçici bir dizine yazılır, sonra silinir (`tag` dizin adını
/// testler arasında ayırır). Dönüş: (dosya sayısı, kitap bloğu sayısı).
pub fn for_each_input(tag: &str, each: &mut dyn FnMut(&Path)) -> (usize, usize) {
    let files = corpus_files();
    for file in &files {
        each(file);
    }
    let work = std::env::temp_dir().join(format!("volt-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    let book = book_blocks(&work, each);
    let _ = std::fs::remove_dir_all(&work);
    (files.len(), book)
}
