//! Üretilen ad çarpışmaları ve `let x = sync(...)` (ADR-0090) uçtan uca:
//! her yeni hata sınıfının ui/fail tanısı (kod, satır, ileti; `volt
//! check`), iki etiketli ileti, `let` köprüsünün `wire` + atama ile aynı
//! SV'yi ve SDC/XDC kısıtını üretmesi, ADR-0084 şablonlarının temiz
//! kalması.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn volt_in(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en"])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("volt çalışmalı")
}

fn check_json(file: &Path) -> Value {
    let out = volt_in(
        &root(),
        &["check", "--format", "json", file.to_str().unwrap()],
    );
    serde_json::from_slice(&out.stdout).expect("json")
}

/// `//~ KOD` + `//~^ ERROR <parça>`: kod, anotasyonun üstündeki satırda
/// (birincil etiket) ve iletisi parçayı içererek raporlanmalı.
fn assert_ui_fail(name: &str) -> Value {
    let file = root().join("tests/ui/fail").join(name);
    let src = std::fs::read_to_string(&file).expect("okunmalı");
    let code = src
        .lines()
        .next()
        .and_then(|l| l.strip_prefix("//~ "))
        .expect("ilk satır //~ KOD")
        .trim()
        .to_string();
    let (line, part) = src
        .lines()
        .enumerate()
        .find_map(|(i, l)| {
            l.trim_start()
                .strip_prefix("//~^ ERROR ")
                .map(|p| (i as u64, p.trim().to_string()))
        })
        .expect("//~^ ERROR satırı");
    let env = check_json(&file);
    let hit = env["diagnostics"]
        .as_array()
        .expect("tanılar")
        .iter()
        .filter(|d| d["code"] == code.as_str())
        .find(|d| {
            let prim = d["spans"]
                .as_array()
                .and_then(|s| s.iter().find(|s| s["primary"] == true));
            prim.and_then(|p| p["start"]["line"].as_u64()) == Some(line)
                && d["message"].as_str().is_some_and(|m| m.contains(&part))
        })
        .cloned();
    hit.unwrap_or_else(|| panic!("{name}: {code} satır {line} '{part}' bekleniyor: {env}"))
}

#[test]
fn ui_fail_189_instance_output() {
    let d = assert_ui_fail("189_generated_name_instance_output.volt");
    // İki konum: örnek (birincil) ve port (ikincil).
    let spans = d["spans"].as_array().expect("spans");
    assert_eq!(spans.len(), 2, "{d}");
    let secondary = spans
        .iter()
        .find(|s| s["primary"] == false)
        .expect("ikincil");
    assert_eq!(secondary["label"], "also 'timer_irq'");
}

#[test]
fn ui_fail_190_two_instances() {
    let d = assert_ui_fail("190_generated_name_two_instances.volt");
    assert_eq!(d["spans"].as_array().map(Vec::len), Some(2), "{d}");
}

#[test]
fn ui_fail_191_builtin_signal() {
    assert_ui_fail("191_generated_name_builtin_signal.volt");
}

#[test]
fn ui_fail_192_reset_port() {
    assert_ui_fail("192_generated_name_reset_port.volt");
}

#[test]
fn ui_fail_193_reset_chain() {
    assert_ui_fail("193_generated_name_reset_chain.volt");
}

#[test]
fn ui_fail_194_sync_stage() {
    assert_ui_fail("194_generated_name_sync_stage.volt");
}

#[test]
fn ui_fail_195_bundle_vs_instance() {
    assert_ui_fail("195_generated_name_bundle_vs_instance.volt");
}

#[test]
fn ui_fail_196_let_sync_width() {
    assert_ui_fail("196_let_sync_width.volt");
}

#[test]
fn ui_fail_197_sync_inside_expression() {
    assert_ui_fail("197_sync_inside_expression.volt");
}

/// Türkçe ileti de iki kaynağı adlandırır.
#[test]
fn collision_message_in_turkish() {
    let file = root().join("tests/ui/fail/189_generated_name_instance_output.volt");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "tr", "check", file.to_str().unwrap()])
        .output()
        .expect("volt");
    let text =
        String::from_utf8_lossy(&out.stderr).to_string() + &String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("'timer_irq' hem 'timer_irq' portu hem 'timer' örneğinin 'irq' çıkışı"),
        "{text}"
    );
}

const DOMS: &str = "\
domain Fast {
    clock = posedge
    reset = sync active_high
}
domain Slow {
    clock = posedge
    reset = sync active_high
}
";

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-gn-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// Kaynağı derler; `(rtl/Top.sv, constraints/Top.sdc, constraints/Top.xdc)`
/// — kaynak adını taşıyan başlık satırı atılmış.
fn build_outputs(dir: &Path, name: &str, body: &str) -> [String; 3] {
    let file = format!("{name}.volt");
    std::fs::write(dir.join(&file), format!("{DOMS}{body}")).expect("yazılmalı");
    let target = format!("out_{name}");
    let out = volt_in(
        dir,
        &["build", &file, "--emit=sdc,xdc", "--target-dir", &target],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let read = |rel: &str| {
        std::fs::read_to_string(dir.join(&target).join(rel))
            .expect("çıktı")
            .lines()
            .filter(|l| !l.contains("Source:"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    [
        read("rtl/Top.sv"),
        read("constraints/Top.sdc"),
        read("constraints/Top.xdc"),
    ]
}

fn sync_design(binding: &str) -> String {
    format!(
        "module Top {{\n    in  fclk : clock @Fast\n    in  sclk : clock @Slow\n    \
         in  x : u4 @Fast\n    out o : u4 @Slow\n{binding}    o = s\n}}\n"
    )
}

#[test]
fn let_sync_equals_wire_and_assignment() {
    let dir = temp_dir("letsync");
    for f in ["sync", "sync3"] {
        let wire = build_outputs(
            &dir,
            &format!("wire_{f}"),
            &sync_design(&format!("    wire s : u4\n    s = {f}(x, sclk)\n")),
        );
        let typed = build_outputs(
            &dir,
            &format!("typed_{f}"),
            &sync_design(&format!("    let s : u4 = {f}(x, sclk)\n")),
        );
        let bare = build_outputs(
            &dir,
            &format!("bare_{f}"),
            &sync_design(&format!("    let s = {f}(x, sclk)\n")),
        );
        assert!(wire[0].contains("sync_x_stage1"), "{}", wire[0]);
        assert!(
            wire[1].contains("sync_x_stage0"),
            "SDC köprüsü: {}",
            wire[1]
        );
        assert_eq!(typed, wire, "{f}: tipli let");
        assert_eq!(bare, wire, "{f}: tipsiz let");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// ADR-0084 şablonları çakışmasız: yeni denetim onlarda tanı üretmez.
#[test]
fn templates_stay_clean() {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(root().join("templates")).expect("templates") {
        let dir = entry.expect("girdi").path();
        for f in std::fs::read_dir(&dir).expect("şablon") {
            let f = f.expect("dosya").path();
            if f.extension().is_some_and(|e| e == "volt") {
                files.push(f);
            }
        }
    }
    assert!(files.len() >= 4, "şablonlar: {files:?}");
    for f in files {
        let env = check_json(&f);
        let e1003 = env["diagnostics"]
            .as_array()
            .map(|a| a.iter().filter(|d| d["code"] == "E1003").count());
        assert_eq!(e1003, Some(0), "{}: {env}", f.display());
    }
}

/// Aynı ad için başka bir E1003 (enum localparam'ı ADR-0074, fn açılımı
/// ADR-0081) varsa çift bildirim denetimi ikinciyi eklemez.
#[test]
fn one_e1003_per_clash() {
    let dir = temp_dir("once");
    let enum_src = "enum S { Idle, Run }\nmodule Top {\n    in  s : S\n    in  x : u8\n    \
                    out o : bool\n    out q : u8\n    let S_Idle = x\n    q = S_Idle\n    o = s == S::Idle\n}\n";
    let fn_src = "fn parity(x: u8) -> bool {\n    let lo = x[3:0] ^ x[7:4]\n    lo[0] != lo[1]\n}\n\
                  module Top {\n    in  a : u8\n    in  b : u8\n    out o : bool\n    out q : u8\n    \
                  let parity_0_x = a & b\n    q = parity_0_x\n    o = parity(a ^ b)\n}\n";
    for (name, src) in [("enum", enum_src), ("fn", fn_src)] {
        let file = dir.join(format!("{name}.volt"));
        std::fs::write(&file, src).expect("yazılmalı");
        let env = check_json(&file);
        let n = env["diagnostics"]
            .as_array()
            .map(|a| a.iter().filter(|d| d["code"] == "E1003").count());
        assert_eq!(n, Some(1), "{name}: {env}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
