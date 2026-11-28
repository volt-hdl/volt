//! Generic `const S : Trit` parametresi Trit sinyaliyle aynı değeri verir
//! (#468, #471). Bakımcının kararı (#468): S'nin geçtiği her ifade, aynı
//! yerde bir Trit sinyali olsaydı alacağı tipi ve değeri alır. Bu test aynı
//! modülde her biçimi iki kez yazar, bir kez S ile, bir kez S'ye bağlanmış
//! Trit portu `t` ile; S = +1, 0, -1 örneklerinin (`F<1>`, `F<0>`, `F<-1>`,
//! #471 ile yazılabilir) her çıkış çifti Verilator'da aynı olmalıdır. Girdiler
//! A15 köşesini içerir (x = i8::MIN, z = i2::MIN; ADR-0145 Karar 8: Trit
//! çarpımı bir işlemin işleneniyken iN'de sarar). Önce (main) S'li çarpım
//! tamsayı çarpımı kuralıyla tipleniyordu: köşede `(-S * x) as i12` +128,
//! `(-t * x) as i12` -128 veriyordu; `S == 1` ve `match S` E2005 alıyordu;
//! `F<-1>` E0010 alıyordu. Elle hesaplanmış değerler
//! `tests/real_bugs/468_generic_trit_const.volt`'ta.
//!
//! Verilator yoksa test atlanır; `VOLT_REQUIRE_TOOLS=verilator` ile (CI'ın
//! araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::PathBuf;
use std::process::Command;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-gentrit-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// Çalıştırılan `volt` komutunun çıktısı: (başarılı mı, stdout + stderr).
fn volt(tag: &str, src: &str, args: &[&str]) -> (bool, String) {
    let dir = temp_dir(tag);
    std::fs::write(dir.join("generic_trit.volt"), src).expect("yaz");
    let output = Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("VOLT_TOOL_BACKEND", "local")
        .env("VOLT_LANG", "en")
        .env("NO_COLOR", "1")
        .current_dir(&dir)
        .args(args)
        .output()
        .expect("volt");
    let _ = std::fs::remove_dir_all(&dir);
    let text = format!(
        "{}\n--- stderr\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output.status.success(), text)
}

/// (çıkış tipi, S'li biçim, t'li biçim). `q` ile başlayan biçim tipsiz bir
/// `let`'tir: `let q<k>s = <biçim>` ve çıkış ona bağlanır.
const FORMS: &[(&str, &str, &str)] = &[
    // A15: `as`'ın işleneni olan çarpım iN'de sarar.
    ("i12", "(S * x) as i12", "(t * x) as i12"),
    ("i12", "(-S * x) as i12", "(-t * x) as i12"),
    ("i12", "(x * -S) as i12", "(x * -t) as i12"),
    ("i12", "(S * z) as i12", "(t * z) as i12"),
    ("i12", "(-S * z) as i12", "(-t * z) as i12"),
    // `+` ve karşılaştırmanın işleneni.
    ("i9", "S * x + x", "t * x + x"),
    ("bool", "(-S * x) < 0", "(-t * x) < 0"),
    ("bool", "S * x == x", "t * x == x"),
    // En üstte geniş hedef: çarpım hedefe itilir.
    ("i12", "-S * x", "-t * x"),
    // §3.4: `-z` i9.
    ("i9", "-(S * x)", "-(t * x)"),
    // Tipsiz `let`: çarpım i8'dir.
    ("i16", "q S * x", "q t * x"),
    ("i16", "q -S * x", "q -t * x"),
    // Trit değerli biçimler.
    ("Trit", "S", "t"),
    ("Trit", "-S", "-t"),
    ("Trit", "S * S", "t * t"),
    ("Trit", "if c { S } else { -S }", "if c { t } else { -t }"),
    (
        "Trit",
        "q if c { S } else { -S }",
        "q if c { t } else { -t }",
    ),
    // §3.3: Trit ± Trit i3; §3.6: Trit → i3.
    ("i3", "S + t", "t + t"),
    ("i3", "S as i3", "t as i3"),
    // S'yle katlanan literal ağacı Trit'tir (`S * 1`): seçicide Trit
    // genişliğinde yazılır.
    ("i8", "(S * 1) * x", "(t * 1) * x"),
    ("i8", "-(S * 1) * x", "-(t * 1) * x"),
    ("i8", "(S * S) * x", "(t * t) * x"),
    ("i8", "x * (S * -1)", "x * (t * -1)"),
    ("bool", "S * 1 == 1", "t * 1 == 1"),
    ("i5", "S as i5", "t as i5"),
    // fn açılımı (ADR-0081): parametrenin yerine geçen kopya S'nin tablodaki
    // değerini okur.
    ("i8", "scale(S, x)", "scale(t, x)"),
    ("i12", "scale(-S, x) as i12", "scale(-t, x) as i12"),
    // Karşılaştırma ve match: önce E2005.
    ("bool", "S == 1", "t == 1"),
    (
        "u4",
        "match S { 1 => 4, 0 => 5, _ => 6 }",
        "match t { 1 => 4, 0 => 5, _ => 6 }",
    ),
];

fn design() -> String {
    let mut ports = String::new();
    let mut body = String::new();
    for (k, (ty, s, t)) in FORMS.iter().enumerate() {
        ports.push_str(&format!("    out s{k} : {ty}\n    out t{k} : {ty}\n"));
        for (name, form) in [(format!("s{k}"), s), (format!("t{k}"), t)] {
            match form.strip_prefix("q ") {
                Some(value) => {
                    body.push_str(&format!(
                        "    let q{name} = {value}\n    {name} = q{name}\n"
                    ));
                }
                None => body.push_str(&format!("    {name} = {form}\n")),
            }
        }
    }
    // Register: S ve t aynı değeri saklar.
    ports.push_str("    out rs : Trit\n    out rt : Trit\n");
    body.push_str(
        "    reg r1 : Trit = 0\n    reg r2 : Trit = 0\n    on clk {\n        r1 <= S\n        r2 <= t\n    }\n    rs = r1\n    rt = r2\n",
    );
    let mut top_ports = String::new();
    let mut top_body = String::new();
    for (inst, arg) in [("p", "1"), ("z", "0"), ("n", "-1")] {
        top_body.push_str(&format!(
            "    let w{inst} : Trit = {arg}\n    let {inst} = F<{arg}> {{ clk: clk, x: x, z: zz, t: w{inst}, c: c }}\n"
        ));
        for (k, (ty, _, _)) in FORMS.iter().enumerate() {
            for side in ["s", "t"] {
                top_ports.push_str(&format!("    out {inst}{side}{k} : {ty}\n"));
                top_body.push_str(&format!("    {inst}{side}{k} = {inst}.{side}{k}\n"));
            }
        }
        for side in ["s", "t"] {
            top_ports.push_str(&format!("    out {inst}r{side} : Trit\n"));
            top_body.push_str(&format!("    {inst}r{side} = {inst}.r{side}\n"));
        }
    }
    let mut tests = String::new();
    // Bit desenleri: x = -128 → 128, 5, 127; z = -2 → 2, 1.
    for (v, (x, z, c)) in [
        (128, 2, "true"),
        (5, 1, "false"),
        (127, 2, "false"),
        (128, 1, "true"),
    ]
    .iter()
    .enumerate()
    {
        tests.push_str(&format!(
            "test \"S and t agree, vector {v}\" {{\n    let dut = Top {{ }};\n    dut.x = {x};\n    dut.zz = {z};\n    dut.c = {c};\n    step(2);\n"
        ));
        for inst in ["p", "z", "n"] {
            for k in 0..FORMS.len() {
                tests.push_str(&format!("    assert_eq(dut.{inst}s{k}, dut.{inst}t{k});\n"));
            }
            tests.push_str(&format!("    assert_eq(dut.{inst}rs, dut.{inst}rt);\n"));
        }
        tests.push_str("}\n\n");
    }
    format!(
        "fn scale(a : Trit, v : i8) -> i8 {{
    a * v
}}

module F<const S : Trit> {{
    in  clk : clock
    in  x   : i8
    in  z   : i2
    in  t   : Trit
    in  c   : bool
{ports}{body}}}

module Top {{
    in  clk : clock
    in  x   : i8
    in  zz  : i2
    in  c   : bool
{top_ports}{top_body}}}

{tests}"
    )
}

/// Araç gerektirmez: tasarım `volt check`'ten tanısız geçer. Main'de `S ==
/// 1` ve `match S` E2005, `F<-1>` E0010 alıyordu.
#[test]
fn the_design_checks_without_a_diagnostic() {
    let (ok, out) = volt("check", &design(), &["check", "generic_trit.volt"]);
    assert!(ok, "{out}");
    assert!(out.contains("Result 0 error(s), 0 warning(s)"), "{out}");
}

#[test]
fn every_form_with_s_equals_the_form_with_a_trit_signal_in_verilator() {
    if tools::require(tools::Tool::Verilator).is_none() {
        return;
    }
    let (_, out) = volt(
        "sim",
        &design(),
        &["test", "generic_trit.volt", "--target-dir", "build"],
    );
    assert!(out.contains("test result: ok. 4 passed; 0 failed"), "{out}");
}
