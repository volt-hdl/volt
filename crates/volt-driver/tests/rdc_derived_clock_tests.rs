//! Reset yakınsaması bir örnek çıkışından gelen saatte (#371, ADR-0065
//! R6) — uçtan uca, gerçek Yosys'le.
//!
//! İki kardeş örnek aynı ham reset'i aynı saatte ayrı ayrı senkronluyor;
//! saat bir extern örneğinin (PLL) çıkışı. R6: örnekleme ağacında (saat,
//! ham reset) çifti başına en fazla bir zincir (E3003). Önce yalnız üst
//! modülün kendi saat portu sayılıyordu: `volt check` sessizdi, Yosys'in
//! düzleştirilmiş netlist'inde `rst_n` aynı saat ağında iki bağımsız
//! zincirle bırakılıyordu.
//!
//! Özellik: Volt bir tasarımı kabul ederse netlist'te her saat ağında ham
//! `rst_n`'nin en fazla bir senkronizör zinciri vardır. Zincirin ilk aşaması
//! `rst_n` ile asenkron sıfırlanan ve girişi sabit olan flop'tur (sonraki
//! aşamaların girişi bir önceki aşamadır).
//!
//! Yosys yoksa netlist adımı atlanır; `VOLT_REQUIRE_TOOLS=yosys` ile (CI'ın
//! araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const HEAD: &str = "\
domain Core { clock = posedge, reset = async active_low }

extern module Pll {
    in  clk_in : clock @Core
    out clk_o  : clock @Core
}

module Child {
    in  clk   : clock @Core
    in  rst_n : reset(async, active_low)
    in  d     : u8
    out q     : u8
    reg r : u8 = 0
    on clk { r <= d }
    q = r
}
";

/// `Top`: PLL ve iki `Child`; `a_clk`/`b_clk` çocukların saati.
fn design(a_clk: &str, b_clk: &str) -> String {
    format!(
        "{HEAD}
module Top {{
    in  clk   : clock @Core
    in  rst_n : reset(async, active_low)
    in  d     : u8
    out q1    : u8
    out q2    : u8
    let p = Pll {{ clk_in: clk }}
    let a = Child {{ clk: {a_clk}, rst_n: rst_n, d: d }}
    let b = Child {{ clk: {b_clk}, rst_n: rst_n, d: d }}
    q1 = a.q
    q2 = b.q
}}
"
    )
}

/// Tek `Child`, PLL çıkışında.
fn single() -> String {
    format!(
        "{HEAD}
module Top {{
    in  clk   : clock @Core
    in  rst_n : reset(async, active_low)
    in  d     : u8
    out q1    : u8
    let p = Pll {{ clk_in: clk }}
    let a = Child {{ clk: p.clk_o, rst_n: rst_n, d: d }}
    q1 = a.q
}}
"
    )
}

/// Extern PLL'in Yosys için kara kutusu: çıkışı Top'un saatinden ayrı bir
/// ağdır (gerçek bir PLL'in çıkışı gibi).
const PLL_STUB: &str = "(* blackbox *)\nmodule Pll(input clk_in, output clk_o);\nendmodule\n";

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-r6-derived-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// `volt build`: kabul edildi mi, çıktı metni.
fn build(dir: &Path, src: &str) -> (bool, String) {
    let file = dir.join("top.volt");
    std::fs::write(&file, src).expect("tasarım");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["build", "--color", "never", "--target-dir"])
        .arg(dir.join("build"))
        .arg(&file)
        .current_dir(dir)
        .env("VOLT_LANG", "en")
        .output()
        .expect("volt çalışmalı");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.success(), text)
}

/// Düzleştirilmiş `Top` netlist'inde, her saat ağında ham `rst_n`'nin
/// zincir sayısı (Yosys yoksa `None`).
fn chains_per_clock(dir: &Path) -> Option<Vec<usize>> {
    let yosys = tools::require(tools::Tool::Yosys)?;
    let rtl = dir.join("build").join("rtl");
    std::fs::write(rtl.join("pll_stub.v"), PLL_STUB).expect("stub");
    let mut files: Vec<String> = std::fs::read_dir(&rtl)
        .expect("rtl")
        .map(|e| e.expect("girdi").path())
        .filter(|p| p.extension().is_some_and(|x| x == "sv"))
        .map(|p| format!("\"{}\"", p.display()))
        .collect();
    files.sort();
    let json = dir.join("top.json");
    let script = format!(
        "read_verilog \"{}\"; read_verilog -sv {}; hierarchy -check -top Top; proc; flatten; opt_clean; write_json \"{}\"",
        rtl.join("pll_stub.v").display(),
        files.join(" "),
        json.display()
    );
    let out = Command::new(&yosys)
        .args(["-q", "-p", &script])
        .output()
        .expect("yosys çalışmalı");
    assert!(
        out.status.success(),
        "yosys başarısız:\n{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let net: Value =
        serde_json::from_slice(&std::fs::read(&json).expect("json")).expect("geçerli json");
    let top = &net["modules"]["Top"];
    let raw = &top["ports"]["rst_n"]["bits"];
    let mut by_clock: BTreeMap<String, usize> = BTreeMap::new();
    for cell in top["cells"].as_object().expect("hücreler").values() {
        if cell["type"] != "$adff" {
            continue;
        }
        let c = &cell["connections"];
        // İlk aşama: ham reset'le sıfırlanır, girişi sabittir (Yosys
        // JSON'unda sabit bit bir dizgedir, ağ biti bir sayı).
        let constant_d = c["D"]
            .as_array()
            .is_some_and(|d| d.iter().all(Value::is_string));
        if &c["ARST"] == raw && constant_d {
            *by_clock.entry(c["CLK"].to_string()).or_default() += 1;
        }
    }
    Some(by_clock.into_values().collect())
}

/// Volt kabul ederse her saat ağında en çok bir zincir; reddederse SV yok.
/// Dönüş: (kabul edildi mi, çıktı, Yosys'in saydığı zincirler).
fn check(tag: &str, src: &str) -> (bool, String, Option<Vec<usize>>) {
    let dir = temp_dir(tag);
    let (accepted, text) = build(&dir, src);
    let chains = if accepted {
        let chains = chains_per_clock(&dir);
        if let Some(c) = &chains {
            assert!(
                c.iter().all(|&n| n <= 1),
                "{tag}: Volt kabul etti, ham rst_n bir saatte birden çok zincirle bırakılıyor: {c:?}\n{text}"
            );
        }
        chains
    } else {
        assert!(
            !dir.join("build").join("rtl").join("Top.sv").exists(),
            "{tag}: reddedilen tasarımın SV'si yazıldı"
        );
        None
    };
    let _ = std::fs::remove_dir_all(&dir);
    (accepted, text, chains)
}

#[test]
fn issue_371_two_chains_on_an_instance_clock_are_rejected() {
    // Önce: `volt check` sessiz, Yosys `p_clk_o` üzerinde iki zincir.
    let (accepted, text, _) = check("issue", &design("p.clk_o", "p.clk_o"));
    assert!(!accepted, "{text}");
    assert!(
        text.contains("error[E3003]: raw reset 'rst_n' is synchronized twice on 'p.clk_o'"),
        "{text}"
    );
}

#[test]
fn one_chain_per_clock_net_is_accepted_and_the_netlist_agrees() {
    // Tek çocuk PLL saatinde: bir zincir.
    let (accepted, text, chains) = check("single", &single());
    assert!(accepted, "{text}");
    if let Some(c) = chains {
        assert_eq!(c, vec![1], "{text}");
    }
    // İki çocuk iki ayrı saatte (Top'un portu ve PLL çıkışı): her saatte
    // bir zincir — analiz saat ağlarını ayırır.
    let (accepted, text, chains) = check("two-clocks", &design("clk", "p.clk_o"));
    assert!(accepted, "{text}");
    if let Some(c) = chains {
        assert_eq!(c, vec![1, 1], "{text}");
    }
}
