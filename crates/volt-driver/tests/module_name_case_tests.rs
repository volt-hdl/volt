//! #250: her modül adını taşıyan dosyalara yazılır (`rtl/<Modül>.sv`,
//! `constraints/<Modül>.{sdc,xdc}`); SVA dosyası ve denetleyici modülü
//! adın küçük harflisini kullanır (`formal/<modül>.sva`, `<modül>_sva`).
//! Yalnız harf büyüklüğüyle ayrılan iki modül adı (`Foo`/`foo`, ya da
//! generic örneğin ürettiği `M_8` ile `m_8`) her işletim sisteminde tek
//! SVA dosyası, Windows ve macOS'un büyük/küçük harf duyarsız dosya
//! sisteminde tek RTL ve kısıt dosyası yazıyordu: bir modülün SV'si ya da
//! kontratları sessizce kayboluyordu. Volt modül adını değiştirmez
//! (ADR-0090): E1003.
//!
//! Davranış: tanının kodu ve konumları, hatalı tasarımın hiçbir dosya
//! yazmaması (`--emit sva,sdc,xdc` ile); çok dosyalı birim; ve güvenli
//! ikizde üretilen her dosyanın kendi modülüne ait olması — hiçbir iki
//! çıktı yolu yalnız harf büyüklüğüyle ayrılmaz, her RTL dosyası adındaki
//! modülü bildirir, her modülün kontratları kendi modülüne bağlanır.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Testler paralel koşar: her test kendi `tag`'ini kullanır.
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-case-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// Ek çıktıların hepsi: SVA, SDC, XDC.
const EMIT_ALL: &str = "sva,sdc,xdc";

/// `volt <cmd> --format json <file>` `dir`'de; (çıkış kodu, zarf).
fn run_json(dir: &Path, cmd: &str, file: &Path) -> (Option<i32>, Value) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_volt"));
    c.current_dir(dir)
        .env("VOLT_LANG", "en")
        .env("NO_COLOR", "1")
        .args([cmd, "--format", "json"]);
    if cmd == "build" {
        c.args(["--target-dir", "build", "--emit", EMIT_ALL]);
    }
    let out = c.arg(file).output().expect("volt çalışmalı");
    let env: Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "JSON değil ({e}): {}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    });
    (out.status.code(), env)
}

fn diagnostics(env: &Value) -> &Vec<Value> {
    env["diagnostics"].as_array().expect("tanılar")
}

fn codes(env: &Value) -> Vec<String> {
    diagnostics(env)
        .iter()
        .map(|d| d["code"].as_str().unwrap_or_default().to_string())
        .collect()
}

/// Tanının bütün konumları: (satır, birincil mi).
fn span_lines(d: &Value) -> Vec<(u64, bool)> {
    let mut lines: Vec<(u64, bool)> = d["spans"]
        .as_array()
        .expect("spans")
        .iter()
        .map(|s| {
            (
                s["start"]["line"].as_u64().expect("satır"),
                s["primary"] == true,
            )
        })
        .collect();
    lines.sort_unstable();
    lines
}

/// 1 tabanlı satır numarası: `needle`'ı içeren ilk satır.
fn line_of(src: &str, needle: &str) -> u64 {
    let i = src
        .lines()
        .position(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("satır yok: {needle}"));
    u64::try_from(i + 1).expect("küçük dosya")
}

/// ui/fail fikstürü. Önce `volt build --emit sva,sdc,xdc`: 1 ile çıkar ve
/// hiçbir dosya yazmaz. Düzeltmeden önce 0 ile çıkıyordu; o yolda
/// çıktılar [`assert_each_module_writes_its_own_files`] ile denetlenir ki
/// ezilen dosya adıyla raporlansın. Sonra `volt check`: tek tanı E1003;
/// birincil konum `//~^ ERROR` işaretinin bir üstündeki satırda, öteki
/// konumlar `others`'ın satırlarında; ileti işaretteki metni içerir.
fn assert_case_fixture(name: &str, modules: Modules, others: &[&str]) {
    let file = root().join("tests/ui/fail").join(name);
    let src = std::fs::read_to_string(&file).expect("okunmalı");
    assert!(src.starts_with("//~ E1003"), "{name}");
    let part = src
        .lines()
        .find_map(|l| l.trim_start().strip_prefix("//~^ ERROR "))
        .expect("//~^ ERROR satırı")
        .trim()
        .to_string();

    let dir = temp_dir(name.trim_end_matches(".volt"));
    assert_build_rejected(&dir, &file, modules);

    let (code, env) = run_json(&root(), "check", &file);
    assert_eq!(code, Some(1), "{env}");
    assert_eq!(codes(&env), ["E1003"], "{env}");
    let d = &diagnostics(&env)[0];
    assert!(
        d["message"].as_str().is_some_and(|m| m.contains(&part)),
        "{name}: '{part}' bekleniyor: {d}"
    );
    let mut want = vec![(line_of(&src, "//~^ ERROR") - 1, true)];
    want.extend(others.iter().map(|o| (line_of(&src, o), false)));
    want.sort_unstable();
    assert_eq!(span_lines(d), want, "{d}");
}

/// Birimin modülleri ve kontratlı olanları.
type Modules = (&'static [&'static str], &'static [&'static str]);

/// fail/270, 271 ve çok dosyalı ikiz: `Foo` ve `foo` kontratlı.
const FOO_FOO: Modules = (&["Foo", "Top", "foo"], &["Foo", "foo"]);
/// fail/272 ve 273: `M<8>` → `M_8` ve `m_8`, ikisi de kontratlı.
const M8_M8: Modules = (&["M_8", "Top", "m_8"], &["M_8", "m_8"]);

/// `volt build --emit sva,sdc,xdc` 1 ile çıkar, `dir`'e hiçbir dosya
/// yazmaz. 0 ile çıkarsa (düzeltmeden önceki yol) önce çıktıların her
/// modüle ayrı olup olmadığı denetlenir: ezilen dosya adıyla raporlanır.
fn assert_build_rejected(dir: &Path, file: &Path, (all, with_contracts): Modules) {
    let (code, env) = run_json(dir, "build", file);
    if code == Some(0) {
        assert_each_module_writes_its_own_files(&dir.join("build"), all, with_contracts);
    }
    assert_eq!(code, Some(1), "{env}");
    assert!(
        !dir.join("build").exists(),
        "hatalı tasarım dosya yazmamalı: {env}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn ui_fail_270_module_names_differ_in_case() {
    assert_case_fixture(
        "270_module_names_differ_in_case.volt",
        FOO_FOO,
        &["module Foo {"],
    );
}

#[test]
fn ui_fail_271_module_names_differ_in_case_reversed() {
    assert_case_fixture(
        "271_module_names_differ_in_case_reversed.volt",
        FOO_FOO,
        &["module foo {"],
    );
}

/// Generic örnek: örnek sitesi ve generic modülün adı da etiketlenir.
#[test]
fn ui_fail_272_generic_name_differs_in_case() {
    assert_case_fixture(
        "272_generic_name_differs_in_case.volt",
        M8_M8,
        &["module m_8 {", "module M<"],
    );
}

#[test]
fn ui_fail_273_generic_name_differs_in_case_reversed() {
    assert_case_fixture(
        "273_generic_name_differs_in_case_reversed.volt",
        M8_M8,
        &["let p = M<8>", "module M<"],
    );
}

/// Çok dosyalı birim: `Foo` lib.volt'ta, `foo` main.volt'ta. Tek tanı
/// E1003, iki konum iki dosyada; `volt build` dosya yazmaz.
#[test]
fn multifile_module_names_differ_in_case() {
    let file = root().join("tests/ui/multifile/modulecase/main.volt");
    assert_build_rejected(&temp_dir("multifile"), &file, FOO_FOO);

    let (code, env) = run_json(&root(), "check", &file);
    assert_eq!(code, Some(1), "{env}");
    assert_eq!(codes(&env), ["E1003"], "{env}");
    let mut files: Vec<String> = diagnostics(&env)[0]["spans"]
        .as_array()
        .expect("spans")
        .iter()
        .map(|s| {
            let f = s["file"].as_str().unwrap_or_default().replace('\\', "/");
            f.rsplit('/').next().unwrap_or_default().to_string()
        })
        .collect();
    files.sort();
    assert_eq!(files, ["lib.volt", "main.volt"], "{env}");
}

// ═══ Davranış: üretilen dosyalar birbirinin üstüne yazılmaz ════════════

/// `dir` altındaki dosyalar, `dir`'e göreli ve `/` ayraçlı.
fn files_under(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).expect("dizin").flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let rel = path.strip_prefix(dir).expect("göreli");
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    out.sort();
    out
}

/// `prefix` dizinindeki `.ext` dosyalarının kökleri.
fn stems(files: &[String], prefix: &str, ext: &str) -> BTreeSet<String> {
    files
        .iter()
        .filter_map(|f| f.strip_prefix(prefix)?.strip_suffix(ext))
        .map(str::to_string)
        .collect()
}

/// SV metninde bildirilen modül adları (`module <ad>` satırları).
fn declared_modules(sv: &str) -> Vec<String> {
    sv.lines()
        .filter_map(|l| l.strip_prefix("module "))
        .filter_map(|rest| {
            rest.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .next()
        })
        .map(str::to_string)
        .collect()
}

/// `bind <hedef> ...` satırlarının hedef modülleri.
fn bind_targets(sva: &str) -> Vec<String> {
    sva.lines()
        .filter_map(|l| l.trim_start().strip_prefix("bind "))
        .filter_map(|rest| rest.split_whitespace().next())
        .map(str::to_string)
        .collect()
}

fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|n| n.to_string()).collect()
}

/// `build` dizinindeki çıktılar her modüle ayrı:
/// - hiçbir iki çıktı yolu yalnız harf büyüklüğüyle ayrılmaz (büyük/küçük
///   harf duyarsız dosya sisteminde de ayrı dosyalar);
/// - `rtl/` her modül için bir dosya; her `rtl/<X>.sv` tek modül bildirir
///   ve o modül `X`'tir;
/// - kontratlı her modülün SVA'sı ayrı bir dosyada ve kendi modülüne
///   bağlanır (bir modülün kontratları kaybolmaz);
/// - her modülün SDC ve XDC dosyası vardır ve kendi modülünü adlandırır.
fn assert_each_module_writes_its_own_files(
    build: &Path,
    modules: &[&str],
    with_contracts: &[&str],
) {
    let files = files_under(build);
    let mut folded = BTreeSet::new();
    for f in &files {
        assert!(
            folded.insert(f.to_ascii_lowercase()),
            "'{f}' başka bir çıktıdan yalnız harf büyüklüğüyle ayrılıyor: {files:?}"
        );
    }

    let modules = set(modules);
    assert_eq!(stems(&files, "rtl/", ".sv"), modules, "{files:?}");
    for m in &modules {
        let sv = std::fs::read_to_string(build.join(format!("rtl/{m}.sv"))).expect("rtl");
        assert_eq!(declared_modules(&sv), std::slice::from_ref(m), "rtl/{m}.sv");
    }

    let mut bound = BTreeSet::new();
    for f in files.iter().filter(|f| f.ends_with(".sva")) {
        let sva = std::fs::read_to_string(build.join(f)).expect("sva");
        let targets = bind_targets(&sva);
        assert_eq!(targets.len(), 1, "{f}: {targets:?}");
        assert!(bound.insert(targets[0].clone()), "{f}: iki kez bağlandı");
    }
    assert_eq!(
        bound,
        set(with_contracts),
        "kontratları bir SVA dosyasına bağlanan modüller: {files:?}"
    );

    for ext in [".sdc", ".xdc"] {
        assert_eq!(stems(&files, "constraints/", ext), modules, "{ext}");
        for m in &modules {
            let text = std::fs::read_to_string(build.join(format!("constraints/{m}{ext}")))
                .expect("kısıt");
            assert!(
                text.lines()
                    .any(|l| l.trim_end() == format!("# Module: {m}")),
                "constraints/{m}{ext}"
            );
        }
    }
}

/// Güvenli ikiz (pass/152): adlar harf büyüklüğünden fazlasıyla ayrılıyor;
/// `--emit sva,sdc,xdc` ile her modül kendi dosyalarını yazar. Top'un
/// kontratı yok; öteki altı modülün birer invariant'ı var.
#[test]
fn ui_pass_152_every_module_writes_its_own_files() {
    let file = root().join("tests/ui/pass/152_module_names_distinct_beyond_case.volt");
    let dir = temp_dir("pass152");
    let (code, env) = run_json(&dir, "build", &file);
    assert_eq!(code, Some(0), "{env}");
    assert!(codes(&env).is_empty(), "{env}");
    assert_each_module_writes_its_own_files(
        &dir.join("build"),
        &["Bar", "Foo", "Foo_2", "M_8", "Top", "bar_1", "m_9"],
        &["Bar", "Foo", "Foo_2", "M_8", "bar_1", "m_9"],
    );
    let _ = std::fs::remove_dir_all(&dir);
}
