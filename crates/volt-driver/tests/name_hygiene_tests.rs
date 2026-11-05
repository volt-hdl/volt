//! Ad çözümü uçtan uca (#128): açılmış fn gövdesinin adları fn
//! kapsamında, blok adları kapsam zincirinde çözülür (name-resolution.md
//! §2, §3.1; ADR-0081 Karar 12.1). Çözüm SV'yi değiştirir, uyarıyı
//! değiştirmez: iç kapsamın dış adı gölgelediği her yerde W1002 çıkmaya
//! devam eder (§6 "aynı kapsamda yasak, iç kapsamda izinli + uyarı"),
//! başka yerde çıkmaz. Bu dosya tanıları ve derlemenin başarısını sınar;
//! SV metnini karşılaştırmaz. Çözümün anlamını (simülasyon değerleri)
//! `name_scope_sim_tests` sınar; ui/pass/135'in SV'si `output_net_tests`'te
//! Verilator `-Wall` ile lint'lenir.

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

/// `volt build` başarılı ve `module`'ün SV dosyası yazıldı.
fn build_ok(file: &Path, target: &Path, module: &str) {
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
    assert!(
        target.join("rtl").join(format!("{module}.sv")).is_file(),
        "{module}.sv yazılmalı"
    );
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
/// yok, derlenir. (Değeri, her k için `ys[k] = xs[1]`:
/// `name_scope_sim_tests::issue_128_…`.)
#[test]
fn ui_fail_224_warns_w1002_and_builds() {
    let file = root().join("tests/ui/fail/224_loop_var_shadows_inlined_const.volt");
    let text = std::fs::read_to_string(&file).expect("okunmalı");
    let marked = lines_with(&text, "//~^ ERROR 'I' shadows")[0] - 1;
    let env = check_json(&file);
    assert_eq!(w1002_lines(&env), vec![marked], "{:?}", diags(&env));
    assert!(errors(&env).is_empty(), "{:?}", errors(&env));
    let target = temp_dir("224");
    build_ok(&file, &target, "LoopVarShadowsInlinedConst");
    let _ = std::fs::remove_dir_all(&target);
}

/// ui/pass/135: her gölgeleme bir W1002 (port `W`, üç döngü değişkeni),
/// hata yok, derlenir. (Gövdelerin const'larını okuduğunu
/// `name_scope_sim_tests` simüle eder; SV `output_net_tests`'te lint'lenir.)
#[test]
fn ui_pass_135_warns_once_per_shadowing_and_builds() {
    let file = root().join("tests/ui/pass/135_fn_const_name_hygiene.volt");
    let text = std::fs::read_to_string(&file).expect("okunmalı");
    let mut expected = lines_with(&text, "in  W  : u8");
    expected.extend(lines_with(&text, "for I in"));
    expected.extend(lines_with(&text, "for T in"));
    expected.sort_unstable();
    let env = check_json(&file);
    assert_eq!(w1002_lines(&env), expected, "{:?}", diags(&env));
    assert!(errors(&env).is_empty(), "{:?}", errors(&env));
    let target = temp_dir("135");
    build_ok(&file, &target, "FnConstHygiene");
    let _ = std::fs::remove_dir_all(&target);
}

/// §6: W1002 iç kapsamdaki bildirimin satırında — fn parametresi ve fn
/// `let`'i kök const'u (fn kapsamının ebeveyni kök), blok `let`'i ve
/// döngü değişkeni dıştaki adı gölgeler. Aynı adın gölgelemeyen kullanımı
/// (gövdedeki `x[I]`, çağıranın `ys[i]`'si) uyarı değildir. Her
/// bildirimin kendi bağlamasını okuduğunu aynı tasarımla
/// `name_scope_sim_tests::every_shadowing_declaration_reads_its_own_binding`
/// simüle eder.
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
    build_ok(&file, &dir.join("out"), "M");
    let _ = std::fs::remove_dir_all(&dir);
}
