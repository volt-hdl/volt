//! Dış araç keşfi — tek kaynak (ADR-0084 §1).
//!
//! `volt test`/`volt run` (Verilator), `volt verify` (sby), `volt doctor`
//! (hepsi) ve araç bağımlı testler (`VOLT_REQUIRE_TOOLS`, ADR-0079 §3)
//! araçları buradan bulur. ADR-0084 öncesinde aynı arama beş yerde
//! kopyalıydı ve kopyalar ayrışmıştı: sürücü `.bat` ekini tanıyor, ortam
//! değişkenindeki çıplak adı tanımıyordu; testler tam tersi (ADR-0070
//! dersi: kopyalar ayrışır).
//!
//! Arama sırası: aracın ortam değişkeni (`VOLT_VERILATOR`, `VOLT_SBY`,
//! `VOLT_DOCKER`, `CC`, `CXX`) — dosya yolu ya da `PATH`'teki ad — sonra adaylar
//! `PATH`'te sırayla. Değişken bulunamayan bir yolu gösterirse arama
//! `PATH`'e düşer (eski davranış).
//!
//! `docker`/`docker_paths`: eksik Verilator/sby'yi sabitlenmiş imajda
//! koşturan köprünün dil bağımsız mekanizması (ADR-0094).

pub mod docker;
pub mod docker_paths;
mod probe;

pub use probe::{probe_version, Probe, DEFAULT_TIMEOUT};

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Volt'un tanıdığı dış araçlar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    Verilator,
    Sby,
    Yosys,
    Boolector,
    Bitwuzla,
    Yices,
    Z3,
    /// OpenSTA (`sta`) — üretilen SDC'nin ikinci ağı (ADR-0065).
    OpenSta,
    /// C derleyicisi (`CC`, gcc, cc, clang).
    Cc,
    /// C++ derleyicisi (`CXX`, g++, c++, clang++) — Verilator sürer.
    Cxx,
    Rustc,
    /// Verilator `--build` make çağırır.
    Make,
    Docker,
}

impl Tool {
    pub const ALL: [Tool; 13] = [
        Tool::Verilator,
        Tool::Sby,
        Tool::Yosys,
        Tool::Boolector,
        Tool::Bitwuzla,
        Tool::Yices,
        Tool::Z3,
        Tool::OpenSta,
        Tool::Cc,
        Tool::Cxx,
        Tool::Rustc,
        Tool::Make,
        Tool::Docker,
    ];

    /// Kararlı ad (`VOLT_REQUIRE_TOOLS`, `volt doctor --format=json`).
    pub fn name(self) -> &'static str {
        match self {
            Tool::Verilator => "verilator",
            Tool::Sby => "sby",
            Tool::Yosys => "yosys",
            Tool::Boolector => "boolector",
            Tool::Bitwuzla => "bitwuzla",
            Tool::Yices => "yices",
            Tool::Z3 => "z3",
            Tool::OpenSta => "opensta",
            Tool::Cc => "cc",
            Tool::Cxx => "cxx",
            Tool::Rustc => "rustc",
            Tool::Make => "make",
            Tool::Docker => "docker",
        }
    }

    /// Aramayı öne alan ortam değişkeni. `CC`/`CXX` yerleşik derleyici
    /// değişkenleridir.
    pub fn env_override(self) -> Option<&'static str> {
        match self {
            Tool::Verilator => Some("VOLT_VERILATOR"),
            Tool::Sby => Some("VOLT_SBY"),
            Tool::Docker => Some("VOLT_DOCKER"),
            Tool::Cc => Some("CC"),
            Tool::Cxx => Some("CXX"),
            _ => None,
        }
    }

    /// `PATH`'te sırayla denenen adlar.
    pub fn candidates(self) -> &'static [&'static str] {
        match self {
            Tool::Verilator => &["verilator"],
            Tool::Sby => &["sby"],
            Tool::Yosys => &["yosys"],
            Tool::Boolector => &["boolector"],
            Tool::Bitwuzla => &["bitwuzla"],
            Tool::Yices => &["yices-smt2", "yices"],
            Tool::Z3 => &["z3"],
            Tool::OpenSta => &["sta"],
            Tool::Cc => &["gcc", "cc", "clang"],
            Tool::Cxx => &["g++", "c++", "clang++"],
            Tool::Rustc => &["rustc"],
            Tool::Make => &["make", "gmake"],
            Tool::Docker => &["docker"],
        }
    }

    /// Sürümü soran argümanlar (`probe_version`).
    pub fn version_args(self) -> &'static [&'static str] {
        match self {
            Tool::Yosys => &["-V"],
            Tool::OpenSta => &["-version"],
            _ => &["--version"],
        }
    }
}

/// Aramanın ortamdan aldığı girdiler; testler süreç ortamına dokunmadan
/// kurar (`manifest_search::SearchEnv` emsali).
#[derive(Clone, Debug, Default)]
pub struct ToolEnv {
    /// `PATH` değeri.
    pub path: Option<OsString>,
    /// Aracın ortam değişkeni değeri (`Tool::env_override`).
    pub overrides: Vec<(Tool, OsString)>,
}

impl ToolEnv {
    /// Süreç ortamından okur.
    pub fn from_process() -> Self {
        Self {
            path: std::env::var_os("PATH"),
            overrides: Tool::ALL
                .iter()
                .filter_map(|t| Some((*t, std::env::var_os(t.env_override()?)?)))
                .collect(),
        }
    }

    fn override_of(&self, tool: Tool) -> Option<&OsString> {
        self.overrides
            .iter()
            .find(|(t, v)| *t == tool && !v.is_empty())
            .map(|(_, v)| v)
    }
}

/// Windows'ta (ve eklenmiş betiklerde) denenen ekler. `.bat`/`.cmd`
/// testlerin sahte araçlarıdır; Unix'te `sby.exe` gibi bir ad zararsızdır.
const EXTENSIONS: [&str; 4] = ["", ".exe", ".bat", ".cmd"];

/// Aracın yolu, süreç ortamıyla.
pub fn find(tool: Tool) -> Option<PathBuf> {
    find_in(tool, &ToolEnv::from_process())
}

/// `find`in ortamı açıkça verilen biçimi.
pub fn find_in(tool: Tool, env: &ToolEnv) -> Option<PathBuf> {
    if let Some(value) = env.override_of(tool) {
        let as_path = PathBuf::from(value);
        if as_path.is_file() {
            return Some(as_path);
        }
        if let Some(found) = on_path(&value.to_string_lossy(), env.path.as_deref()) {
            return Some(found);
        }
    }
    tool.candidates()
        .iter()
        .find_map(|name| on_path(name, env.path.as_deref()))
}

/// `PATH` üzerinde ad; birden çok bileşenli ad doğrudan dosya yoludur.
fn on_path(name: &str, path: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
    let direct = Path::new(name);
    if direct.components().count() > 1 {
        return direct.is_file().then(|| direct.to_path_buf());
    }
    std::env::split_paths(path?).find_map(|dir| {
        EXTENSIONS
            .iter()
            .map(|ext| dir.join(format!("{name}{ext}")))
            .find(|p| p.is_file())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("volt-tools-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dizini");
        dir
    }

    fn env_with_path(dir: &Path) -> ToolEnv {
        ToolEnv {
            path: Some(dir.as_os_str().to_owned()),
            overrides: Vec::new(),
        }
    }

    #[test]
    fn names_are_unique_and_stable() {
        let mut names: Vec<&str> = Tool::ALL.iter().map(|t| t.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), Tool::ALL.len());
        assert_eq!(Tool::Verilator.name(), "verilator");
        assert_eq!(Tool::Cxx.name(), "cxx");
    }

    #[test]
    fn empty_path_finds_nothing() {
        let env = ToolEnv::default();
        assert!(Tool::ALL.iter().all(|t| find_in(*t, &env).is_none()));
    }

    #[test]
    fn finds_every_extension_on_path() {
        for ext in EXTENSIONS {
            let dir = temp_dir(&format!("ext{}", ext.trim_start_matches('.')));
            std::fs::write(dir.join(format!("verilator{ext}")), "").expect("yaz");
            assert_eq!(
                find_in(Tool::Verilator, &env_with_path(&dir)),
                Some(dir.join(format!("verilator{ext}"))),
                "ek '{ext}'"
            );
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn candidates_are_tried_in_order() {
        let dir = temp_dir("order");
        std::fs::write(dir.join("clang"), "").expect("yaz");
        std::fs::write(dir.join("cc"), "").expect("yaz");
        assert_eq!(
            find_in(Tool::Cc, &env_with_path(&dir)),
            Some(dir.join("cc"))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn override_file_path_wins_over_path() {
        let dir = temp_dir("override");
        let custom = dir.join("my-sby");
        std::fs::write(&custom, "").expect("yaz");
        std::fs::write(dir.join("sby"), "").expect("yaz");
        let mut env = env_with_path(&dir);
        env.overrides
            .push((Tool::Sby, custom.clone().into_os_string()));
        assert_eq!(find_in(Tool::Sby, &env), Some(custom));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn override_bare_name_is_looked_up_on_path() {
        let dir = temp_dir("bare");
        std::fs::write(dir.join("clang"), "").expect("yaz");
        std::fs::write(dir.join("gcc"), "").expect("yaz");
        let mut env = env_with_path(&dir);
        env.overrides.push((Tool::Cc, "clang".into()));
        assert_eq!(find_in(Tool::Cc, &env), Some(dir.join("clang")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_override_falls_back_to_path() {
        let dir = temp_dir("fallback");
        std::fs::write(dir.join("verilator"), "").expect("yaz");
        let mut env = env_with_path(&dir);
        env.overrides.push((
            Tool::Verilator,
            dir.join("no-such-verilator").into_os_string(),
        ));
        assert_eq!(find_in(Tool::Verilator, &env), Some(dir.join("verilator")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_override_is_ignored() {
        let dir = temp_dir("emptyvar");
        std::fs::write(dir.join("sby"), "").expect("yaz");
        let mut env = env_with_path(&dir);
        env.overrides.push((Tool::Sby, OsString::new()));
        assert_eq!(find_in(Tool::Sby, &env), Some(dir.join("sby")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn directories_are_not_tools() {
        let dir = temp_dir("dir");
        std::fs::create_dir_all(dir.join("yosys")).expect("dizin");
        assert_eq!(find_in(Tool::Yosys, &env_with_path(&dir)), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
