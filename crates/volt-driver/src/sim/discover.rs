//! `volt test` test dosyası keşfi (ADR-0089).
//!
//! * Proje (Volt.toml, ADR-0061 araması) varsa **proje kökünden
//!   özyinelemeli**: `*_test.volt` her alt dizinde bulunur; `[test] paths`
//!   verilmişse yalnız o dizinler (kökten göreli) taranır.
//! * Proje yoksa eski davranış: yalnız çalışma dizini (özyinelemesiz).
//!
//! Atlanan dizinler: `.` ile başlayanlar (`.git`), `build` ve `target`
//! (her düzeyde — derleme çıktısı), kendi `Volt.toml`'u olan alt dizinler
//! (iç içe başka proje), kökteki `.gitignore`'un düz dizin girdileri
//! (`build/`, `/out`, `vendor`; joker ve `!` içeren satırlar yok sayılır).
//! Sembolik bağlantılar izlenmez (döngü yok).

use std::path::{Component, Path, PathBuf};

use volt_hir::unit_load::Manifest;

/// Keşfin dosya süzgeci.
type Keep<'a> = &'a dyn Fn(&Path) -> bool;

/// Özyineleme sınırı: bundan derin dizin taranmaz (patolojik ağaçlar).
const MAX_DEPTH: usize = 32;

/// Keşfedilen test dosyaları, çalışma dizinine göre yollarla, sıralı.
pub(super) fn discover_test_files() -> Vec<PathBuf> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    match Manifest::discover(&cwd) {
        Some(m) => project_test_files(&m, &cwd),
        None => flat_test_files(Path::new(".")),
    }
}

/// Tek dizindeki `*_test.volt` dosyaları (özyinelemesiz, eski kural).
fn flat_test_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_test_file(p))
        .collect();
    files.sort();
    files
}

pub(crate) fn is_test_file(p: &Path) -> bool {
    p.file_name()
        .is_some_and(|n| n.to_string_lossy().ends_with("_test.volt"))
}

/// Projenin test dosyaları, çalışma dizinine göre (argümansız `volt
/// check` de denetler, `volt test` ile aynı keşif).
pub(crate) fn project_test_files_here(m: &Manifest) -> Vec<PathBuf> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    project_test_files(m, &cwd)
        .into_iter()
        .map(|p| {
            p.strip_prefix(".")
                .map_or_else(|_| p.clone(), Path::to_path_buf)
        })
        .collect()
}

/// Projenin test dosyaları: kök (ya da `[test] paths`) altında
/// özyinelemeli; yollar `cwd`'ye göreli.
fn project_test_files(m: &Manifest, cwd: &Path) -> Vec<PathBuf> {
    let starts: Vec<PathBuf> = match &m.test_paths {
        Some(paths) => paths.clone(),
        None => vec![PathBuf::new()],
    };
    project_files(m, cwd, &starts, &is_test_file)
}

/// Proje kökünden göreli `starts` altında `keep`'i sağlayan dosyalar —
/// test keşfiyle aynı atlama kuralları (ADR-0089); proje kipi kaynakları
/// ve `volt test --watch` de bunu kullanır (ADR-0095). Yollar `cwd`'ye
/// göreli, sıralı, tekil.
pub(crate) fn project_files(
    m: &Manifest,
    cwd: &Path,
    starts: &[PathBuf],
    keep: &dyn Fn(&Path) -> bool,
) -> Vec<PathBuf> {
    let root = m.root.canonicalize().unwrap_or_else(|_| m.root.clone());
    let ignored = gitignored_dirs(&root);
    let mut found = Vec::new();
    // `src = "."` gibi `.` parçaları yola sızmasın (`./counter.volt`).
    let starts = starts.iter().map(|p| {
        root.join(p)
            .components()
            .filter(|c| !matches!(c, Component::CurDir))
            .collect::<PathBuf>()
    });
    for start in starts {
        if start.is_file() {
            if keep(&start) {
                found.push(start);
            }
            continue;
        }
        walk(&root, &start, (&ignored, keep), 0, &mut found);
    }
    found.sort();
    found.dedup();
    let cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    found.iter().map(|f| relative_to(f, &cwd)).collect()
}

fn walk(
    root: &Path,
    dir: &Path,
    (ignored, keep): (&[Ignore], Keep<'_>),
    depth: usize,
    out: &mut Vec<PathBuf>,
) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if kind.is_file() {
            if keep(&path) {
                out.push(path);
            }
        } else if kind.is_dir() && !skip_dir(root, &path, ignored) {
            walk(root, &path, (ignored, keep), depth + 1, out);
        }
    }
}

/// Taranmayan dizin mi? (gizli, derleme çıktısı, iç içe proje, .gitignore)
fn skip_dir(root: &Path, dir: &Path, ignored: &[Ignore]) -> bool {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if name.starts_with('.') || name == "build" || name == "target" {
        return true;
    }
    if dir.join(volt_hir::MANIFEST_FILE).is_file() {
        return true;
    }
    let rel = dir.strip_prefix(root).unwrap_or(dir);
    ignored.iter().any(|ig| ig.matches(rel))
}

/// Kökteki `.gitignore`'un düz dizin girdisi.
#[derive(Debug, PartialEq, Eq)]
enum Ignore {
    /// `/out`, `docs/gen` — kökten göreli tam yol.
    Anchored(PathBuf),
    /// `vendor` — her düzeydeki aynı adlı dizin.
    Name(String),
}

impl Ignore {
    fn matches(&self, rel: &Path) -> bool {
        match self {
            Ignore::Anchored(p) => rel == p,
            Ignore::Name(n) => rel.file_name().is_some_and(|f| f.to_string_lossy() == *n),
        }
    }
}

fn gitignored_dirs(root: &Path) -> Vec<Ignore> {
    std::fs::read_to_string(root.join(".gitignore"))
        .map(|t| parse_gitignore(&t))
        .unwrap_or_default()
}

/// Düz girdiler: yorum, olumsuzlama (`!`) ve joker (`*?[`) içeren
/// satırlar yok sayılır (git'in tam desen dili desteklenmez).
fn parse_gitignore(text: &str) -> Vec<Ignore> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('!'))
        .filter(|l| !l.contains(['*', '?', '[', '\\']))
        .filter_map(|l| {
            let anchored = l.starts_with('/') || l.trim_end_matches('/').contains('/');
            let clean = l.trim_start_matches('/').trim_end_matches('/');
            if clean.is_empty() {
                return None;
            }
            Some(if anchored {
                Ignore::Anchored(PathBuf::from(clean))
            } else {
                Ignore::Name(clean.to_string())
            })
        })
        .collect()
}

/// `path`'in `base`'e göre yolu (ikisi de mutlak): altındaysa `./a/b`
/// (eski `read_dir(".")` biçimi), değilse `../a/b`.
fn relative_to(path: &Path, base: &Path) -> PathBuf {
    let p: Vec<Component> = path.components().collect();
    let b: Vec<Component> = base.components().collect();
    let common = p.iter().zip(&b).take_while(|(x, y)| x == y).count();
    if common == 0 {
        return path.to_path_buf();
    }
    let ups = b.len() - common;
    let mut out = PathBuf::from(if ups == 0 { "." } else { ".." });
    for _ in 1..ups.max(1) {
        out.push("..");
    }
    for c in &p[common..] {
        out.push(c.as_os_str());
    }
    out
}

/// Simülasyon çıktı dizini anahtarı: test dosyasının yolu, `.volt`'suz;
/// `.` atılır, `..` → `up` (aynı adlı iki alt dizin testi çakışmaz).
/// Mutlak yol yalnız dosya adını verir (önceki biçim).
pub(super) fn sim_key(file: &Path) -> PathBuf {
    let stem = file
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    if file.is_absolute() {
        return PathBuf::from(stem);
    }
    let mut key = PathBuf::new();
    if let Some(parent) = file.parent() {
        for c in parent.components() {
            match c {
                Component::Normal(s) => key.push(s),
                Component::ParentDir => key.push("up"),
                _ => {}
            }
        }
    }
    key.push(stem);
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gitignore_keeps_only_plain_directory_entries() {
        let ig = parse_gitignore(
            "# build output\nbuild/\n/out\ndocs/gen/\nvendor\n*.log\n!keep\nsrc/**\n",
        );
        assert_eq!(
            ig,
            [
                Ignore::Name("build".into()),
                Ignore::Anchored("out".into()),
                Ignore::Anchored("docs/gen".into()),
                Ignore::Name("vendor".into()),
            ]
        );
        assert!(Ignore::Name("vendor".into()).matches(Path::new("a/vendor")));
        assert!(!Ignore::Anchored("out".into()).matches(Path::new("a/out")));
    }

    #[test]
    fn relative_paths_walk_up_with_dotdot() {
        let root = Path::new(if cfg!(windows) { "C:\\p" } else { "/p" });
        let file = root.join("tests").join("a_test.volt");
        assert_eq!(
            relative_to(&file, root),
            Path::new(".").join("tests").join("a_test.volt")
        );
        let sub = root.join("rtl");
        assert_eq!(
            relative_to(&file, &sub),
            Path::new("..").join("tests").join("a_test.volt")
        );
    }

    #[test]
    fn sim_key_separates_same_named_tests_in_two_directories() {
        assert_eq!(
            sim_key(Path::new("./counter_test.volt")),
            Path::new("counter_test")
        );
        assert_eq!(
            sim_key(Path::new("./a/fifo_test.volt")),
            Path::new("a").join("fifo_test")
        );
        assert_ne!(
            sim_key(Path::new("./a/fifo_test.volt")),
            sim_key(Path::new("./b/fifo_test.volt"))
        );
        assert_eq!(
            sim_key(Path::new("../x/t_test.volt")),
            Path::new("up").join("x").join("t_test")
        );
    }
}
