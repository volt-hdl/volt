//! Ad çözümü uçtan uca (#128): açılmış fn gövdesinin adları fn
//! kapsamında, blok adları kapsam zincirinde çözülür (name-resolution.md
//! §2, §3.1; ADR-0081 Karar 12.1). Çözüm SV'yi değiştirir, uyarıyı
//! değiştirmez: iç kapsamın dış adı gölgelediği her yerde W1002 çıkmaya
//! devam eder (§6 "aynı kapsamda yasak, iç kapsamda izinli + uyarı"),
//! başka yerde çıkmaz. Üretilen SV'nin ayrıntılı denetimi
//! volt-sv-emit'in `name_scope_tests`'indedir.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-hygiene-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
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

/// (önem, kod, birincil satır), kaynak sırasıyla.
fn diags(env: &Value) -> Vec<(String, String, u64)> {
    let mut v: Vec<(String, String, u64)> = env["diagnostics"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .map(|d| {
            let line = d["spans"]
                .as_array()
                .and_then(|s| s.iter().find(|s| s["primary"] == true))
                .and_then(|s| s["start"]["line"].as_u64())
                .unwrap_or(0);
            (
                d["severity"].as_str().unwrap_or_default().to_string(),
                d["code"].as_str().unwrap_or_default().to_string(),
                line,
            )
        })
        .collect();
    v.sort_by_key(|d| d.2);
    v
}

fn w1002_lines(env: &Value) -> Vec<u64> {
    diags(env)
        .into_iter()
        .filter(|(_, c, _)| c == "W1002")
        .map(|(_, _, l)| l)
        .collect()
}

fn errors(env: &Value) -> Vec<(String, u64)> {
    diags(env)
        .into_iter()
        .filter(|(s, _, _)| s == "error")
        .map(|(_, c, l)| (c, l))
        .collect()
}

fn build_sv(file: &Path, target: &Path, module: &str) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "build", "--target-dir"])
        .arg(target)
        .arg(file)
        .output()
        .expect("volt çalışmalı");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    std::fs::read_to_string(target.join("rtl").join(format!("{module}.sv")))
        .unwrap_or_else(|e| panic!("{module}.sv okunmalı: {e}"))
}

/// Satırları 1'den sayılan kaynakta `marker` içeren satırların numaraları.
fn lines_with(src: &str, marker: &str) -> Vec<u64> {
    src.lines()
        .enumerate()
        .filter(|(_, l)| l.contains(marker))
        .map(|(i, _)| i as u64 + 1)
        .collect()
}

/// #128'in fixture'ı: W1002 döngü değişkeninin satırında (işaretli), hata
/// yok; her k için `ys[k] = xs[1]`.
#[test]
fn ui_fail_224_warns_w1002_and_the_function_reads_the_const() {
    let file = root().join("tests/ui/fail/224_loop_var_shadows_inlined_const.volt");
    let text = std::fs::read_to_string(&file).expect("okunmalı");
    let marked = lines_with(&text, "//~^ ERROR 'I' shadows")[0] - 1;
    let env = check_json(&file);
    assert_eq!(w1002_lines(&env), vec![marked], "{:?}", diags(&env));
    assert!(errors(&env).is_empty(), "{:?}", errors(&env));
    let target = temp_dir("224");
    let sv = build_sv(&file, &target, "LoopVarShadowsInlinedConst");
    for k in 0..4 {
        let line = format!("ys[{} +: 8] = xs[8 * (32'd1) +: 8];", 8 * k);
        assert!(sv.lines().any(|l| l.trim() == line), "{line}\n{sv}");
    }
    let _ = std::fs::remove_dir_all(&target);
}

/// ui/pass/135: her gölgeleme bir W1002 (port `W`, dört döngü değişkeni),
/// hata yok; SV'de gövdenin const'ları.
#[test]
fn ui_pass_135_warns_once_per_shadowing_and_builds_with_the_consts() {
    let file = root().join("tests/ui/pass/135_fn_const_name_hygiene.volt");
    let text = std::fs::read_to_string(&file).expect("okunmalı");
    let mut expected = lines_with(&text, "in  W  : u8");
    expected.extend(lines_with(&text, "for I in"));
    expected.extend(lines_with(&text, "for T in"));
    expected.extend(lines_with(&text, "for B in"));
    expected.sort_unstable();
    let env = check_json(&file);
    assert_eq!(w1002_lines(&env), expected, "{:?}", diags(&env));
    assert!(errors(&env).is_empty(), "{:?}", errors(&env));
    let target = temp_dir("135");
    let sv = build_sv(&file, &target, "FnConstHygiene");
    for line in [
        "ys[24 +: 8] = xs[8 * (32'd1) +: 8];",
        "ps[0 +: 2] = a[32'd1 +: 2];",
        "qs[4 +: 4] = 4'(b + 4'd1);",
        "2'd1: cls_0 = 8'd7;",
        "wire [7:0] lut_0 = T_at(s);",
        "bs[1 +: 1] = a[4'd2];",
    ] {
        assert!(sv.lines().any(|l| l.trim() == line), "{line}\n{sv}");
    }
    let _ = std::fs::remove_dir_all(&target);
}

/// §6: W1002 iç kapsamdaki bildirimin satırında — fn parametresi ve fn
/// `let`'i kök const'u (fn kapsamının ebeveyni kök), blok `let`'i ve
/// döngü değişkeni dıştaki adı gölgeler. Aynı adın gölgelemeyen kullanımı
/// (gövdedeki `x[I]`, çağıranın `ys[i]`'si) uyarı değildir.
#[test]
fn w1002_marks_every_inner_declaration_that_shadows_and_only_those() {
    let src = "const I : u32 = 1
const K : u32 = 3
fn by_param(x: [u8; 4], I: u2) -> u8 { x[I] }
fn by_let(x: [u8; 4]) -> u8 {
    let I : u2 = 2
    x[I]
}
fn by_root(x: [u8; 4]) -> u8 { x[I] }
module M {
    in  xs : [u8; 4]
    in  s  : u2
    in  i  : u2
    out ys : [u8; 4]
    out a  : u8
    out b  : u8
    out c  : u8
    out d  : u8
    comb {
        let K = s
        a = xs[K]
        for i in 0..4 {
            let i = s
            ys[i] = by_root(xs)
        }
    }
    b = by_param(xs, 2)
    c = by_let(xs)
    d = xs[i]
}
";
    let dir = temp_dir("w1002");
    let file = dir.join("w1002.volt");
    std::fs::write(&file, src).expect("yazılmalı");
    let env = check_json(&file);
    let mut expected: Vec<u64> = [
        "fn by_param",    // parametre I, kök const I
        "let I : u2 = 2", // fn let'i I, kök const I
        "let K = s",      // blok let'i K, kök const K
        "for i in 0..4",  // döngü değişkeni i, modül portu i
        "let i = s",      // döngü gövdesinin let'i i, döngü değişkeni i
    ]
    .iter()
    .flat_map(|m| lines_with(src, m))
    .collect();
    expected.sort_unstable();
    assert_eq!(w1002_lines(&env), expected, "{:?}", diags(&env));
    assert!(errors(&env).is_empty(), "{:?}", errors(&env));
    let sv = build_sv(&file, &dir.join("out"), "M");
    for line in [
        // blok let'i K = s
        "a = xs[8 * (K) +: 8];",
        // döngü gövdesinin let'i i = s (SV'de port `i` ile çakışmasın diye
        // `i_2`, sv-mapping.md §19); by_root'un I'sı const 1
        "ys[8 * (i_2) +: 8] = xs[8 * (32'd1) +: 8];",
        // modül düzeyinde i porttur
        "assign d = xs[8 * (i) +: 8];",
    ] {
        assert!(sv.lines().any(|l| l.trim() == line), "{line}\n{sv}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
