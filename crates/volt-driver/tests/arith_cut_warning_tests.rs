//! W2016 (ADR-0116, #266'nın geçici uyarısı): `+`, `-` ya da `*`'nın
//! işleneni, doğal genişliği işlemin yapıldığı genişlikten geniş bir
//! aritmetik değerse işlemden önce kesilir; derleyici uyarır, reddetmez.
//!
//! İşlem genişliği (type-inference.md §3.3, ADR-0025): `+ - *` iki
//! işlenenin aralıklarının kesişiminde, kendi taşmasıyla yapılır; `C`
//! kesişimin üst ucu + 1 (`*` için iki katı). `a * b - a` (a, b : i32):
//! `a * b` i32..i64, `- a` 32'de buluşur, işlem 33 bitte, çarpım 64 bitten
//! 33 bite kesilir.
//!
//! Uyarı yok: bildirilmiş genişliğe atanan sonuç (hedef `C` biti ya da
//! daha azını tutar, kesme görünmez), sonucun değer aralığı `C` bite
//! sığan işlem, itilen (check kipi, ADR-0041) işlem. Kuralın sağlamlığı
//! `volt-hir/tests/arith_cut_property_tests.rs`'te rastgele sınanır.
//!
//! Bölme ve kalan W2015'tir (ADR-0112): iki uyarı aynı işlenende çıkmaz.
//! Üretilen SV değişmez; Verilator davranış testi uyarılı biçimin bugünkü
//! kuralın değerini, `let`'li biçimin gerçek değeri verdiğini gösterir.

mod tools;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-arithcut-{tag}-{}", std::process::id()));
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

/// Bir uyarı: satır, sütun, ileti, yardım.
#[derive(Debug, Clone)]
struct Warn {
    line: u64,
    col: u64,
    message: String,
    help: String,
}

fn with_code(env: &Value, code: &str) -> Vec<Warn> {
    env["diagnostics"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .filter(|d| d["code"] == code)
        .map(|d| Warn {
            line: d["spans"][0]["start"]["line"].as_u64().unwrap_or(0),
            col: d["spans"][0]["start"]["col"].as_u64().unwrap_or(0),
            message: d["message"].as_str().unwrap_or_default().to_string(),
            help: d["help"].as_str().unwrap_or_default().to_string(),
        })
        .collect()
}

/// `src` (bir dosya) için `volt check` ve `volt build`: ikisi de çıkış 0
/// (uyarı reddetmez) ve aynı uyarılar; (W2016, W2015).
fn warnings(tag: &str, src: &str) -> (Vec<Warn>, Vec<Warn>) {
    let dir = temp_dir(tag);
    let file = dir.join("d.volt");
    std::fs::write(&file, src).expect("yaz");
    let (code, env) = run_json("check", &file, None);
    assert_eq!(code, Some(0), "{env:#}");
    let (bcode, benv) = run_json("build", &file, Some(&dir.join("build")));
    assert_eq!(bcode, Some(0), "{benv:#}");
    let w = with_code(&env, "W2016");
    let pos = |v: &[Warn]| v.iter().map(|w| (w.line, w.col)).collect::<Vec<_>>();
    assert_eq!(
        pos(&w),
        pos(&with_code(&benv, "W2016")),
        "check ve build aynı uyarıları vermeli"
    );
    let d = with_code(&env, "W2015");
    let _ = std::fs::remove_dir_all(&dir);
    (w, d)
}

/// `ports` ve tek çıkış `out o : <ty>`, `body` satırları.
fn module(ports: &str, ty: &str, body: &str) -> String {
    format!("module D {{\n{ports}\n    out o : {ty}\n{body}\n}}\n")
}

const I32S: &str = "    in  a : i32\n    in  b : i32\n    in  c : i32";
const I64S: &str = "    in  a : i64\n    in  b : i64";
const U8S: &str = "    in  u : u8\n    in  v : u8\n    in  w : u8";

/// İletinin zorunlu parçaları: kesilme ve v0.2.0'daki değişiklik (#266).
fn assert_message(w: &Warn, op: &str, width: u32, natural: u32) {
    assert!(
        w.message.contains(&format!(
            "this operand of '{op}' is cut to {width} bits before the operation (its natural width is {natural})"
        )),
        "{w:?}"
    );
    assert!(
        w.message.contains("the rule changes in v0.2.0 (#266)"),
        "{w:?}"
    );
}

// ═══ #266'nın durumları: uyarı var ═══════════════════════════════════════

/// #266'nın eki: `a * b - a == -1048576` (a, b : i32). Çarpım i32..i64,
/// `- a` 33 bitte: çarpım 64 bitten 33 bite kesilir (satır 6, sütun 9,
/// `a`). Yardım açık genişlikli `let`'i (i64) gösterir.
#[test]
fn issue_266_i32_product_minus_operand_warns() {
    let (w, d) = warnings(
        "i32",
        &module(I32S, "bool", "    o = a * b - a == -1048576"),
    );
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(w.len(), 1, "{w:?}");
    assert_eq!((w[0].line, w[0].col), (6, 9), "{w:?}");
    assert_message(&w[0], "-", 33, 64);
    assert!(w[0].help.contains("'let v : i64 = …'"), "{w:?}");
    assert!(w[0].help.contains("computed at 64 bits"), "{w:?}");
}

/// Aynı ifade, ara sonuç açık genişlikli `let`'te: itme (ADR-0041) 64
/// bitte hesaplar, hiçbir şey kesilmez.
#[test]
fn issue_266_let_of_explicit_width_does_not_warn() {
    let (w, d) = warnings(
        "i32let",
        &module(
            I32S,
            "bool",
            "    let d : i64 = a * b - a\n    o = d == -1048576",
        ),
    );
    assert!(w.is_empty() && d.is_empty(), "{w:?} {d:?}");
}

/// #266'nın ilk örneği: i64 işlenenler. Çarpım 128 bit, `- a` 65 bitte;
/// 128 bitlik `let` yok (uN/iN en çok 64): yardım kaçınma yolu olmadığını
/// ve #266'yı söyler.
#[test]
fn issue_266_i64_operands_warn_without_a_way_around() {
    let (w, _) = warnings(
        "i64",
        &module(I64S, "bool", "    o = a * b - a == -1099511627776"),
    );
    assert_eq!(w.len(), 1, "{w:?}");
    assert_message(&w[0], "-", 65, 128);
    assert!(
        w[0].help.contains("no integer type is 128 bits wide yet")
            && w[0].help.contains("issue #266"),
        "{w:?}"
    );
    assert!(!w[0].help.contains("'let v"), "{w:?}");
}

/// Kesilen işlenen itilen bir toplamın içinde: dıştaki `+` 64 bitte
/// (ADR-0041), içteki `a * b - a` yine 33 bitte.
#[test]
fn cut_inside_a_pushed_sum_warns() {
    let (w, _) = warnings("nested", &module(I32S, "i64", "    o = (a * b - a) + c"));
    assert_eq!(w.len(), 1, "{w:?}");
    assert_message(&w[0], "-", 33, 64);
}

/// u8: `u * v + w > 300`. Çarpım 16 bit, toplam 9 bitte.
#[test]
fn u8_product_plus_operand_in_a_comparison_warns() {
    let (w, _) = warnings("u8cmp", &module(U8S, "bool", "    o = u * v + w > 300"));
    assert_eq!(w.len(), 1, "{w:?}");
    assert_message(&w[0], "+", 9, 16);
    assert!(w[0].help.contains("'let v : u16 = …'"), "{w:?}");
}

/// İşlenenin değeri işlem genişliğine sığsa da sonucunki sığmıyor: `u * 2`
/// 0..510 (9 bite sığar), `+ w` 0..765 (sığmaz). Doğal genişlikte 765,
/// bugün 9 bitte 253: uyarı.
#[test]
fn operand_that_fits_but_result_that_does_not_warns() {
    let (w, _) = warnings("fitop", &module(U8S, "bool", "    o = u * 2 + w > 300"));
    assert_eq!(w.len(), 1, "{w:?}");
    assert_message(&w[0], "+", 9, 16);
}

/// Tipsiz `let`'in değeri tam okunur: uyarı `let`'te.
#[test]
fn untyped_let_warns() {
    let (w, _) = warnings(
        "let",
        &module(U8S, "bool", "    let s = u * v + w\n    o = s > 300"),
    );
    assert_eq!(w.len(), 1, "{w:?}");
    assert_eq!((w[0].line, w[0].col), (6, 13), "{w:?}");
}

// ═══ Uyarı yok ═══════════════════════════════════════════════════════════

/// Bildirilmiş genişliğe atanan sonuç: hedef sonucun alt 33 (ya da 9, 8)
/// bitini tutar; işlem genişliği bundan dar değil, kesme görünmez.
#[test]
fn result_assigned_to_a_declared_width_does_not_warn() {
    let (w, _) = warnings("i33", &module(I32S, "i33", "    o = a * b - a"));
    assert!(w.is_empty(), "{w:?}");
    for ty in ["u9", "u8"] {
        let (w, _) = warnings(&format!("{ty}a"), &module(U8S, ty, "    o = u * v + w"));
        assert!(w.is_empty(), "{ty}: {w:?}");
    }
}

/// Biriktirici deseni: `acc <= acc + a * b`, `acc : i32`. Toplam 33 bitte,
/// çarpım kesilir, ama register 32 bit tutar.
#[test]
fn accumulator_register_does_not_warn() {
    let src = "\
module Mac {
    in  clk : clock
    in  a   : i16
    in  b   : i16
    out o   : i32
    reg acc : i32 = 0
    on clk {
        acc <= acc + a * b
    }
    o = acc
}
";
    let (w, _) = warnings("mac", src);
    assert!(w.is_empty(), "{w:?}");
}

/// Sonucun değeri işlem genişliğine sığıyor: modül düzeyi `for`'un dizini
/// `y * (N + 1) + N` (y literale dönüşür, N const). Değer aralığı 4..24,
/// işlem 33 bitte; kesme değeri değiştirmez.
#[test]
fn generate_index_whose_value_fits_does_not_warn() {
    let src = "\
const N : u32 = 4

module Grid {
    in  left  : [u8; N]
    out right : [u8; N]
    wire link : [u8; N * (N + 1)]
    for y in 0..N {
        link[y * (N + 1)] = left[y]
        right[y] = link[y * (N + 1) + N]
    }
    for y in 0..N {
        for x in 0..N {
            link[y * (N + 1) + x + 1] = link[y * (N + 1) + x]
        }
    }
}
";
    let (w, _) = warnings("grid", src);
    assert!(w.is_empty(), "{w:?}");
}

/// İtilen işlem (ADR-0041): işlenenler hedefe sığıyor, hiçbir şey
/// kesilmez.
#[test]
fn pushed_operation_does_not_warn() {
    let (w, _) = warnings("u16", &module(U8S, "u16", "    o = u * v + w"));
    assert!(w.is_empty(), "{w:?}");
}

// ═══ W2015 ile çakışma yok ═══════════════════════════════════════════════

/// Bölme ikizi: `a * b / a`. Bölmenin işleneni W2015 alır, W2016 değil.
#[test]
fn division_twin_gets_w2015_only() {
    let (w, d) = warnings("div", &module(I32S, "bool", "    o = a * b / a == 1048576"));
    assert!(w.is_empty(), "{w:?}");
    assert_eq!(d.len(), 1, "{d:?}");
}

/// `(a * b - a) / b`: bölme `a * b - a`'yı 33 bitten 32 bite keser
/// (W2015) ve yalnız alt 32 bitini okur. İçteki `-`'nin 33 bitlik kesmesi
/// bu bitleri değiştirmez: W2016 yok, uyarı aynı yerde iki kez çıkmaz (iki
/// işlenen de sütun 10'da başlar).
#[test]
fn division_that_cuts_the_chain_takes_over_the_warning() {
    let (w, d) = warnings(
        "div2",
        &module(I32S, "bool", "    o = (a * b - a) / b == 3"),
    );
    assert!(w.is_empty(), "{w:?}");
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!((d[0].line, d[0].col), (6, 10), "{d:?}");
}

/// Bölme işleneni kesmiyorsa (`b as i33`: bölme 33 bitte) değerin tamamını
/// okur: W2016 kalır, W2015 yok.
#[test]
fn division_that_reads_the_whole_value_keeps_w2016() {
    let (w, d) = warnings(
        "div3",
        &module(I32S, "bool", "    o = (a * b - a) / (b as i33) == 3"),
    );
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(w.len(), 1, "{w:?}");
    assert_message(&w[0], "-", 33, 64);
}

/// Daraltan `as` (33 → 32 bit) alt bitleri okur: W2016 yok (W2010 daraltmayı
/// söyler). Genişleten `as i64` 33 bitlik değeri genişletir: W2016.
#[test]
fn narrowing_cast_takes_over_and_widening_cast_keeps_the_warning() {
    let (w, _) = warnings("cast32", &module(I32S, "i32", "    o = (a * b - a) as i32"));
    assert!(w.is_empty(), "{w:?}");
    let (w, _) = warnings(
        "cast64",
        &module(I32S, "bool", "    o = (a * b - a) as i64 == 5"),
    );
    assert_eq!(w.len(), 1, "{w:?}");
}

// ═══ tests/ui ═══════════════════════════════════════════════════════════

/// ui/fail 274: W2016 `//~^ ERROR` satırının üstündeki satırda; çıkış 0.
#[test]
fn ui_fail_274_arith_operand_cut_w2016() {
    let file = root().join("tests/ui/fail/274_arith_operand_cut.volt");
    let src = std::fs::read_to_string(&file).expect("fixture");
    let marker = src
        .lines()
        .position(|l| l.trim_start().starts_with("//~^ ERROR"))
        .expect("//~^ ERROR satırı") as u64; // üst satır (1 tabanlı)
    let (code, env) = run_json("check", &file, None);
    assert_eq!(code, Some(0), "{env:#}");
    let w = with_code(&env, "W2016");
    assert_eq!(
        w.iter().map(|d| d.line).collect::<Vec<_>>(),
        [marker],
        "{env:#}"
    );
    assert_message(&w[0], "-", 33, 64);
}

/// ui/pass 153: aynı hesaplar açık genişlikle ve atanan sonuçlarla; W2016
/// yok.
#[test]
fn ui_pass_153_explicit_width_has_no_w2016() {
    let file = root().join("tests/ui/pass/153_arith_operand_explicit_width.volt");
    let (code, env) = run_json("check", &file, None);
    assert_eq!(code, Some(0), "{env:#}");
    assert!(with_code(&env, "W2016").is_empty(), "{env:#}");
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

/// Gürültü ölçümü (ADR-0116): depodaki hiçbir `.volt` dosyası W2016 almaz
/// (uyarıyı bilerek taşıyan ui/fail 274 dışında). Kural daraltılmadan
/// önce 10 uyarı vardı, hepsi zararsızdı (ADR-0116, "Gürültü").
#[test]
fn no_volt_file_in_the_repository_gets_w2016() {
    let mut files = Vec::new();
    for d in ["examples", "tests", "templates", "book", "demo", "crates"] {
        volt_files(&root().join(d), &mut files);
    }
    files.sort();
    assert!(files.len() > 500, "{} dosya", files.len());
    let mut hits = Vec::new();
    let mut expected = 0;
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap_or_default();
        if text.starts_with("//~ W2016") {
            expected += 1;
            continue;
        }
        let (_, env) = run_json("check", f, None);
        for w in with_code(&env, "W2016") {
            hits.push(format!("{}:{}:{}", f.display(), w.line, w.col));
        }
    }
    eprintln!(
        "arith_cut_warning_tests: {} .volt dosyası denetlendi, W2016: {} (W2016 bekleyen fikstür: {expected})",
        files.len() - expected,
        hits.len()
    );
    assert_eq!(expected, 1, "W2016 bekleyen fikstür sayısı");
    assert!(hits.is_empty(), "{}", hits.join("\n"));
}

// ═══ Verilator: uyarılı biçim bugünkü kuralın değerini verir ═══════════

/// Beklenen değerler elle, şartnameden (§3.3, ADR-0025) hesaplandı:
///
/// - `CutI32`, a = b = 2^20: a · b = 2^40 = 128 · 2^33, 33 bitte 0;
///   `0 - 2^20 = -2^20`, yani `cut` doğru. Gerçek değer 2^40 - 2^20 =
///   1099510579200 ≠ -2^20: `let d : i64` ile `kept` yanlış, `diff` o
///   değer.
/// - `CutU8`, u = v = 32, w = 0: u · v = 1024 = 2 · 2^9, 9 bitte 0; `cut`
///   (0 > 300) yanlış. Gerçek değer 1024 > 300: `let s : u16` ile `kept`
///   doğru.
/// - `CutI64`, a = b = 2^40: a · b = 2^80, 65 bitte 0 (80 ≥ 65); `0 - 2^40`
///   `-2^40`'a eşit: `cut` doğru, gerçekte yanlış. Kaçınma yolu yok
///   (#266).
///
/// Bu değerler kural v0.2.0'da değişince (#266) değişir; test o zaman
/// yeni kurala göre yazılır.
const SIM: &str = "\
module CutI32 {
    in  a    : i32
    in  b    : i32
    out cut  : bool
    out kept : bool
    out diff : i64
    cut = a * b - a == -1048576
    let d : i64 = a * b - a
    kept = d == -1048576
    diff = d
}

module CutU8 {
    in  u    : u8
    in  v    : u8
    in  w    : u8
    out cut  : bool
    out kept : bool
    cut = u * v + w > 300
    let s : u16 = u * v + w
    kept = s > 300
}

module CutI64 {
    in  a   : i64
    in  b   : i64
    out cut : bool
    cut = a * b - a == -1099511627776
}

test \"i32: the cut product gives -2^20, the let gives 2^40 - 2^20\" {
    let dut = CutI32 { };
    dut.a = 1048576;
    dut.b = 1048576;
    step(1);
    assert_eq(dut.cut, true);
    assert_eq(dut.kept, false);
    assert_eq(dut.diff, 1099510579200);
}

test \"u8: the cut product gives 0, the let gives 1024\" {
    let dut = CutU8 { };
    dut.u = 32;
    dut.v = 32;
    dut.w = 0;
    step(1);
    assert_eq(dut.cut, false);
    assert_eq(dut.kept, true);
}

test \"i64: the cut product gives -2^40\" {
    let dut = CutI64 { };
    dut.a = 1099511627776;
    dut.b = 1099511627776;
    step(1);
    assert_eq(dut.cut, true);
}
";

/// Uyarılı biçimler (`cut`, satır 7, 19 ve 28) W2016 alır ve
/// simülasyonda bugünkü kuralın (kesilmiş) değerini verir; `let`'li
/// biçimler uyarı almaz ve gerçek değeri verir.
#[test]
fn cut_forms_give_the_rule_value_and_let_forms_the_true_value_in_simulation() {
    if tools::require(tools::Tool::Verilator).is_none() {
        return;
    }
    let dir = temp_dir("sim");
    std::fs::write(dir.join("cut_test.volt"), SIM).expect("yaz");
    let output = Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("VOLT_TOOL_BACKEND", "local")
        .env("VOLT_LANG", "en")
        .env("NO_COLOR", "1")
        .current_dir(&dir)
        .args(["test", "cut_test.volt", "--target-dir", "build"])
        .output()
        .expect("volt test");
    let out = format!(
        "{}\n--- stderr\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(output.status.code(), Some(0), "{out}");
    assert!(out.contains("test result: ok. 3 passed; 0 failed"), "{out}");
    let lines: Vec<&str> = out
        .lines()
        .filter_map(|l| l.trim().strip_prefix("┌─ cut_test.volt:"))
        .collect();
    let warned = out.matches("warning[W2016]").count();
    assert_eq!(warned, 3, "{out}");
    for at in ["7:11", "19:11", "28:11"] {
        assert!(
            lines.contains(&at),
            "W2016 {at} bekleniyor: {lines:?}\n{out}"
        );
    }
}
