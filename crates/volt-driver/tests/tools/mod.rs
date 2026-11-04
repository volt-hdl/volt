//! Araç bağımlı testlerin ortak araç araması (ADR-0079 §3, ADR-0107).
//!
//! Araç yoksa test varsayılan olarak ATLANIR (geliştirici makinesi, araçsız
//! CI işi). `VOLT_REQUIRE_TOOLS` virgüllü bir araç listesidir (ya da `all`):
//! listedeki araç bulunamazsa test atlanmaz, DÜŞER — aracın kurulu olması
//! gereken CI işlerinde kurulum sessizce bozulursa testler yeşil kalmasın.
//! Listede bilinmeyen ad da düşürür: yazım hatası hiçbir aracı sessizce
//! zorunluluktan çıkarmasın.
//!
//! Defter (`VOLT_TOOL_LEDGER`, ADR-0107): değişken bir dosya yolu ise her
//! araç sorgusu oraya bir satır ekler — test atladıysa `SKIP`, araç
//! bulunduysa `RAN` (araç zorunlu muydu). CI her test koşan işin sonunda
//! defteri sayar (`scripts/ci/tool_ledger.py`): araçlı işte tek `SKIP` işi
//! düşürür; araçsız işte görülen her araçlı testin bir araçlı işte aracı
//! zorunluyken koştuğu ayrıca denetlenir. Araç isteyen test bu yüzden elle
//! tutulan bir listeye bağlı değildir: bu modülü kullanan her test
//! kendiliğinden deftere girer.

// Her test ikilisi bu modülün yalnız bir kısmını kullanır.
#![allow(dead_code)]

use std::io::Write as _;
use std::path::PathBuf;

pub use volt_tools::Tool;

/// Ortam değişkeni: zorunlu araçlar (`verilator,sby,...` ya da `all`).
pub const REQUIRE_ENV: &str = "VOLT_REQUIRE_TOOLS";

/// Ortam değişkeni: defter dosyası (boş ya da yoksa defter tutulmaz).
pub const LEDGER_ENV: &str = "VOLT_TOOL_LEDGER";

/// Test ikilisinin adı, cargo'nun `-<16 onaltılık>` eki olmadan.
pub fn binary_name() -> String {
    let exe = std::env::current_exe().unwrap_or_default();
    let stem = exe
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    match stem.rsplit_once('-') {
        Some((name, hash)) if hash.len() == 16 && hash.bytes().all(|b| b.is_ascii_hexdigit()) => {
            name.to_string()
        }
        _ => stem,
    }
}

/// Deftere bir satır ekler:
/// `<SKIP|RAN>\t<ikili>\t<test>\t<gereksinim>\t<zorunlu 0|1>`.
/// Test adı libtest'in iş parçacığı adıdır (`modül::test`); başka bir iş
/// parçacığından çağrılırsa `?`.
pub fn record(event: &str, requirement: &str, required: bool) {
    let Some(path) = std::env::var_os(LEDGER_ENV).filter(|p| !p.is_empty()) else {
        return;
    };
    let test = std::thread::current()
        .name()
        .unwrap_or("?")
        .replace(['\t', '\n'], " ");
    let line = format!(
        "{event}\t{}\t{test}\t{requirement}\t{}\n",
        binary_name(),
        u8::from(required)
    );
    // Tek `write_all`, ekleme kipinde: paralel testlerin satırları karışmaz.
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .unwrap_or_else(|e| panic!("{LEDGER_ENV} ({}) açılamadı: {e}", path.to_string_lossy()));
    file.write_all(line.as_bytes())
        .unwrap_or_else(|e| panic!("{LEDGER_ENV} yazılamadı: {e}"));
}

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
/// atlama notu basar ve `None` döner (çağıran `return` eder). Her sonuç
/// deftere yazılır.
pub fn require(tool: Tool) -> Option<PathBuf> {
    let required = is_required(tool);
    match find(tool) {
        Some(path) => {
            record("RAN", tool.name(), required);
            Some(path)
        }
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
            record("SKIP", tool.name(), false);
            None
        }
    }
}

/// Bir ortam değişkeniyle açılan ağır test (ör. `VOLT_DOCKER_E2E=1`,
/// ADR-0107 §2): değişken `1` ise deftere `RAN` (zorunlu) ve `true`;
/// değilse atlama notu, deftere `SKIP` ve `false`. Gereksinimin adı
/// değişkenin kendisidir; araçlı işlerin sıfır-SKIP kuralı bunu da sayar.
pub fn opt_in(var: &str) -> bool {
    if std::env::var(var).is_ok_and(|v| v == "1") {
        record("RAN", var, true);
        true
    } else {
        eprintln!("SKIP: {var}=1 değil (açmak için {var}=1)");
        record("SKIP", var, false);
        false
    }
}
