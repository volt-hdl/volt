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

use std::path::PathBuf;

pub use volt_tools::Tool;

/// Ortam değişkeni: zorunlu araçlar (`verilator,sby,...` ya da `all`).
pub const REQUIRE_ENV: &str = "VOLT_REQUIRE_TOOLS";

/// Aracın yolu — `volt test`/`volt verify`/`volt doctor` ile AYNI arama
/// (`volt-tools`, ADR-0084 §1): önce ortam değişkeni, sonra PATH.
pub fn find(tool: Tool) -> Option<PathBuf> {
    volt_tools::find(tool)
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
