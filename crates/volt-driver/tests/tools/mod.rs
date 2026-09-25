//! Araç bağımlı testlerin ortak araç araması (ADR-0079 §3).
//!
//! Araç yoksa test varsayılan olarak ATLANIR (geliştirici makinesi, araçsız
//! CI işi). `VOLT_REQUIRE_TOOLS` virgüllü bir araç listesidir (ya da `all`):
//! listedeki araç bulunamazsa test atlanmaz, DÜŞER — aracın kurulu olması
//! gereken CI işlerinde kurulum sessizce bozulursa testler yeşil kalmasın.
//! Listede bilinmeyen ad da düşürür: yazım hatası hiçbir aracı sessizce
//! zorunluluktan çıkarmasın.

// Her test ikilisi bu modülün yalnız bir kısmını kullanır.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// Ortam değişkeni: zorunlu araçlar (`verilator,sby,...` ya da `all`).
pub const REQUIRE_ENV: &str = "VOLT_REQUIRE_TOOLS";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Verilator,
    Sby,
    Yosys,
    /// C derleyicisi (`CC`, gcc, cc, clang).
    Cc,
    /// C++ derleyicisi (`CXX`, g++, c++, clang++).
    Cxx,
    Rustc,
}

impl Tool {
    pub const ALL: [Tool; 6] = [
        Tool::Verilator,
        Tool::Sby,
        Tool::Yosys,
        Tool::Cc,
        Tool::Cxx,
        Tool::Rustc,
    ];

    /// `VOLT_REQUIRE_TOOLS` içindeki ad.
    pub fn name(self) -> &'static str {
        match self {
            Tool::Verilator => "verilator",
            Tool::Sby => "sby",
            Tool::Yosys => "yosys",
            Tool::Cc => "cc",
            Tool::Cxx => "cxx",
            Tool::Rustc => "rustc",
        }
    }

    /// Volt'un kendi araç araması da bu değişkene bakar (sim, verify);
    /// `CC`/`CXX` yerleşik derleyici değişkenleridir.
    fn env_override(self) -> Option<&'static str> {
        match self {
            Tool::Verilator => Some("VOLT_VERILATOR"),
            Tool::Sby => Some("VOLT_SBY"),
            Tool::Cc => Some("CC"),
            Tool::Cxx => Some("CXX"),
            Tool::Yosys | Tool::Rustc => None,
        }
    }

    fn candidates(self) -> &'static [&'static str] {
        match self {
            Tool::Verilator => &["verilator"],
            Tool::Sby => &["sby"],
            Tool::Yosys => &["yosys"],
            Tool::Cc => &["gcc", "cc", "clang"],
            Tool::Cxx => &["g++", "c++", "clang++"],
            Tool::Rustc => &["rustc"],
        }
    }
}

/// `PATH` üzerinde ad (Windows'ta `.exe`/`.cmd` ekiyle de).
fn on_path(name: &str) -> Option<PathBuf> {
    let direct = Path::new(name);
    if direct.components().count() > 1 {
        return direct.is_file().then(|| direct.to_path_buf());
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|dir| {
        ["", ".exe", ".cmd"]
            .iter()
            .map(|ext| dir.join(format!("{name}{ext}")))
            .find(|p| p.is_file())
    })
}

/// Aracın yolu: önce ortam değişkeni (dosya yolu ya da PATH'teki ad),
/// sonra adaylar PATH'te sırayla.
pub fn find(tool: Tool) -> Option<PathBuf> {
    if let Some(var) = tool.env_override() {
        if let Some(value) = std::env::var_os(var).filter(|v| !v.is_empty()) {
            let as_path = PathBuf::from(&value);
            if as_path.is_file() {
                return Some(as_path);
            }
            if let Some(found) = on_path(&value.to_string_lossy()) {
                return Some(found);
            }
        }
    }
    tool.candidates().iter().find_map(|name| on_path(name))
}

/// `VOLT_REQUIRE_TOOLS` değerini ayrıştırır; bilinmeyen ad `Err`.
pub fn parse_required(value: &str) -> Result<Vec<Tool>, String> {
    let mut tools = Vec::new();
    for raw in value.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        if raw == "all" || raw == "1" {
            tools.extend(Tool::ALL);
            continue;
        }
        match Tool::ALL.iter().find(|t| t.name() == raw) {
            Some(t) => tools.push(*t),
            None => {
                return Err(format!(
                    "{REQUIRE_ENV}: bilinmeyen araç '{raw}' (geçerli: {}, all)",
                    Tool::ALL.map(Tool::name).join(", ")
                ))
            }
        }
    }
    Ok(tools)
}

/// Araç `VOLT_REQUIRE_TOOLS` ile zorunlu mu? Bozuk değer testi düşürür.
pub fn is_required(tool: Tool) -> bool {
    let value = std::env::var(REQUIRE_ENV).unwrap_or_default();
    match parse_required(&value) {
        Ok(tools) => tools.contains(&tool),
        Err(e) => panic!("{e}"),
    }
}

/// Test için araç: bulunursa yolu; bulunamazsa zorunluysa DÜŞER, değilse
/// atlama notu basar ve `None` döner (çağıran `return` eder).
pub fn require(tool: Tool) -> Option<PathBuf> {
    let required = is_required(tool);
    match find(tool) {
        Some(path) => Some(path),
        None if required => panic!(
            "{} bulunamadı ama {REQUIRE_ENV} onu zorunlu kılıyor — CI kurulumu bozuk \
             (araç atlanmaz, ADR-0079 §3)",
            tool.name()
        ),
        None => {
            eprintln!(
                "SKIP: {} bulunamadı (zorunlu kılmak için {REQUIRE_ENV}={})",
                tool.name(),
                tool.name()
            );
            None
        }
    }
}
