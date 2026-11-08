//! W2015 (ADR-0112, #173, #150'nin geçici uyarısı): `/` ya da `%`'nin
//! işleneni, doğal genişliği bölmenin yapıldığı genişlikten geniş bir
//! aritmetik değerse bölmeden önce kesilir; derleyici uyarır, reddetmez.
//!
//! Bölmenin genişliği (type-inference.md §3.3, ADR-0025): `/` ve `%`
//! genişlemez, iki işlenenin aralıklarının kesişiminde (üst ucu)
//! yapılır. `a * b` (a, b : u8) esnek u8..u16, `/ c` (c : u8) 8 bitte:
//! `a * b` 16 bitten 8 bite kesilir. Check kipinde itme (ADR-0041,
//! type-inference §5) işlenenler hedefe sığıyorsa bölmeyi hedef genişlikte
//! yapar; o zaman hiçbir şey kesilmez, uyarı yok.
//!
//! Uyarının metni kuralın v0.2.0'da değişeceğini (#150) ve genişliğin açık
//! yazılmasını söyler. Üretilen SV değişmez.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-divcut-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// `volt <cmd> --format json` `file` üzerinde; (çıkış kodu, zarf).
fn run_json(cmd: &str, file: &Path, target: Option<&Path>) -> (Option<i32>, Value) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_volt"));
    c.env("VOLT_LANG", "en")
        .env("NO_COLOR", "1")
        .args([cmd, "--format", "json"]);
    if let Some(t) = target {
        c.arg("--target-dir").arg(t);
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

/// W2015'lerin (satır, sütun, ileti) listesi.
fn w2015(env: &Value) -> Vec<(u64, u64, String)> {
    env["diagnostics"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .filter(|d| d["code"] == "W2015")
        .map(|d| {
            (
                d["spans"][0]["start"]["line"].as_u64().unwrap_or(0),
                d["spans"][0]["start"]["col"].as_u64().unwrap_or(0),
                d["message"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

/// `ports` + tek atama `out o : <ty> = <expr>`; `volt check` ve `volt
/// build` (uyarı reddetmez: ikisi de çıkış 0) sonucu W2015 listesi.
fn warnings(tag: &str, ports: &str, ty: &str, expr: &str) -> Vec<(u64, u64, String)> {
    let dir = temp_dir(tag);
    let file = dir.join("d.volt");
    std::fs::write(
        &file,
        format!("module D {{\n{ports}\n    out o : {ty}\n    o = {expr}\n}}\n"),
    )
    .expect("yaz");
    let (code, env) = run_json("check", &file, None);
    assert_eq!(code, Some(0), "{env:#}");
    let (bcode, benv) = run_json("build", &file, Some(&dir.join("build")));
    assert_eq!(bcode, Some(0), "{benv:#}");
    let w = w2015(&env);
    assert_eq!(w, w2015(&benv), "check ve build aynı uyarıları vermeli");
    let _ = std::fs::remove_dir_all(&dir);
    w
}

const U8S: &str = "    in a : u8\n    in b : u8\n    in c : u8\n    in d : u8";

/// İletinin iki zorunlu parçası: kesilme ve v0.2.0'daki değişiklik (#150).
fn assert_message(msg: &str, op: &str, width: u32, natural: u32) {
    assert!(
        msg.contains(&format!(
            "this operand of '{op}' is cut to {width} bits before the division (its natural width is {natural})"
        )),
        "{msg}"
    );
    assert!(msg.contains("the rule changes in v0.2.0 (#150)"), "{msg}");
}

// ═══ #150'nin durumları: uyarı var ═══════════════════════════════════════

/// #150'nin örneği: `a * b / c + d` (o : u9). `a * b` u8..u16, `/ c` 8
/// bitte: `a * b` 16 bitten 8 bite kesilir (sütun 9, `a`). Üstteki `+`
/// itilir (ADR-0041), bölme itilmez (`a * b` 9 bite sığmaz).
#[test]
fn issue_150_product_divided_in_a_sum_warns() {
    let w = warnings("prod", U8S, "u9", "a * b / c + d");
    assert_eq!(w.len(), 1, "{w:?}");
    assert_eq!((w[0].0, w[0].1), (7, 9), "{w:?}");
    assert_message(&w[0].2, "/", 8, 16);
}

/// Toplamın taşma biti: `(a + b) / c`, o : u8. Bölme 8 bitte, `a + b`
/// 9 bitten kesilir.
#[test]
fn sum_divided_at_its_operand_width_warns() {
    let w = warnings("sum", U8S, "u8", "(a + b) / c");
    assert_eq!(w.len(), 1, "{w:?}");
    assert_message(&w[0].2, "/", 8, 9);
}

/// `%` de: `(a + b) % c`.
#[test]
fn remainder_warns_too() {
    let w = warnings("rem", U8S, "u8", "(a + b) % c");
    assert_eq!(w.len(), 1, "{w:?}");
    assert_message(&w[0].2, "%", 8, 9);
}

/// Bölen tarafı: `c / (a + b)`.
#[test]
fn divisor_operand_warns() {
    let w = warnings("divisor", U8S, "u8", "c / (a + b)");
    assert_eq!(w.len(), 1, "{w:?}");
    assert_eq!(w[0].1, 14, "{w:?}");
    assert_message(&w[0].2, "/", 8, 9);
}

/// İki işlenen de esnek: yalnız bölmenin genişliğinden geniş olan uyarı
/// alır.
#[test]
fn only_the_operand_wider_than_the_division_warns() {
    let w = warnings("both", U8S, "u8", "(a + b) / (c + d)");
    // (a + b) ve (c + d) aynı aralıkta (u8..u9): kesişim 8..9, bölme 9
    // bitte, hiçbiri kesilmez.
    assert!(w.is_empty(), "{w:?}");
    let w = warnings("both2", U8S, "u8", "(a * b) / (c + d)");
    // u8..u16 ile u8..u9: kesişim 8..9, bölme 9 bitte; `a * b` 16 bitten
    // kesilir, `c + d` kesilmez.
    assert_eq!(w.len(), 1, "{w:?}");
    assert_message(&w[0].2, "/", 9, 16);
}

/// İşaretli: `(x * y) / z`, üçü i8: i8..i16, bölme 8 bitte.
#[test]
fn signed_product_divided_warns() {
    let w = warnings(
        "signed",
        "    in x : i8\n    in y : i8\n    in z : i8",
        "i8",
        "(x * y) / z",
    );
    assert_eq!(w.len(), 1, "{w:?}");
    assert_message(&w[0].2, "/", 8, 16);
}

/// #150'nin fark testi bulgusundaki biçim: `p * p / (p & p)` (p : u21).
#[test]
fn issue_150_difftest_finding_shape_warns() {
    let w = warnings("finding", "    in p : u21", "u21", "p * p / (p & p)");
    assert_eq!(w.len(), 1, "{w:?}");
    assert_message(&w[0].2, "/", 21, 42);
}

/// Tipsiz `let`'in esnek değeri de aynı biçimde kesilir: `let s = a + b`
/// (u8..u9), `s / c` 8 bitte.
#[test]
fn untyped_let_of_a_sum_warns() {
    let dir = temp_dir("let");
    let file = dir.join("d.volt");
    std::fs::write(
        &file,
        format!("module D {{\n{U8S}\n    out o : u8\n    let s = a + b\n    o = s / c\n}}\n"),
    )
    .expect("yaz");
    let (code, env) = run_json("check", &file, None);
    assert_eq!(code, Some(0), "{env:#}");
    let w = w2015(&env);
    assert_eq!(w.len(), 1, "{w:?}");
    assert_eq!((w[0].0, w[0].1), (8, 9), "{w:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

// ═══ Uyarı yok ═══════════════════════════════════════════════════════════

/// Aynı genişlikte atom işlenenler.
#[test]
fn same_width_atoms_do_not_warn() {
    assert!(warnings("atoms", U8S, "u8", "a / c").is_empty());
    assert!(warnings("atoms-rem", U8S, "u8", "a % c").is_empty());
}

/// Literal işlenen karşı yana uyarlanır (§4): `(a + b) / 3` 9 bitte,
/// hiçbir şey kesilmez. Literal aritmetiği de (#144) literaldir.
#[test]
fn literal_operands_do_not_warn() {
    assert!(warnings("lit", U8S, "u9", "(a + b) / 3").is_empty());
    assert!(warnings("lit2", U8S, "u8", "a / 3").is_empty());
    assert!(warnings("litarith", U8S, "u8", "a / (6 / 2)").is_empty());
}

/// Const işlenen somut tiplidir.
#[test]
fn const_operands_do_not_warn() {
    let dir = temp_dir("const");
    let file = dir.join("d.volt");
    std::fs::write(
        &file,
        format!("const K : u8 = 3\n\nmodule D {{\n{U8S}\n    out o : u8\n    o = a / K\n}}\n"),
    )
    .expect("yaz");
    let (code, env) = run_json("check", &file, None);
    assert_eq!(code, Some(0), "{env:#}");
    assert!(w2015(&env).is_empty(), "{env:#}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// İtilen bölme (check kipi, ADR-0041): işlenenler hedefe (u9) sığıyor,
/// bölme 9 bitte, `a + b` kesilmez.
#[test]
fn pushed_division_does_not_warn() {
    assert!(warnings("pushed", U8S, "u9", "(a + b) / c").is_empty());
}

/// Uyarının önerdiği biçim: genişlik açık, uyarı yok.
#[test]
fn explicit_width_does_not_warn() {
    assert!(warnings("explicit", U8S, "u9", "(a + b) as u9 / (c as u9)").is_empty());
    assert!(warnings("explicit16", U8S, "u16", "(a * b) as u16 / (c as u16)").is_empty());
}

/// Uyarı reddetmez: `volt build` SV yazar (yukarıdaki her durumda çıkış
/// 0, `warnings`); burada dosyanın varlığı da.
#[test]
fn warning_does_not_reject() {
    let dir = temp_dir("build");
    let file = dir.join("d.volt");
    std::fs::write(
        &file,
        format!("module D {{\n{U8S}\n    out o : u9\n    o = a * b / c + d\n}}\n"),
    )
    .expect("yaz");
    let (code, env) = run_json("build", &file, Some(&dir.join("build")));
    assert_eq!(code, Some(0), "{env:#}");
    assert_eq!(w2015(&env).len(), 1, "{env:#}");
    assert!(dir.join("build/rtl/D.sv").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

// ═══ tests/ui ═══════════════════════════════════════════════════════════

/// ui/fail 245: W2015 `//~^ ERROR` satırının üstündeki satırda; çıkış 0.
#[test]
fn ui_fail_245_division_operand_cut_w2015() {
    let file = root().join("tests/ui/fail/245_division_operand_cut.volt");
    let src = std::fs::read_to_string(&file).expect("fixture");
    let marker = src
        .lines()
        .position(|l| l.trim_start().starts_with("//~^ ERROR"))
        .expect("//~^ ERROR satırı") as u64; // üst satır (1 tabanlı)
    let (code, env) = run_json("check", &file, None);
    assert_eq!(code, Some(0), "{env:#}");
    let w = w2015(&env);
    assert_eq!(
        w.iter().map(|d| d.0).collect::<Vec<_>>(),
        [marker],
        "{env:#}"
    );
    assert_message(&w[0].2, "/", 8, 16);
}

/// ui/pass 140: genişliği açık bölmeler, W2015 yok.
#[test]
fn ui_pass_140_explicit_width_has_no_w2015() {
    let file = root().join("tests/ui/pass/140_division_operand_explicit_width.volt");
    let (code, env) = run_json("check", &file, None);
    assert_eq!(code, Some(0), "{env:#}");
    assert!(w2015(&env).is_empty(), "{env:#}");
}

// ═══ Depodaki .volt dosyaları ═══════════════════════════════════════════

/// `dir` altındaki `.volt` dosyaları (özyineli; `target` ve `build`
/// dizinleri hariç).
fn volt_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if p.is_dir() {
            if name != "target" && name != "build" && !name.starts_with('.') {
                volt_files(&p, out);
            }
        } else if p.extension().is_some_and(|x| x == "volt") {
            out.push(p);
        }
    }
}

/// Etki analizi (ADR-0112): depodaki hiçbir `.volt` dosyası W2015 almaz
/// (uyarıyı bilerek taşıyan ui/fail 245 dışında).
/// Örnekler, kitap, şablonlar ve testlerin fikstürleri bölmeyi atom ya
/// da literal işlenenle yazıyor.
#[test]
fn no_volt_file_in_the_repository_gets_w2015() {
    let mut files = Vec::new();
    for d in ["examples", "tests", "templates", "book", "demo", "crates"] {
        volt_files(&root().join(d), &mut files);
    }
    files.sort();
    assert!(files.len() > 500, "{} dosya", files.len());
    let mut hits = Vec::new();
    let mut expected = 0;
    for f in &files {
        // W2015'i bilerek taşıyan fikstür (ui/fail 245) kendi testinde.
        let text = std::fs::read_to_string(f).unwrap_or_default();
        if text.starts_with("//~ W2015") {
            expected += 1;
            continue;
        }
        let (_, env) = run_json("check", f, None);
        for (l, c, _) in w2015(&env) {
            hits.push(format!("{}:{l}:{c}", f.display()));
        }
    }
    eprintln!(
        "division_cut_warning_tests: {} .volt dosyası denetlendi, W2015: {} (W2015 bekleyen fikstür: {expected})",
        files.len() - expected,
        hits.len()
    );
    assert_eq!(expected, 1, "W2015 bekleyen fikstür sayısı");
    assert!(hits.is_empty(), "{}", hits.join("\n"));
}
