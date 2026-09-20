//! `volt test` dosya keşfi: süzgeç çözümü, `*_test.volt` taraması ve
//! kardeş (`X_test.volt` → `X.volt`) dosya kuralı.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use volt_diagnostics::lstr;

/// Filtre bir .volt dosyası mı, ad süzgeci mi? (dosyalar, ad süzgeci)
pub(super) fn resolve_files(
    filter: Option<&str>,
) -> Result<(Vec<PathBuf>, Option<&str>), ExitCode> {
    match filter {
        Some(f) if f.ends_with(".volt") => {
            let p = PathBuf::from(f);
            if !p.is_file() {
                eprintln!(
                    "{}",
                    lstr!(
                        en: "error: cannot read '{f}': file not found";
                        tr: "hata: '{f}' okunamadı: dosya yok"
                    )
                );
                return Err(ExitCode::from(3));
            }
            Ok((vec![p], None))
        }
        other => Ok((discover_test_files(), other)),
    }
}

/// `X_test.volt` için kardeş `X.volt` yolu (varsa).
pub(super) fn sibling_path(file: &Path) -> Option<PathBuf> {
    let stem = file.file_stem()?.to_string_lossy();
    let base = stem.strip_suffix("_test")?;
    let sibling = file.with_file_name(format!("{base}.volt"));
    sibling.is_file().then_some(sibling)
}

/// Çalışma dizinindeki `*_test.volt` dosyaları (ad sırasıyla).
fn discover_test_files() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(".")
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().ends_with("_test.volt"))
        })
        .collect();
    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sibling_path_only_for_test_suffix() {
        // Kardeşi olmayan adlar: _test soneki yoksa None.
        assert!(sibling_path(Path::new("counter.volt")).is_none());
        // _test soneki var ama kardeş dosya diskte yok → None.
        assert!(sibling_path(Path::new("nonexistent_test.volt")).is_none());
    }

    #[test]
    fn sibling_path_finds_existing_sibling() {
        let dir = std::env::temp_dir().join(format!("volt-sim-build-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dizini");
        std::fs::write(dir.join("counter.volt"), "").expect("yaz");

        let found = sibling_path(&dir.join("counter_test.volt"));

        assert_eq!(found, Some(dir.join("counter.volt")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolve_files_missing_volt_file_is_an_error() {
        assert!(resolve_files(Some("volt-no-such-file_test.volt")).is_err());
    }

    #[test]
    fn resolve_files_treats_other_text_as_name_filter() {
        let (_, name_filter) = resolve_files(Some("uart")).expect("süzgeç");
        assert_eq!(name_filter, Some("uart"));
    }
}
