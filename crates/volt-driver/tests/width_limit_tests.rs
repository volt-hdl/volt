//! Tip genişliğinin üst sınırı (#418): `bits<N>`, `uint<N>`, `sint<N>`'de
//! N en çok 65535'tir ve sınır tek bir sabittir
//! (`volt_ast::const_eval::MAX_WIDTH`). Bakımcının #418'deki kararı: tek
//! sınır 65535; `bits<65536>` E2025 ile reddedilir (const-eval.md, E2025;
//! sessiz yanlış düzeltmesi, ADR-0100).
//!
//! Önce E2025 65536'yı kabul ediyor, tip denetimi ise 65535'in üstünü
//! sessizce hata tipine çeviriyordu: `bits<65536>`'yı `bits<8>`'e atamak
//! E2001'siz derleniyor, SV `8'(x)` ile kesiyordu. Beklenen değerler
//! elle, bu karardan yazılmıştır; sınır burada sabitten okunmaz, 65535 ve
//! 65536 sayı olarak yazılır.

use std::path::PathBuf;
use std::process::Command;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-width-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("geçici dizin");
    dir
}

/// İnsan biçimindeki rapordaki hata ve uyarı kodları, sıralı.
fn codes(report: &str) -> Vec<String> {
    let mut out = Vec::new();
    for head in ["error[", "warning["] {
        let mut rest = report;
        while let Some(i) = rest.find(head) {
            let code = &rest[i + head.len()..];
            if let Some(end) = code.find(']') {
                out.push(code[..end].to_string());
            }
            rest = &rest[i + head.len()..];
        }
    }
    out.sort();
    out
}

struct Run {
    exit: Option<i32>,
    codes: Vec<String>,
    report: String,
    /// `out/` altındaki dosyaların adları.
    files: Vec<String>,
}

/// `volt <args> t.volt` kaynağı geçici bir dizinde çalıştırır.
fn volt(tag: &str, args: &[&str], src: &str) -> Run {
    let dir = temp_dir(tag);
    std::fs::write(dir.join("t.volt"), src).expect("kaynak yazılmalı");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .current_dir(&dir)
        .args(args)
        .arg("t.volt")
        .args(["--color=never"])
        .env("VOLT_LANG", "en")
        .env("VOLT_TOOL_BACKEND", "local")
        .output()
        .expect("volt çalışmalı");
    let report =
        String::from_utf8_lossy(&out.stderr).into_owned() + &String::from_utf8_lossy(&out.stdout);
    let mut files = Vec::new();
    collect(&dir.join("out"), "", &mut files);
    files.sort();
    let _ = std::fs::remove_dir_all(&dir);
    Run {
        exit: out.status.code(),
        codes: codes(&report),
        report,
        files,
    }
}

/// `dir` altındaki dosyalar, `/` ayraçlı göreli adlarıyla.
fn collect(dir: &std::path::Path, prefix: &str, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let name = format!("{prefix}{}", e.file_name().to_string_lossy());
        if e.path().is_dir() {
            collect(&e.path(), &format!("{name}/"), out);
        } else {
            out.push(name);
        }
    }
}

fn check(tag: &str, src: &str) -> Run {
    volt(tag, &["check"], src)
}

fn build(tag: &str, src: &str) -> Run {
    volt(tag, &["build", "--single-file", "--target-dir", "out"], src)
}

/// Çıkış kodu ve tanı kodları tam olarak bunlar.
fn assert_codes(run: &Run, exit: i32, want: &[&str]) {
    let mut want: Vec<String> = want.iter().map(|c| c.to_string()).collect();
    want.sort();
    assert_eq!(
        (run.exit, &run.codes),
        (Some(exit), &want),
        "{}",
        run.report
    );
}

/// E2025'in yardım satırı sınırı söyler (const-eval.md, E2025).
fn assert_names_the_limit(run: &Run) {
    assert!(
        run.report.contains("width must be in the range 1..=65535"),
        "E2025 sınırı 65535 olarak söylemeli:\n{}",
        run.report
    );
}

#[test]
fn the_widest_bits_uint_and_sint_are_accepted() {
    let src = "module W {\n    in  x : bits<65535>\n    in  u : uint<65535>\n    \
               in  s : sint<65535>\n    out y : bits<65535>\n    out v : uint<65535>\n    \
               out t : sint<65535>\n    y = x\n    v = u\n    t = s\n}\n";
    assert_codes(&check("max", src), 0, &[]);
    let run = build("max-build", src);
    assert_codes(&run, 0, &[]);
    assert!(
        run.files.iter().any(|f| f.ends_with(".sv")),
        "SV yazılmalı: {:?}\n{}",
        run.files,
        run.report
    );
}

#[test]
fn a_width_of_65536_is_e2025_in_every_family() {
    for (i, ty) in ["bits<65536>", "uint<65536>", "sint<65536>"]
        .iter()
        .enumerate()
    {
        // İki genişlik ifadesi (x'inki ve y'ninki): ikisi de raporlanır.
        let src = format!("module W {{\n    in  x : {ty}\n    out y : {ty}\n    y = x\n}}\n");
        let run = check(&format!("over-{i}"), &src);
        assert_codes(&run, 1, &["E2025", "E2025"]);
        assert_names_the_limit(&run);
    }
}

#[test]
fn a_constant_expression_equal_to_65536_is_e2025() {
    // 256 * 256 = 65536 ve 65535 + 1 = 65536: değer sınırın bir üstü.
    for (i, width) in ["N", "65535 + 1"].iter().enumerate() {
        let src = format!(
            "const N : u32 = 256 * 256\nmodule W {{\n    in  x : bits<{width}>\n    \
             out y : bits<8>\n    y = x[7:0]\n}}\n"
        );
        let run = check(&format!("const-{i}"), &src);
        assert_codes(&run, 1, &["E2025"]);
        assert_names_the_limit(&run);
    }
}

#[test]
fn a_generic_parameter_of_65536_is_e2025_and_65535_compiles() {
    let src = |n: u32| {
        format!(
            "module G<const N: u32> {{\n    in  x : uint<N>\n    out y : u8\n    \
             y = x[7:0] as u8\n}}\nmodule Top {{\n    in  a : u8\n    out b : u8\n    \
             let g = G<{n}> {{ x: a }}\n    b = g.y\n}}\n"
        )
    };
    let run = check("generic-over", &src(65536));
    assert_codes(&run, 1, &["E2025"]);
    assert_names_the_limit(&run);
    assert_codes(&check("generic-max", &src(65535)), 0, &[]);
}

/// #418'in örneği: `bits<65536>`'yı `bits<8>`'e atamak. Önce 0 hatayla
/// derleniyor, SV `assign y = 8'(x);` ile 65528 biti atıyordu; şimdi
/// E2025 ile durur ve SV yazılmaz.
#[test]
fn issue_418_example_stops_with_e2025_and_writes_no_sv() {
    let src = "module W {\n    in  x : bits<65536>\n    out y : bits<8>\n    y = x\n}\n";
    let run = check("418-check", src);
    assert_codes(&run, 1, &["E2025"]);
    assert_names_the_limit(&run);
    let run = build("418-build", src);
    assert_codes(&run, 1, &["E2025"]);
    assert!(run.files.is_empty(), "SV yazılmamalı: {:?}", run.files);
}

/// Tip denetiminin çözdüğü her konum aynı sınırı okur: takma ad hedefi,
/// blok içi `let` ve fonksiyon parametresi de E2025 alır (önce üçü de
/// sessizce hata tipine dönüyordu).
#[test]
fn every_type_position_rejects_65536_with_e2025() {
    let cases = [
        (
            "alias",
            "type Wide = bits<65536>\nmodule W {\n    in  x : Wide\n    out y : bits<8>\n    \
             y = x\n}\n",
        ),
        (
            "comb-let",
            "module W {\n    in  x : bits<8>\n    out y : bits<8>\n    comb {\n        \
             let t : bits<65536> = x as bits<65536>\n        y = t\n    }\n}\n",
        ),
        (
            "fn-param",
            "fn f(a : bits<65536>) -> bits<8> {\n    a\n}\nmodule W {\n    in  x : bits<8>\n    \
             out y : bits<8>\n    y = f(x as bits<65536>)\n}\n",
        ),
    ];
    for (tag, src) in cases {
        let run = check(tag, src);
        assert!(
            run.codes.iter().any(|c| c == "E2025") && run.exit == Some(1),
            "{tag}: E2025 beklenir:\n{}",
            run.report
        );
        assert_names_the_limit(&run);
    }
}

/// Aynı genişlik ifadesi bir kez raporlanır: modül düzeyi port hem sabit
/// değerlendiricinin konum denetiminden hem tip denetiminden geçer; tanı
/// ikinci kez üretilmez (katlanıp "özdeş kopya" notu da almaz).
#[test]
fn one_width_expression_is_reported_once() {
    let src = "type Wide = bits<65536>\nmodule W {\n    in  x : Wide\n    in  z : Wide\n    \
               in  q : bits<65536>\n    out y : bits<8>\n    y = 0\n}\n";
    let run = check("once", src);
    let e2025 = run.codes.iter().filter(|c| *c == "E2025").count();
    // İki ifade: takma adın `65536`'sı ve `q`'nunki.
    assert_eq!(e2025, 2, "{}", run.report);
    // Her ifade kaynakta bir kez yazılmıştır: özdeş tanı katlamasının
    // (ADR-0068) "N özdeş kopya" notu yanlış olurdu; tanı ikinci kez hiç
    // üretilmez.
    let port = check(
        "once-port",
        "module W {\n    in  q : bits<65536>\n    out y : bits<8>\n    y = q[7:0]\n}\n",
    );
    assert_codes(&port, 1, &["E2025"]);
    for run in [&run, &port] {
        assert!(
            !run.report.contains("identical occurrences"),
            "tanı bir kez üretilmeli:\n{}",
            run.report
        );
    }
}

/// Genişlik sonekli tam sayı adları (`u9`, `i33`) tip denetiminde
/// genişliklerini korur: sınırın içindeki her ad bir tamsayı tipidir.
/// `u9`'u `u8`'e (`i33`'ü `i8`'e) atamak 9 biti 8 bite sığdırmaz, E2001'dir
/// (type-inference.md, örtük daraltma yok); hata tipine dönüşüp sessiz
/// kalmaz. `u65535` de sınırın içindedir.
#[test]
fn widened_int_names_keep_their_width_in_type_checking() {
    for (name, bits, target) in [("u9", 9, "u8"), ("i33", 33, "i8"), ("u65535", 65535, "u8")] {
        let src = format!("module M {{\n    in  x : {name}\n    out y : {target}\n    y = x\n}}\n");
        let run = check(name, &src);
        assert_codes(&run, 1, &["E2001"]);
        assert!(
            run.report.contains(&format!(
                "a {bits}-bit value does not fit in an 8-bit target"
            )),
            "{name}: genişlik {bits} olmalı:\n{}",
            run.report
        );
    }
}

/// Sınırı aşan genişlik sonekli ad (`u65537`, `u70000`) başka bir
/// genişlikteki tamsayı sayılmaz: 65537 ya da 70000 bit `u16`'ya kesilip
/// 1 ya da 4464 bitlik bir tip olmaz. Tip denetimi ona bir genişlik
/// yüklemez, dolayısıyla genişliğe dayalı E2001 ve E2010 çıkmaz. Birim
/// yine de bir hatayla durur.
#[test]
fn a_widened_int_name_above_65535_gets_no_width() {
    let cases = [
        ("u65537", "out y : bool\n    y = x == 3"),
        ("u70000", "out y : u8\n    y = x"),
        ("i70000", "out y : i8\n    y = x"),
    ];
    for (name, rest) in cases {
        let src = format!("module M {{\n    in  x : {name}\n    {rest}\n}}\n");
        let run = check(name, &src);
        assert_eq!(run.exit, Some(1), "{name}: hata beklenir:\n{}", run.report);
        assert!(
            !run.codes.iter().any(|c| c == "E2001" || c == "E2010"),
            "{name}: tip denetimi sınırı aşan ada genişlik yüklememeli:\n{}",
            run.report
        );
    }
}

/// Struct bit düzeni (ADR-0077) aralık dışı genişlikte kurulmaz: alanın
/// ve dizi elemanının genişliği de `1..=65535` aralığındadır; dışındaki
/// genişlik düzeni `UnknownWidth` ile durdurur (tanısı E2025'te).
fn struct_width(src: &str) -> Result<u32, volt_ast::struct_layout::LayoutError> {
    let parsed = volt_syntax::parse(volt_syntax::FileId(0), src);
    assert!(parsed.diagnostics.is_empty(), "kaynak ayrışmalı: {src}");
    let ast = &parsed.ast;
    let decl = volt_ast::struct_layout::struct_named(ast, "S").expect("S tanımlı");
    let mut eval = |e| volt_ast::const_eval::const_int(ast, e);
    volt_ast::struct_layout::layout(ast, decl, &mut eval).map(|l| l.width)
}

#[test]
fn a_struct_layout_takes_field_widths_up_to_65535() {
    use volt_ast::struct_layout::LayoutError::UnknownWidth;
    let field = |ty: &str| struct_width(&format!("struct S {{\n    a : {ty}\n    b : bool\n}}\n"));
    assert_eq!(field("bits<65535>"), Ok(65536));
    assert_eq!(field("bits<65536>"), Err(UnknownWidth));
    assert_eq!(field("bits<0>"), Err(UnknownWidth));
    assert_eq!(field("[bits<65535>; 1]"), Ok(65536));
    assert_eq!(field("[bits<65536>; 1]"), Err(UnknownWidth));
    assert_eq!(field("[bits<0>; 2]"), Err(UnknownWidth));
}

/// Enum taban tipinin genişliği de `1..=65535` aralığındadır: dışındaki
/// genişlik bir kodlama genişliği olarak kabul edilmez (tanısı E2025'te).
#[test]
fn an_enum_base_width_is_taken_up_to_65535() {
    use volt_ast::enum_layout::{enum_named, repr_of, Repr};
    let repr = |ty: &str| {
        let src = format!("enum E : {ty} {{\n    A\n    B\n}}\n");
        let parsed = volt_syntax::parse(volt_syntax::FileId(0), &src);
        assert!(parsed.diagnostics.is_empty(), "kaynak ayrışmalı: {src}");
        let ast = &parsed.ast;
        let decl = enum_named(ast, "E").expect("E tanımlı");
        repr_of(ast, decl, &mut |e| volt_ast::const_eval::const_int(ast, e))
    };
    assert_eq!(repr("bits<65535>"), Repr::Width(65535));
    assert_eq!(repr("uint<65535>"), Repr::Width(65535));
    for ty in ["bits<65536>", "uint<65536>", "bits<0>"] {
        assert!(
            !matches!(repr(ty), Repr::Width(_)),
            "{ty}: genişlik olarak kabul edilmemeli, {:?}",
            repr(ty)
        );
    }
}
