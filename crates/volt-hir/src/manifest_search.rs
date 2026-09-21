//! `Volt.toml` araması ve tavanı (ADR-0061).
//!
//! Arama kaynak dosyanın dizininden yukarı çıkar ve ilk `Volt.toml`'da
//! durur. ADR-0042'den beri tavansızdı: kendi manifest'i olmayan bir
//! proje üst dizindeki başıboş bir `Volt.toml`'u kök sayabiliyordu.
//! Tavan iki sınırdır; hangisi önce gelirse:
//!
//!   * **Git kökü** — `.git` (dizin ya da worktree/alt modül dosyası)
//!     taşıyan dizin. Kapsayıcıdır: kökün kendi `Volt.toml`'u okunur,
//!     üstü okunmaz.
//!   * **Ev dizini** (`HOME`, `USERPROFILE`) — dışlayıcıdır: ev
//!     dizinindeki `Volt.toml` yalnız arama zaten oradan başladıysa
//!     okunur; alt dizindeki bir proje onu kök sayamaz.
//!
//! `VOLT_MANIFEST_DIR` aramayı tümüyle geçersiz kılar: o dizindeki
//! `Volt.toml` kullanılır, yoksa manifest yoktur (yukarı aranmaz).
//! Sürücü (`Manifest::discover`), test veri kökü ve LSP (`UnenforcedLint`)
//! aynı kararı buradan alır.

use std::path::{Path, PathBuf};

/// Paket manifest'inin dosya adı.
pub const MANIFEST_FILE: &str = "Volt.toml";

/// Aramayı geçersiz kılan ortam değişkeni.
pub const MANIFEST_DIR_ENV: &str = "VOLT_MANIFEST_DIR";

/// Ev dizinini veren ortam değişkenleri (Unix, Windows).
const HOME_ENVS: [&str; 2] = ["HOME", "USERPROFILE"];

/// Aramanın ortamdan aldığı girdiler; testler süreç ortamına
/// dokunmadan kurar.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchEnv {
    /// `VOLT_MANIFEST_DIR` (boş değer yok sayılır).
    pub override_dir: Option<PathBuf>,
    /// Ev dizinleri — dışlayıcı tavan.
    pub homes: Vec<PathBuf>,
}

impl SearchEnv {
    /// Süreç ortamından okur.
    pub fn from_process() -> Self {
        let non_empty = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty());
        Self {
            override_dir: non_empty(MANIFEST_DIR_ENV).map(PathBuf::from),
            homes: HOME_ENVS
                .iter()
                .filter_map(|name| non_empty(name))
                .map(PathBuf::from)
                .collect(),
        }
    }
}

/// Aramanın manifest bulamadan durduğu yer (E1011 notu bunu söyler).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchStop {
    /// `VOLT_MANIFEST_DIR` bu dizini gösteriyor, içinde `Volt.toml` yok.
    Override(PathBuf),
    /// Git kökü (kapsayıcı tavan).
    GitRoot(PathBuf),
    /// Ev dizini (dışlayıcı tavan).
    Home(PathBuf),
    /// Dosya sistemi kökü ya da göreli yolun başı — tavan görülmedi.
    Exhausted,
}

/// Manifest'in dizinini (`Ok`) ya da aramanın durduğu yeri (`Err`) verir.
pub fn find_manifest_dir(start: &Path) -> Result<PathBuf, SearchStop> {
    find_manifest_dir_in(start, &SearchEnv::from_process())
}

/// `find_manifest_dir`in ortamı açıkça verilen biçimi.
pub fn find_manifest_dir_in(start: &Path, env: &SearchEnv) -> Result<PathBuf, SearchStop> {
    if let Some(dir) = &env.override_dir {
        return if dir.join(MANIFEST_FILE).is_file() {
            Ok(dir.clone())
        } else {
            Err(SearchStop::Override(dir.clone()))
        };
    }
    let homes: Vec<PathBuf> = env.homes.iter().map(|h| canonical(h)).collect();
    let mut cur = Some(start);
    while let Some(dir) = cur {
        let here = canonical(dir);
        let is_home = homes.contains(&here);
        if is_home && dir != start {
            return Err(SearchStop::Home(dir.to_path_buf()));
        }
        if dir.join(MANIFEST_FILE).is_file() {
            return Ok(dir.to_path_buf());
        }
        if dir.join(".git").exists() {
            return Err(SearchStop::GitRoot(dir.to_path_buf()));
        }
        if is_home {
            return Err(SearchStop::Home(dir.to_path_buf()));
        }
        cur = dir.parent();
    }
    Err(SearchStop::Exhausted)
}

/// Karşılaştırma için kanonik yol; boş göreli yol çalışma dizinidir.
/// Çözülemeyen yol olduğu gibi kalır (sözcüksel karşılaştırma).
fn canonical(dir: &Path) -> PathBuf {
    let dir = if dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        dir
    };
    std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test başına ayrı geçici ağaç; `.git` taşımayan bir üst dizinde.
    fn tree(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("volt-manifest-search-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("geçici dizin");
        dir
    }

    fn touch_manifest(dir: &Path) {
        std::fs::create_dir_all(dir).expect("dizin");
        std::fs::write(dir.join(MANIFEST_FILE), "[package]\nname = \"p\"\n").expect("yaz");
    }

    fn mkdir(dir: &Path) -> PathBuf {
        std::fs::create_dir_all(dir).expect("dizin");
        dir.to_path_buf()
    }

    fn no_env() -> SearchEnv {
        SearchEnv::default()
    }

    #[test]
    fn manifest_in_the_start_directory_is_found() {
        let tmp = tree("start");
        touch_manifest(&tmp);
        assert_eq!(find_manifest_dir_in(&tmp, &no_env()), Ok(tmp.clone()));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn manifest_at_the_git_root_is_found_from_a_subdirectory() {
        let tmp = tree("git-root");
        let repo = tmp.join("repo");
        mkdir(&repo.join(".git"));
        touch_manifest(&repo);
        let src = mkdir(&repo.join("src"));
        assert_eq!(find_manifest_dir_in(&src, &no_env()), Ok(repo));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn stray_manifest_above_the_git_root_is_ignored() {
        // tmp/Volt.toml, tmp/repo/.git/, tmp/repo/src/main.volt
        let tmp = tree("stray-git");
        touch_manifest(&tmp);
        let repo = tmp.join("repo");
        mkdir(&repo.join(".git"));
        let src = mkdir(&repo.join("src"));
        assert_eq!(
            find_manifest_dir_in(&src, &no_env()),
            Err(SearchStop::GitRoot(repo))
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn git_file_of_a_worktree_or_submodule_also_stops_the_search() {
        let tmp = tree("git-file");
        touch_manifest(&tmp);
        let sub = mkdir(&tmp.join("sub"));
        std::fs::write(sub.join(".git"), "gitdir: ../.git/modules/sub\n").expect("yaz");
        assert_eq!(
            find_manifest_dir_in(&sub, &no_env()),
            Err(SearchStop::GitRoot(sub.clone()))
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn manifest_in_the_home_directory_is_not_a_project_root() {
        // Senaryo: ~/Volt.toml unutulmuş, ~/projects/foo'nun manifest'i yok.
        let tmp = tree("home");
        let home = tmp.join("home");
        touch_manifest(&home);
        let foo = mkdir(&home.join("projects").join("foo"));
        let env = SearchEnv {
            override_dir: None,
            homes: vec![home.clone()],
        };
        assert_eq!(
            find_manifest_dir_in(&foo, &env),
            Err(SearchStop::Home(home))
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn home_directory_manifest_is_read_when_the_search_starts_there() {
        let tmp = tree("home-start");
        let home = tmp.join("home");
        touch_manifest(&home);
        let env = SearchEnv {
            override_dir: None,
            homes: vec![home.clone()],
        };
        assert_eq!(find_manifest_dir_in(&home, &env), Ok(home));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn override_directory_wins_over_a_nearer_manifest_and_the_ceiling() {
        let tmp = tree("override");
        touch_manifest(&tmp);
        let repo = tmp.join("repo");
        mkdir(&repo.join(".git"));
        touch_manifest(&repo);
        let env = SearchEnv {
            override_dir: Some(tmp.clone()),
            homes: Vec::new(),
        };
        assert_eq!(find_manifest_dir_in(&repo, &env), Ok(tmp.clone()));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn override_directory_without_a_manifest_means_no_manifest() {
        let tmp = tree("override-empty");
        touch_manifest(&tmp);
        let empty = mkdir(&tmp.join("empty"));
        let env = SearchEnv {
            override_dir: Some(empty.clone()),
            homes: Vec::new(),
        };
        // Yukarı aranmaz: tmp/Volt.toml bulunmaz.
        assert_eq!(
            find_manifest_dir_in(&empty, &env),
            Err(SearchStop::Override(empty.clone()))
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
