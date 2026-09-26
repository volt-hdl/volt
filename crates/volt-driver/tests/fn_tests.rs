//! Fonksiyon desteği uçtan uca (ADR-0081): `tests/ui/fail/*_fn_*`
//! fixture'ları `volt check`'te beklenen kodu beklenen satırda verir
//! (parser, HIR ve emitter tanıları aynı boru hattında — ADR-0070);
//! `tests/ui/pass/*_fn_*` SV'si Karar 12'nin açılım biçimindedir; çok
//! dosyalı birimde `pub fn` (ADR-0042).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-fn-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

fn fn_fixtures(dir: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(root().join(dir))
        .expect("ui dizini okunmalı")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension().is_some_and(|e| e == "volt")
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().contains("_fn_"))
        })
        .collect();
    files.sort();
    files
}

fn check_json(file: &Path) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "check", "--format", "json"])
        .arg(file)
        .output()
        .expect("volt çalışmalı");
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "{}: JSON değil ({e}): {}",
            file.display(),
            String::from_utf8_lossy(&out.stderr)
        )
    })
}

/// Hata tanıları: (kod, birincil satır).
fn errors(env: &Value) -> Vec<(String, u64)> {
    env["diagnostics"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .filter(|d| d["severity"] == "error")
        .map(|d| {
            let line = d["spans"]
                .as_array()
                .and_then(|s| s.iter().find(|s| s["primary"] == true))
                .and_then(|s| s["start"]["line"].as_u64())
                .unwrap_or(0);
            (d["code"].as_str().unwrap_or_default().to_string(), line)
        })
        .collect()
}

fn build(file: &Path, target: &Path, extra: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "build", "--target-dir"])
        .arg(target)
        .args(extra)
        .arg(file)
        .output()
        .expect("volt çalışmalı")
}

fn read_sv(target: &Path, module: &str) -> String {
    std::fs::read_to_string(target.join("rtl").join(format!("{module}.sv")))
        .unwrap_or_else(|e| panic!("{module}.sv okunmalı: {e}"))
}

// ═══ ui/fail ═════════════════════════════════════════════════════════

/// Satır 1 `//~ KOD`; `//~^ ERROR` bir üst satırı işaretler.
fn expectation(text: &str) -> (String, u64) {
    let code = text
        .lines()
        .next()
        .and_then(|l| l.trim().strip_prefix("//~ "))
        .map(|c| c.trim().to_string())
        .expect("ilk satır '//~ KOD'");
    let line = text
        .lines()
        .position(|l| l.trim_start().starts_with("//~^ ERROR"))
        .expect("'//~^ ERROR' anotasyonu") as u64;
    (code, line)
}

#[test]
fn every_fn_fail_fixture_reports_its_code_on_the_marked_line() {
    let files = fn_fixtures("tests/ui/fail");
    assert_eq!(files.len(), 21, "ADR-0081 ui/fail fixture sayısı");
    let mut bad = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).expect("okunmalı");
        let (code, line) = expectation(&text);
        let errs = errors(&check_json(file));
        if !errs.iter().any(|(c, l)| *c == code && *l == line) {
            bad.push(format!(
                "{}: {code}@{line} bekleniyor, bulunan {errs:?}",
                file.display()
            ));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Tanının yeri ilkesi (ADR-0081 §6): fn'in kendisiyle ilgili hata
/// tanımda ve BİR KEZ — iki çağrı iki tanı değildir.
#[test]
fn a_function_body_error_is_reported_once_regardless_of_call_count() {
    let file = root().join("tests/ui/fail/160_fn_match_body.volt");
    let errs = errors(&check_json(&file));
    assert_eq!(errs, vec![("E0003".to_string(), 5)], "{errs:?}");
}

/// Karşılıklı özyinelemede döngüdeki her fn raporlanır (E4009 biçimi).
#[test]
fn every_function_on_a_call_cycle_gets_e4013() {
    let file = root().join("tests/ui/fail/152_fn_recursive_mutual.volt");
    let env = check_json(&file);
    let errs = errors(&env);
    assert_eq!(
        errs,
        vec![("E4013".to_string(), 3), ("E4013".to_string(), 7)],
        "{errs:?}"
    );
    let notes: Vec<String> = env["diagnostics"]
        .as_array()
        .expect("tanılar")
        .iter()
        .flat_map(|d| d["notes"].as_array().cloned().unwrap_or_default())
        .filter_map(|n| n["text"].as_str().map(str::to_string))
        .collect();
    assert!(notes.iter().any(|n| n == "cycle: g → h → g"), "{notes:?}");
    assert!(notes.iter().any(|n| n == "cycle: h → g → h"), "{notes:?}");
}

// ═══ ui/pass: açılım biçimi (Karar 12) ════════════════════════════════

#[test]
fn wire_mode_names_follow_fn_k_let_and_param() {
    let target = temp_dir("simple");
    let out = build(
        &root().join("tests/ui/pass/112_fn_simple.volt"),
        &target,
        &[],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let sv = read_sv(&target, "FnSimple");
    // Tüm sağ taraf: sonuç teli yok, `t` açılmış ifadeyle sürülür.
    assert!(
        sv.contains("wire [7:0] t = (a == 8'd255) ? a : (a + 8'd1);"),
        "{sv}"
    );
    // İkinci çağrı k = 1; ifade içinde olduğu için sonuç teli.
    assert!(sv.contains("// sat_inc(b) — 112_fn_simple.volt:25"), "{sv}");
    assert!(sv.contains("wire [7:0] sat_inc_1 = "), "{sv}");
    // Yalın olmayan argüman teli ve let telleri.
    assert!(sv.contains("wire [7:0] parity_0_x = a ^ b;"), "{sv}");
    assert!(sv.contains("wire [3:0] parity_0_lo = "), "{sv}");
    assert!(sv.contains("wire [1:0] parity_0_q = "), "{sv}");
    // fn SV'de görünmez.
    assert!(!sv.contains("function"), "{sv}");
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn a_const_in_a_function_body_is_not_captured_by_a_caller_signal() {
    let target = temp_dir("hygiene");
    let out = build(
        &root().join("tests/ui/pass/113_fn_nested_call.volt"),
        &target,
        &[],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let sv = read_sv(&target, "FnNested");
    // Modülde `let K = n` var; gövdedeki K birimin const'u (3).
    assert!(sv.contains("wire [7:0] add_k_0 = a + 8'd3;"), "{sv}");
    assert!(sv.contains("wire [7:0] add_k_1 = add_k_0 + 8'd3;"), "{sv}");
    // İç çağrının başlığı argümanı gövdede yazıldığı gibi gösterir.
    assert!(
        sv.contains("// add_k(add_k(a)) — 113_fn_nested_call.volt:11"),
        "{sv}"
    );
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn struct_literal_argument_becomes_a_per_field_param_wire() {
    let target = temp_dir("struct");
    let out = build(
        &root().join("tests/ui/pass/114_fn_struct_enum_params.volt"),
        &target,
        &[],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let sv = read_sv(&target, "FnStructEnum");
    assert!(sv.contains("wire [7:0] alu_0_p_a = x;"), "{sv}");
    assert!(sv.contains("wire [7:0] alu_0_p_b = y;"), "{sv}");
    assert!(sv.contains("assign s_a = swap_0_p_b;"), "{sv}");
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn comb_and_block_for_calls_substitute_without_wires() {
    let target = temp_dir("comb");
    let out = build(
        &root().join("tests/ui/pass/116_fn_comb_block.volt"),
        &target,
        &[],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let sv = read_sv(&target, "FnComb");
    assert!(!sv.contains("mix_"), "ikame kipinde tel yok: {sv}");
    // Parametre tipinde olmayan argüman ve sonuç boyut dönüşümüyle.
    assert!(
        sv.contains("m = 8'((8'(xs[0 +: 8] + 8'd1) ^ k) + ((8'(xs[0 +: 8] + 8'd1) ^ k) >> 1));"),
        "{sv}"
    );
    assert!(
        sv.contains("ys[24 +: 8] = 8'((xs[24 +: 8] ^ k) + ((xs[24 +: 8] ^ k) >> 1));"),
        "{sv}"
    );
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn contract_calls_do_not_change_the_rtl() {
    let file = root().join("tests/ui/pass/117_fn_contract_call.volt");
    let plain = temp_dir("contract-plain");
    let sva = temp_dir("contract-sva");
    assert!(build(&file, &plain, &[]).status.success());
    let out = build(&file, &sva, &["--emit=sva"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(read_sv(&plain, "FnContract"), read_sv(&sva, "FnContract"));
    let props = std::fs::read_to_string(sva.join("formal/fncontract.sva")).expect("sva");
    assert!(
        props.contains("(a_r & 32'd3) == 32'd0"),
        "kontrat ikame edilir: {props}"
    );
    assert!(!props.contains("is_aligned"), "{props}");
    let _ = std::fs::remove_dir_all(&plain);
    let _ = std::fs::remove_dir_all(&sva);
}

// ═══ Çok dosyalı birim (ADR-0042) ═══════════════════════════════════

#[test]
fn pub_fn_from_another_file_is_inlined_in_the_caller() {
    let target = temp_dir("multifile");
    let out = build(
        &root().join("tests/ui/multifile/fn/main.volt"),
        &target,
        &[],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let sv = read_sv(&target, "Top");
    assert!(sv.contains("// sat_inc(a) — main.volt:8"), "{sv}");
    assert!(
        sv.contains("wire [7:0] sat_inc_0 = (a == 8'd255) ? a : (a + 8'd1);"),
        "{sv}"
    );
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn a_private_fn_cannot_be_imported_from_another_file() {
    let dir = temp_dir("private");
    std::fs::copy(
        root().join("tests/ui/multifile/fn/fnlib.volt"),
        dir.join("fnlib.volt"),
    )
    .expect("fnlib");
    let main = dir.join("main.volt");
    std::fs::write(
        &main,
        "use fnlib::hidden;\n\nmodule Top {\n    in  a : u8\n    out y : u8\n    y = hidden(a)\n}\n",
    )
    .expect("main");
    let errs = errors(&check_json(&main));
    assert!(errs.iter().any(|(c, _)| c == "E1004"), "{errs:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
