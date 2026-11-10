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

/// K4 (karar B): saatsiz extern en kötü durumdur; varsayımla kurulan yol
/// da E4007'dir. Not yolun varsayıma dayandığını söyler, öneri valid'i
/// register'da üretmektir; `@no_protocol_check` anılmaz.
#[test]
fn clockless_extern_handshake_note_names_the_assumption() {
    let d = assert_ui_fail("253_handshake_valid_via_clockless_extern.volt");
    let n = note(&d, "note");
    assert!(
        n.contains("extern 'AndExt' has no visible body; Volt assumes every output depends on every input, so 'y' is taken to depend on 'b'")
            && n.contains("Decide valid from registered state"),
        "{d}"
    );
    let help = d["help"].as_str().unwrap_or_default();
    assert!(
        help.starts_with("decide valid from registered state"),
        "{d}"
    );
    assert!(!help.contains("@no_protocol_check"), "{d}");
}

/// K4 (karar B): `@no_protocol_check` E4007'yi etkilemez (ADR-0050 §3:
/// yalnız otomatik protokol kontratları). Varsayımla kurulan yol port
/// düzeyinde de, modül düzeyinde de, nitelik olmadan da E4007'dir.
#[test]
fn no_protocol_check_does_not_silence_e4007() {
    let fail = root().join("tests/ui/fail/255_handshake_clockless_extern_no_protocol_check.volt");
    let src = std::fs::read_to_string(&fail).expect("okunmalı");
    let d = assert_ui_fail("255_handshake_clockless_extern_no_protocol_check.volt");
    assert!(
        note(&d, "note").contains("extern 'Pair' has no visible body"),
        "{d}"
    );
    let dir = temp_dir("npc");
    let file = dir.join("variant.volt");
    let without = src.replace("    @no_protocol_check\n", "");
    assert_ne!(without, src);
    let module_level =
        without.replace("module Producer {", "@no_protocol_check\nmodule Producer {");
    for (name, variant) in [("without", without), ("module-level", module_level)] {
        std::fs::write(&file, variant).unwrap();
        let env = check_json_in(&dir, &file);
        let e4007 = codes(&env).iter().filter(|c| *c == "E4007").count();
        assert_eq!(e4007, 1, "{name}: {env}");
    }
    let _ = std::fs::remove_dir_all(&dir);
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
        "148_clockless_child_annotated_output.volt",
    ] {
        let env = check_json_in(&root(), &root().join("tests/ui/pass").join(name));
        assert!(codes(&env).is_empty(), "{name}: {env}");
    }
}

/// K4 (karar B): varsayımla kurulan yol (iç içe bir Volt modülünün içindeki
/// saatsiz extern) ve görünür yol (Volt alt modülünün gerçek kapısı)
/// ikisi de E4007'dir, nitelikle ya da niteliksiz. Not yalnız varsayımla
/// kurulan yolda varsayımı söyler; öneri ikisinde de aynıdır.
#[test]
fn assumed_and_visible_paths_are_both_e4007() {
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
    let the_e4007 = |env: &Value| {
        let found: Vec<Value> = diagnostics(env)
            .into_iter()
            .filter(|d| d["code"] == "E4007")
            .collect();
        assert_eq!(found.len(), 1, "tek E4007 bekleniyor: {env}");
        found[0].clone()
    };
    for attr in ["@no_protocol_check\n    ", ""] {
        let d = the_e4007(&run(producer(attr, "Wrap")));
        assert!(
            note(&d, "note")
                .contains("the path through 'w' rests on an extern without a clock inside 'Wrap'"),
            "{attr:?}: {d}"
        );
        let d = the_e4007(&run(producer(attr, "AndGate")));
        assert!(
            note(&d, "note").is_empty(),
            "{attr:?}: görünür yolda varsayım notu olmamalı: {d}"
        );
        assert!(
            d["help"]
                .as_str()
                .is_some_and(|h| h.starts_with("decide valid from registered state")
                    && !h.contains("@no_protocol_check")),
            "{attr:?}: {d}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

const TWO_DOMAINS: &str = "domain Fast {\n    clock = posedge\n    reset = sync active_high\n}\n\
                           domain Slow {\n    clock = posedge\n    reset = sync active_high\n}\n";

fn check_src(tag: &str, src: &str) -> Value {
    let dir = temp_dir(tag);
    let file = dir.join("m.volt");
    std::fs::write(&file, src).unwrap();
    let env = check_json_in(&dir, &file);
    let _ = std::fs::remove_dir_all(&dir);
    env
}

fn the_error(env: &Value, code: &str) -> Value {
    let found: Vec<Value> = diagnostics(env)
        .into_iter()
        .filter(|d| d["code"] == code)
        .collect();
    assert_eq!(found.len(), 1, "tek {code} bekleniyor: {env}");
    found[0].clone()
}

/// `sync()` yalnız tek biti köprüler (K9): çok bitli örnek çıkışında
/// makinece uygulanan öneri yoktur, yardım genel biçimdedir.
#[test]
fn a_multi_bit_instance_output_gets_no_sync_fix() {
    let src = format!(
        "{TWO_DOMAINS}module Pass8 {{\n    in  a : u8\n    out y : u8\n    y = a\n}}\n\
         module Cross {{\n    in  _fclk : clock @Fast\n    in  sclk  : clock @Slow\n    in  a     : u8    @Fast\n    out y     : u8    @Slow\n    let p = Pass8 {{ a: a }}\n    y = p.y\n}}\n"
    );
    let d = the_error(&check_src("wide", &src), "E3001");
    assert_eq!(d["suggestions"].as_array().map(Vec::len), Some(0), "{d}");
    assert!(
        d["help"]
            .as_str()
            .is_some_and(|h| h.contains("dest = sync(src, <clock of Slow>)")),
        "{d}"
    );
}

/// Not, kaynağın alanını gerçekten taşıyan örnek çıkışı için yazılır:
/// sabit bağlanmış saatsiz çocuğun çıkışı Timeless'tır, geçişin kaynağı
/// başka bir işlenendir.
#[test]
fn the_origin_note_names_only_an_output_that_carries_the_domain() {
    let src = format!(
        "{TWO_DOMAINS}module Pass {{\n    in  a : bool\n    out y : bool\n    y = a\n}}\n\
         module Cross {{\n    in  _fclk : clock @Fast\n    in  sclk  : clock @Slow\n    in  f     : bool  @Fast\n    out y     : bool  @Slow\n    let p = Pass {{ a: true }}\n    y = f && p.y\n}}\n"
    );
    let d = the_error(&check_src("note", &src), "E3001");
    assert!(!note(&d, "note").contains("carries"), "{d}");
}

/// Birleşme tanısının ikincil etiketi ilk alanın İLK bağlamasıdır; aynı
/// alandan ikinci bir giriş onu değiştirmez.
#[test]
fn the_merge_error_labels_the_first_binding_of_the_first_domain() {
    let src = format!(
        "{TWO_DOMAINS}module And3 {{\n    in  a : bool\n    in  b : bool\n    in  c : bool\n    out y : bool\n    y = a && b && c\n}}\n\
         module Merge {{\n    in  _fc : clock @Fast\n    in  _sc : clock @Slow\n    in  f1  : bool  @Fast\n    in  f2  : bool  @Fast\n    in  s   : bool  @Slow\n    let g = And3 {{ a: f1, b: f2, c: s }}\n}}\n"
    );
    let d = the_error(&check_src("first", &src), "E3001");
    let line = src
        .lines()
        .position(|l| l.contains("let g = And3"))
        .unwrap() as u64
        + 1;
    let col = src
        .lines()
        .nth(line as usize - 1)
        .unwrap()
        .find("a: f1")
        .unwrap() as u64
        + 1;
    let fast = d["spans"]
        .as_array()
        .expect("spans")
        .iter()
        .find(|s| s["label"] == "@Fast")
        .cloned()
        .unwrap_or_else(|| panic!("@Fast etiketi yok: {d}"));
    assert_eq!(fast["start"]["line"].as_u64(), Some(line), "{d}");
    assert_eq!(fast["start"]["col"].as_u64(), Some(col), "{d}");
}

/// Saatsiz çocuğun içinde değerin geçtiği her blok biçimi (ADR-0114 §1):
/// `comb` ataması ve blok `let`'i değeri taşır; `if`/`else if` koşulu,
/// `match` sınananı ve kol muhafızı işlenendir (domain-inference.md K5,
/// ADR-0083 Karar 8); kol gövdesindeki ve `comb` içi `for`daki atama
/// değeri taşır; `if` ve `match` ifadelerinin her parçası işlenendir.
/// Beklenen kod şartnameden, elle: `f` @Fast ve `fk` @Fast, `o` @Slow;
/// öteki girişler sabittir (Timeless, K6 muaf), dolayısıyla `p.y` @Fast
/// olur ve `o = p.y` satırında E3001 (K8b). Güvenli ikizler aynı blokları
/// kullanır ama `y` yalnız @Slow ya da sabit bir girişe bağlıdır: tanı yok
/// (K2, port başına bağımlılık).
#[test]
fn a_value_through_a_block_of_a_clockless_child_carries_its_domain() {
    // (ad, çocuğun gövdesi, bağlamalar, E3001 bekleniyor mu)
    let cases: &[(&str, &str, &str, bool)] = &[
        ("comb", "comb {\n        y = a\n        z = false\n    }", "a: f, b: true, k: 0", true),
        ("block-let", "comb {\n        let t = !a\n        y = t\n        z = false\n    }", "a: f, b: true, k: 0", true),
        ("if-cond", "comb {\n        if a {\n            y = true\n        } else {\n            y = false\n        }\n        z = false\n    }", "a: f, b: true, k: 0", true),
        ("else-if-cond", "comb {\n        if b {\n            y = true\n        } else if a {\n            y = false\n        } else {\n            y = true\n        }\n        z = false\n    }", "a: f, b: true, k: 0", true),
        ("if-branch", "comb {\n        if b {\n            y = a\n        } else {\n            y = false\n        }\n        z = false\n    }", "a: f, b: true, k: 0", true),
        ("match-scrutinee", "comb {\n        match k {\n            0 => { y = b }\n            _ => { y = false }\n        }\n        z = false\n    }", "a: true, b: true, k: fk", true),
        ("match-guard", "comb {\n        match k {\n            0 if a => { y = true }\n            _ => { y = false }\n        }\n        z = false\n    }", "a: f, b: true, k: 0", true),
        ("match-arm", "comb {\n        match k {\n            0 => { y = a }\n            _ => { y = false }\n        }\n        z = false\n    }", "a: f, b: true, k: 0", true),
        ("comb-for", "comb {\n        y = false\n        for i in 0..2 {\n            if a {\n                y = true\n            }\n        }\n        z = false\n    }", "a: f, b: true, k: 0", true),
        ("if-expr", "y = if a { true } else { b }\n    z = false", "a: f, b: true, k: 0", true),
        ("match-expr", "y = match k { 0 => b, _ => false }\n    z = false", "a: true, b: true, k: fk", true),
        // Güvenli ikizler: aynı biçimler, @Fast giriş yalnız `z`ye gider.
        ("comb-twin", "comb {\n        y = b\n        z = a\n    }", "a: f, b: s, k: 0", false),
        ("if-twin", "comb {\n        if b {\n            y = true\n        } else {\n            y = false\n        }\n        z = a\n    }", "a: f, b: s, k: 0", false),
        ("match-twin", "comb {\n        match k {\n            0 => { y = b }\n            _ => { y = false }\n        }\n        z = a\n    }", "a: f, b: s, k: 0", false),
    ];
    for (name, body, binds, crossing) in cases {
        let src = format!(
            "{TWO_DOMAINS}module C {{\n    in  a : bool\n    in  b : bool\n    in  k : u2\n    out y : bool\n    out z : bool\n    {body}\n}}\n\
             module Top {{\n    in  _fclk : clock @Fast\n    in  sclk  : clock @Slow\n    in  f     : bool  @Fast\n    in  fk    : u2    @Fast\n    in  s     : bool  @Slow\n    out o     : bool  @Slow\n    out q     : bool  @Fast\n    let p = C {{ {binds} }}\n    o = p.y\n    q = p.z\n}}\n"
        );
        let env = check_src(name, &src);
        let errors: Vec<Value> = diagnostics(&env)
            .into_iter()
            .filter(|d| d["severity"] == "error")
            .collect();
        if *crossing {
            assert_eq!(errors.len(), 1, "{name}: tek E3001 bekleniyor: {env}");
            assert_eq!(errors[0]["code"], "E3001", "{name}: {env}");
            let line = src.lines().position(|l| l.trim() == "o = p.y").unwrap() as u64 + 1;
            let prim = errors[0]["spans"]
                .as_array()
                .and_then(|s| s.iter().find(|s| s["primary"] == true))
                .expect("birincil etiket");
            assert_eq!(prim["start"]["line"].as_u64(), Some(line), "{name}: {env}");
        } else {
            assert!(errors.is_empty(), "{name}: tanı beklenmiyor: {env}");
        }
    }
}

/// Hata önem derecesindeki tanılar.
fn errors_of(env: &Value) -> Vec<Value> {
    diagnostics(env)
        .into_iter()
        .filter(|d| d["severity"] == "error")
        .collect()
}

/// Birincil etiketin satırı (1 tabanlı).
fn primary_line(d: &Value) -> Option<u64> {
    d["spans"]
        .as_array()
        .and_then(|s| s.iter().find(|s| s["primary"] == true))
        .and_then(|s| s["start"]["line"].as_u64())
}

/// K8b: saatsiz Volt modülünün `@Alan` anotasyonlu çıkışı kendi alanında
/// kalır, ama bağlı olduğu anotasyonsuz girişlere o alanın değeri
/// bağlanmalıdır; başka alandan gelen değer kapının içinden
/// senkronizörsüz geçer: E3001 örnek sitesinde, uyuşmayan bağlamada, çıkış
/// başına bir kez. Okuma anotasyonun alanını taşır (kaskad yok).
/// Anotasyonlu giriş modülün içinde denetlenir; saatsiz extern'de
/// anotasyonlu çıkışın bağımlılıkları kullanıcının beyanıdır,
/// denetlenmez; anotasyonlu girişine bağlanan değer denetlenir (K8 adım 2).
#[test]
fn an_annotated_output_of_a_clockless_child_is_checked_against_its_inputs() {
    assert_ui_fail("254_clockless_child_annotated_output.volt");

    let top = |child: &str, binds: &str| {
        format!(
            "{TWO_DOMAINS}{child}
             module Top {{
    in  _fclk : clock @Fast
    in  _sclk : clock @Slow
    in  f     : bool  @Fast
    in  s     : bool  @Slow
    out o     : bool  @Fast
    let p = P {{ {binds} }}
    o = p.y
}}
"
        )
    };
    let binding_line = |src: &str| {
        src.lines()
            .position(|l| l.trim().starts_with("let p = P"))
            .unwrap() as u64
            + 1
    };
    // (ad, çocuk, bağlamalar, beklenen E3001 satırı: örnek mi, çocuk mu, yok mu)
    let gate = "module P {
    in  a : bool
    in  b : bool
    out y : bool @Fast
    y = a && !b
}";
    let chain = "module P {
    in  a : bool
    out y : bool @Fast
    let t = !a
    comb {
        y = t
    }
}";
    let nested = "module Q {
    in  a : bool
    out y : bool
    y = a
}
module P {
    in  a : bool
    out y : bool @Fast
    let q = Q { a: a }
    y = q.y
}";
    let annotated_in = "module P {
    in  a : bool @Slow
    out y : bool @Fast
    y = a
}";
    let ext = "extern module P {
    in  a : bool
    out y : bool @Fast
}";
    let ext_in = "extern module P {
    in  a : bool @Fast
    out y : bool @Fast
}";
    let cases: &[(&str, &str, &str, Option<bool>)] = &[
        // Tehlike: iki giriş de @Slow, tek E3001 (çıkış başına bir kez).
        ("gate-both-slow", gate, "a: s, b: s", Some(true)),
        ("gate-second-slow", gate, "a: f, b: s", Some(true)),
        ("let-comb-chain", chain, "a: s", Some(true)),
        ("nested-child", nested, "a: s", Some(true)),
        // Anotasyonlu giriş: modülün içinde E3001 (K6), örnekte ikinci yok.
        ("annotated-input", annotated_in, "a: s", Some(false)),
        // Güvenli ikizler: aynı alan ya da sabit.
        ("gate-fast", gate, "a: f, b: f", None),
        ("gate-constant", gate, "a: f, b: false", None),
        // Saatsiz extern: anotasyon kullanıcının beyanıdır.
        ("clockless-extern", ext, "a: s", None),
        // ...ama anotasyonlu girişe bağlanan değer K8 adım 2'deki gibi
        // denetlenir: @Fast girişe @Slow değer E3001.
        (
            "clockless-extern-annotated-input",
            ext_in,
            "a: s",
            Some(true),
        ),
        (
            "clockless-extern-annotated-input-same",
            ext_in,
            "a: f",
            None,
        ),
    ];
    for (name, child, binds, expect) in cases {
        let src = top(child, binds);
        let env = check_src(name, &src);
        let errors = errors_of(&env);
        match expect {
            Some(at_instance) => {
                assert_eq!(errors.len(), 1, "{name}: tek E3001 bekleniyor: {env}");
                assert_eq!(errors[0]["code"], "E3001", "{name}: {env}");
                let line = primary_line(&errors[0]);
                if *at_instance {
                    assert_eq!(line, Some(binding_line(&src)), "{name}: {env}");
                } else {
                    assert!(
                        line < Some(binding_line(&src)),
                        "{name}: hata çocukta olmalı: {env}"
                    );
                }
            }
            None => assert!(errors.is_empty(), "{name}: tanı beklenmiyor: {env}"),
        }
    }
}

/// `ready` → `valid` yolu bir çocuğun içinden geçer; çocuk `ready`'yi
/// `sync()` ya da `sync3()` ile okursa yol iki ya da üç flop'tan geçer ve
/// kesilir (domain-inference.md K8b: "`sync()`/`sync3()` yolu KESER"):
/// E4007 yok. Senkronizörsüz ikizde yol kombinasyoneldir: E4007 (ADR-0050
/// §4, ADR-0114 §3). Beklenen kodlar şartnameden, elle.
#[test]
fn sync_and_sync3_in_a_child_cut_the_handshake_path() {
    // (ad, çocuktaki `ready` okuması, E4007 bekleniyor mu)
    let cases: &[(&str, &str, bool)] = &[
        ("hs-sync", "let s = sync(b, clk)\n    y = a && s", false),
        ("hs-sync3", "let s = sync3(b, clk)\n    y = a && s", false),
        ("hs-direct", "y = a && b", true),
    ];
    for (name, body, e4007) in cases {
        let src = format!(
            "module C {{\n    in  clk : clock\n    in  a   : bool\n    in  b   : bool\n    out y   : bool\n    {body}\n}}\n\
             module Producer {{\n    in  clk  : clock\n    in  have : bool\n    out tx   : Handshake<u8>\n    reg count : u8 = 0\n    on clk {{\n        if tx.fired {{\n            count <= count + 1\n        }}\n    }}\n    let g = C {{ clk: clk, a: have, b: tx.ready }}\n    tx.valid = g.y\n    tx.data  = count\n}}\n"
        );
        let env = check_src(name, &src);
        let errors = errors_of(&env);
        if *e4007 {
            assert_eq!(errors.len(), 1, "{name}: tek E4007 bekleniyor: {env}");
            assert_eq!(errors[0]["code"], "E4007", "{name}: {env}");
        } else {
            assert!(errors.is_empty(), "{name}: yol kesilmeli: {env}");
        }
    }
}

/// K8b: saatsiz Volt modülünün `@Alan` anotasyonlu girişi, anotasyonsuz
/// çıkışın join'ine bağlanan değerle değil kendi alanıyla katılır
/// (domain-inference.md K8 adım 3, "anotasyonlu giriş kendi alanıyla
/// katılır"). `P` `in a : bool @Fast`, `y = a`: `p.y` @Fast taşır; @Slow
/// çıkışa atanması E3001, o satırda. Sabit bağlansa da: sabit Timeless'tır
/// ama girişin alanı anotasyondan gelir. Güvenli ikiz: @Fast çıkışa atama.
#[test]
fn an_annotated_input_of_a_clockless_child_joins_with_its_own_domain() {
    // (ad, bağlama, okuyan çıkışın alanı, E3001 bekleniyor mu)
    let cases: &[(&str, &str, &str, bool)] = &[
        ("annin-fast-value", "fast_flag", "Slow", true),
        ("annin-constant", "true", "Slow", true),
        ("annin-same-domain", "true", "Fast", false),
    ];
    for (name, bind, out_dom, crossing) in cases {
        let src = format!(
            "{TWO_DOMAINS}module P {{\n    in  a : bool @Fast\n    out y : bool\n    y = a\n}}\n\
             module Top {{\n    in  _fast_clk : clock @Fast\n    in  _slow_clk : clock @Slow\n    in  fast_flag : bool  @Fast\n    out o         : bool  @{out_dom}\n    out keep      : bool  @Fast\n    let p = P {{ a: {bind} }}\n    o = p.y\n    keep = fast_flag\n}}\n"
        );
        let env = check_src(name, &src);
        let errors = errors_of(&env);
        if *crossing {
            assert_eq!(errors.len(), 1, "{name}: tek E3001 bekleniyor: {env}");
            assert_eq!(errors[0]["code"], "E3001", "{name}: {env}");
            let line = src.lines().position(|l| l.trim() == "o = p.y").unwrap() as u64 + 1;
            assert_eq!(primary_line(&errors[0]), Some(line), "{name}: {env}");
        } else {
            assert!(errors.is_empty(), "{name}: tanı beklenmiyor: {env}");
        }
    }
}

/// BİLİNEN TUTUCULUK (#195): güven seviyesi (K11) modül özetini kullanmaz
/// (domain-inference.md K11 ve K8b: "Güven seviyesi (K11) özeti
/// kullanmaz"). Örnek çıkışı, çocuğun sınıflandırılmış girişlerinin en
/// yükseğini alır, çıkış o girişe bağlı olmasa da: `Two`'da `y2 = b`
/// yalnız @Debug `ctl`'e bağlıdır, yine de `t.y2` @SecureCore `key`'in
/// `secret` seviyesini taşır ve @Debug (public) çıkışa atanması E3009'dur.
/// Bu test bugünkü şartnameyi bekler; #195 çözülüp K11 port başına özete
/// geçtiğinde bu beklenti bilerek değişecek (tanı yok olacak) ve test o
/// PR'da güncellenecek.
#[test]
fn trust_ignores_the_summary_known_conservative_until_195() {
    let src = "domain SecureCore { clock = posedge, reset = sync active_high, trust_level = secret }\n\
               domain Debug      { clock = posedge, reset = sync active_high, trust_level = public }\n\
               module Two {\n    in  a  : u8\n    in  b  : u8\n    out y1 : u8\n    out y2 : u8\n    y1 = a\n    y2 = b\n}\n\
               module Top {\n    in  key : u8 @SecureCore\n    in  ctl : u8 @Debug\n    out dbg : u8 @Debug\n    let t = Two { a: key, b: ctl }\n    dbg = t.y2\n}\n";
    let env = check_src("trust-195", src);
    let errors = errors_of(&env);
    assert_eq!(errors.len(), 1, "tek E3009 bekleniyor: {env}");
    assert_eq!(errors[0]["code"], "E3009", "{env}");
    let line = src.lines().position(|l| l.trim() == "dbg = t.y2").unwrap() as u64 + 1;
    assert_eq!(primary_line(&errors[0]), Some(line), "{env}");
}

/// `sync()` kaynağı bileşik olamaz: `sync(p.y, clk)` E0003 alır
/// (domain-inference.md K8b, ADR-0114 §2); E3001'in önerisi bu yüzden
/// çıkışı önce bir `let` ile adlandırır. İkiz: `let` ile adlandırılmış
/// kaynak temiz derlenir.
#[test]
fn sync_of_an_instance_output_is_e0003_until_named_with_let() {
    let src = |tail: &str| {
        format!(
            "{TWO_DOMAINS}module Pass {{\n    in  a : bool\n    out y : bool\n    y = a\n}}\n\
             module Top {{\n    in  _fast_clk : clock @Fast\n    in  slow_clk  : clock @Slow\n    in  fast_flag : bool  @Fast\n    out slow_flag : bool  @Slow\n    let p = Pass {{ a: fast_flag }}\n    {tail}\n}}\n"
        )
    };
    let bad = src("slow_flag = sync(p.y, slow_clk)");
    let env = check_src("sync-compound", &bad);
    let errors = errors_of(&env);
    assert_eq!(errors.len(), 1, "tek E0003 bekleniyor: {env}");
    assert_eq!(errors[0]["code"], "E0003", "{env}");
    let line = bad.lines().position(|l| l.contains("sync(p.y")).unwrap() as u64 + 1;
    assert_eq!(primary_line(&errors[0]), Some(line), "{env}");

    let good = src("let p_y_raw = p.y\n    slow_flag = sync(p_y_raw, slow_clk)");
    let env = check_src("sync-named", &good);
    assert!(errors_of(&env).is_empty(), "tanı beklenmiyor: {env}");
}
