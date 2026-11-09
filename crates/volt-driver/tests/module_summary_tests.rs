//! Modül özeti (ADR-0114) uçtan uca: saatsiz alt modül ve saatsiz extern
//! üzerinden geçen CDC E3001 alır (#189); alt modül, saatsiz extern ve
//! yerleşik primitif üzerinden `ready`'den `valid`'e giden yol E4007 alır
//! (#193). Beklenen kodlar şartnameden (domain-inference.md K5, K8b;
//! ADR-0050 §4), elle; notların metni ADR-0114'ün tanımladığı biçimdir.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn check_json_in(dir: &Path, file: &Path) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "check", "--format", "json"])
        .arg(file)
        .current_dir(dir)
        .env("VOLT_TOOL_BACKEND", "local")
        .output()
        .expect("volt çalışmalı");
    serde_json::from_slice(&out.stdout).expect("json")
}

/// Testin kendi geçici dizini (checkout'a yazılmaz).
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-summary-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("geçici dizin");
    dir
}

fn diagnostics(env: &Value) -> Vec<Value> {
    env["diagnostics"].as_array().cloned().unwrap_or_default()
}

fn codes(env: &Value) -> Vec<String> {
    diagnostics(env)
        .iter()
        .filter_map(|d| d["code"].as_str().map(str::to_string))
        .collect()
}

/// `//~ KOD` + `//~^ ERROR <parça>`: kod, anotasyonun üstündeki satırda
/// (birincil etiket) ve iletisi parçayı içererek raporlanmalı; başka hata
/// olmamalı.
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
    let env = check_json_in(&root(), &file);
    let errors: Vec<Value> = diagnostics(&env)
        .into_iter()
        .filter(|d| d["severity"] == "error")
        .collect();
    assert_eq!(errors.len(), 1, "{name}: tek hata bekleniyor: {env}");
    let d = &errors[0];
    let prim = d["spans"]
        .as_array()
        .and_then(|s| s.iter().find(|s| s["primary"] == true))
        .expect("birincil etiket");
    assert_eq!(d["code"], code.as_str(), "{name}: {env}");
    assert_eq!(prim["start"]["line"].as_u64(), Some(line), "{name}: {d}");
    assert!(
        d["message"].as_str().is_some_and(|m| m.contains(&part)),
        "{name}: '{part}' bekleniyor: {d}"
    );
    d.clone()
}

fn note(d: &Value, kind: &str) -> String {
    d["notes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|n| n["kind"] == kind)
        .filter_map(|n| n["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

// ── #189: saat alanı ─────────────────────────────────────────────────

#[test]
fn clockless_child_output_carries_its_input_domain() {
    let d = assert_ui_fail("246_clockless_child_crossing.volt");
    assert!(
        note(&d, "note").contains(
            "'p.y' carries @Fast: 'Pass' has no clock, so its output 'y' carries the domain of the inputs it depends on ('a')"
        ),
        "{d}"
    );
    // Öneri iki düzenlemedir: `sync(p.y, ..)` E0003 alırdı.
    assert!(
        d["help"]
            .as_str()
            .is_some_and(|h| h.contains("let p_y_raw = p.y; then sync(p_y_raw, slow_clk)")),
        "{d}"
    );
    // Birinci düzenleme `let` satırı, ek düzenleme kaynağın yerine `sync()`.
    let s = &d["suggestions"][0];
    assert_eq!(
        s["replacement"],
        "    let p_y_raw = p.y
",
        "{d}"
    );
    assert_eq!(
        s["additional_edits"][0]["replacement"], "sync(p_y_raw, slow_clk)",
        "{d}"
    );
}

#[test]
fn clockless_child_merging_two_domains_is_reported_at_the_instance() {
    let d = assert_ui_fail("247_clockless_child_merge.volt");
    assert!(
        note(&d, "reason").starts_with("'And2.y' depends on 'a' and 'b'; 'And2' has no clock"),
        "{d}"
    );
    let labels: Vec<&str> = d["spans"]
        .as_array()
        .expect("spans")
        .iter()
        .filter_map(|s| s["label"].as_str())
        .collect();
    assert!(
        labels.contains(&"@Fast") && labels.contains(&"@Slow"),
        "{d}"
    );
    assert!(
        d["help"]
            .as_str()
            .is_some_and(|h| h.contains("let slow_in_sync = sync(slow_in, fast_clk)")),
        "{d}"
    );
}

#[test]
fn nested_clockless_child_and_clockless_extern_carry_the_domain() {
    assert_ui_fail("248_clockless_child_nested.volt");
    assert_ui_fail("249_clockless_extern_crossing.volt");
}

/// K3: örnek başına bir tanı; aynı iki bağlamayı paylaşan iki çıkış
/// tanıyı çoğaltmaz ve çıkışın okunması ikinci (kaskad) bir E3001 vermez.
#[test]
fn one_merge_error_per_instance_and_no_cascade() {
    let dir = temp_dir("twice");
    let file = dir.join("twice.volt");
    std::fs::write(
        &file,
        "domain Fast {\n    clock = posedge\n    reset = sync active_high\n}\n\
         domain Slow {\n    clock = posedge\n    reset = sync active_high\n}\n\
         module Both {\n    in  a : bool\n    in  b : bool\n    out y : bool\n    out z : bool\n    y = a && b\n    z = a || b\n}\n\
         module Top {\n    in  _fc : clock @Fast\n    in  _sc : clock @Slow\n    in  f : bool @Fast\n    in  s : bool @Slow\n    out o : bool @Slow\n    let g = Both { a: f, b: s }\n    o = g.y\n}\n",
    )
    .unwrap();
    let env = check_json_in(&dir, &file);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(
        codes(&env).iter().filter(|c| *c == "E3001").count(),
        1,
        "{env}"
    );
}

// ── #193: el sıkışma ─────────────────────────────────────────────────

#[test]
fn handshake_path_through_a_submodule_names_the_instance_step() {
    let d = assert_ui_fail("250_handshake_valid_via_instance.volt");
    assert!(
        note(&d, "reason")
            .contains("combinational path: tx.valid <- g.y (AndGate: y <- b) <- tx_ready."),
        "{d}"
    );
}

#[test]
fn handshake_path_through_builtins() {
    let d = assert_ui_fail("251_handshake_valid_via_edgedetect.volt");
    assert!(
        note(&d, "reason").contains("ed.rising (EdgeDetect: rising <- signal) <- tx_ready"),
        "{d}"
    );
    let d = assert_ui_fail("252_handshake_valid_via_arbiter.volt");
    assert!(
        note(&d, "reason").contains("arb.grant (PriorityArbiter: grant <- req) <- req <- tx_ready"),
        "{d}"
    );
}

/// K4: saatsiz extern en kötü durumdur; yanlış alarmın çıkış yolu portta
/// `@no_protocol_check`'tir ve yardım metni bunu söyler.
#[test]
fn clockless_extern_handshake_names_no_protocol_check() {
    let d = assert_ui_fail("253_handshake_valid_via_clockless_extern.volt");
    let help = d["help"].as_str().unwrap_or_default();
    assert!(
        help.contains("The body of extern 'AndExt' is not visible")
            && help.contains("mark the port @no_protocol_check"),
        "{d}"
    );
}

#[test]
fn no_protocol_check_is_the_way_out_of_the_extern_assumption() {
    let pass = root().join("tests/ui/pass/147_handshake_clockless_extern_no_protocol_check.volt");
    let src = std::fs::read_to_string(&pass).expect("okunmalı");
    let env = check_json_in(&root(), &pass);
    assert!(!codes(&env).iter().any(|c| c == "E4007"), "{env}");
    // Aynı tasarım nitelik olmadan E4007 alır: nitelik etkisiz değil.
    let dir = temp_dir("npc");
    let file = dir.join("without.volt");
    let without = src.replace("    @no_protocol_check\n", "");
    assert_ne!(without, src);
    std::fs::write(&file, without).unwrap();
    let env = check_json_in(&dir, &file);
    assert!(codes(&env).iter().any(|c| c == "E4007"), "{env}");
    // Modül düzeyinde de kapanır.
    let module_level = src
        .replace("    @no_protocol_check\n", "")
        .replace("module Producer {", "@no_protocol_check\nmodule Producer {");
    std::fs::write(&file, module_level).unwrap();
    let env = check_json_in(&dir, &file);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(!codes(&env).iter().any(|c| c == "E4007"), "{env}");
}

/// Yakın ıskalar: register'lı çocuk, FIFO boşaltma, bağımsız yollar,
/// sabit çıkış, köprülenen saatsiz çıkış, anotasyonlu extern.
#[test]
fn near_misses_stay_clean() {
    for name in [
        "141_clockless_child_independent_paths.volt",
        "142_clockless_child_constant_output.volt",
        "143_clockless_child_synchronized.volt",
        "144_clockless_extern_annotated.volt",
        "145_handshake_registered_child.volt",
        "146_handshake_fifo_drain.volt",
        "147_handshake_clockless_extern_no_protocol_check.volt",
    ] {
        let env = check_json_in(&root(), &root().join("tests/ui/pass").join(name));
        assert!(codes(&env).is_empty(), "{name}: {env}");
    }
}

/// K4: `@no_protocol_check` yalnız saatsiz extern varsayımına dayanan
/// yolu kapatır — varsayım iç içe bir Volt modülünün içinden gelse de.
/// Görünür bir yol (Volt alt modülünün gerçek kapısı) yapısal kuraldır,
/// nitelikle kapanmaz.
#[test]
fn no_protocol_check_ignores_only_paths_that_rest_on_the_assumption() {
    let head = "extern module AndExt {\n    in  a : bool\n    in  b : bool\n    out y : bool\n}\n\
                module Wrap {\n    in  a : bool\n    in  b : bool\n    out y : bool\n    let e = AndExt { a: a, b: b }\n    y = e.y\n}\n\
                module AndGate {\n    in  a : bool\n    in  b : bool\n    out y : bool\n    y = a && b\n}\n";
    let producer = |attr: &str, child: &str| {
        format!(
            "{head}module Producer {{\n    in  clk  : clock\n    in  have : bool\n    {attr}out tx   : Handshake<u8>\n    reg count : u8 = 0\n    on clk {{\n        if tx.fired {{\n            count <= count + 1\n        }}\n    }}\n    let w = {child} {{ a: have, b: tx.ready }}\n    tx.valid = w.y\n    tx.data  = count\n}}\n"
        )
    };
    let dir = temp_dir("assumed");
    let file = dir.join("p.volt");
    let run = |src: String| {
        std::fs::write(&file, src).unwrap();
        check_json_in(&dir, &file)
    };
    // Varsayım Wrap'in içindeki extern'den geliyor: nitelik kapatır.
    let env = run(producer("@no_protocol_check\n    ", "Wrap"));
    assert!(!codes(&env).iter().any(|c| c == "E4007"), "{env}");
    // Nitelik yoksa E4007, öneri varsayımı ve niteliği anar.
    let env = run(producer("", "Wrap"));
    let d = diagnostics(&env)
        .into_iter()
        .find(|d| d["code"] == "E4007")
        .unwrap_or_else(|| panic!("E4007 bekleniyor: {env}"));
    assert!(
        d["help"].as_str().is_some_and(|h| h
            .contains("The path through 'w' rests on an extern without a clock inside 'Wrap'")
            && h.contains("@no_protocol_check")),
        "{d}"
    );
    // Görünür yol: nitelik E4007'yi kapatmaz; öneri niteliği anmaz.
    let env = run(producer("@no_protocol_check\n    ", "AndGate"));
    let _ = std::fs::remove_dir_all(&dir);
    let d = diagnostics(&env)
        .into_iter()
        .find(|d| d["code"] == "E4007")
        .unwrap_or_else(|| panic!("E4007 bekleniyor: {env}"));
    assert!(
        !d["help"]
            .as_str()
            .unwrap_or_default()
            .contains("@no_protocol_check"),
        "{d}"
    );
}
