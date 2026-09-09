//! Test bloklarının anlamsal denetimi (ADR-0033, E8501-E8506).
//!
//! Test gövdesi donanım değil betiktir; isim çözümleme/tip kontrolü
//! boru hattına girmez. Bu modül portların varlığını/yönünü ve yerleşik
//! çağrıların imzalarını denetler. Modüller birden çok kaynaktan
//! gelebilir (kardeş dosya kuralı: `X_test.volt` ↔ `X.volt`).

use std::collections::HashMap;

use volt_ast::{
    ItemKind, ModuleDecl, Name, PortDir, SourceFile, TestExpr, TestExprKind, TestStmt, TypeRefKind,
};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan};

/// Test gövdesinin tanıdığı yerleşikler: (ad, argüman sayısı).
pub const TEST_BUILTINS: &[(&str, usize)] = &[
    ("step", 1),
    ("reset", 0),
    ("assert_eq", 2),
    ("assert_ne", 2),
    ("assert_true", 1),
    ("assert_false", 1),
];

/// `sources` içindeki tüm modülleri ad → (kaynak, modül) olarak toplar.
pub fn collect_modules<'a>(
    sources: &[&'a SourceFile],
) -> HashMap<String, (&'a SourceFile, &'a ModuleDecl)> {
    let mut map = HashMap::new();
    for src in sources {
        for idx in &src.items {
            if let ItemKind::Module(m) = &src.items_arena[*idx].kind {
                map.entry(m.name.text.clone()).or_insert((*src, m));
            }
        }
    }
    map
}

/// `tests_from` dosyasındaki test bloklarını denetler.
///
/// `assume_external_modules`: dosyada hiç modül yoksa (testler kardeş
/// dosyanın modüllerini kullanıyordur) modül-varlık denetimi atlanır —
/// tek dosyalık analizde (`analyze`, LSP) sahte E8501 üretmemek için.
/// Sürücü, kardeş dosyayı yükleyip `sources` ile geçirdiğinde bunu
/// `false` verir ve tam denetim yapılır.
pub fn check_tests(
    sources: &[&SourceFile],
    tests_from: &SourceFile,
    assume_external_modules: bool,
) -> Vec<Diagnostic> {
    let modules = collect_modules(sources);
    let mut diags = Vec::new();

    for idx in &tests_from.items {
        let ItemKind::Test(test) = &tests_from.items_arena[*idx].kind else {
            continue;
        };
        // dut adı → Some(modül) | None (varlığı varsayılan dış modül).
        let mut duts: HashMap<&str, Option<(&SourceFile, &ModuleDecl)>> = HashMap::new();

        // Bir test tek DUT sürer (ADR-0033): Verilator modeli test
        // yürütülebiliri başına tek üst modülle kurulur.
        if !test
            .stmts
            .iter()
            .any(|s| matches!(s, TestStmt::LetDut { .. }))
        {
            diags.push(missing_dut(test));
        }

        for stmt in &test.stmts {
            match stmt {
                TestStmt::LetDut { name, module, .. } => {
                    if !duts.is_empty() {
                        diags.push(duplicate_dut(name));
                        continue;
                    }
                    match modules.get(&module.text) {
                        Some(found) => {
                            duts.insert(&name.text, Some(*found));
                        }
                        None if assume_external_modules => {
                            duts.insert(&name.text, None);
                        }
                        None => diags.push(unknown_module(module)),
                    }
                }
                TestStmt::SetPort {
                    dut, port, value, ..
                } => {
                    check_set_port(&duts, dut, port, &mut diags);
                    check_expr(&duts, value, &mut diags);
                }
                TestStmt::Call { span, func, args } => {
                    let Some((_, arity)) =
                        TEST_BUILTINS.iter().find(|(name, _)| *name == func.text)
                    else {
                        diags.push(unknown_builtin(func));
                        continue;
                    };
                    if args.len() != *arity {
                        diags.push(bad_arity(func, *arity, args.len(), *span));
                        continue;
                    }
                    if func.text == "step" {
                        match &args[0].kind {
                            TestExprKind::Int(n) if *n >= 1 => {}
                            _ => diags.push(bad_step_arg(&args[0])),
                        }
                        continue;
                    }
                    for arg in args {
                        check_expr(&duts, arg, &mut diags);
                    }
                }
            }
        }
    }
    diags
}

/// `dut.port = v` sol tarafı: dut tanımlı, port var, yönü `in`,
/// tipi `clock` değil.
fn check_set_port(
    duts: &HashMap<&str, Option<(&SourceFile, &ModuleDecl)>>,
    dut: &Name,
    port: &Name,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(entry) = duts.get(dut.text.as_str()) else {
        diags.push(undefined_dut(dut));
        return;
    };
    let Some((src, module)) = entry else {
        return; // dış modül varsayımı: port denetimi yapılamaz
    };
    let Some(p) = module.ports.iter().find(|p| p.name.text == port.text) else {
        diags.push(unknown_port(port, &module.name.text));
        return;
    };
    let is_clock = matches!(src.types[p.ty].kind, TypeRefKind::Clock);
    if p.direction != PortDir::In || is_clock {
        diags.push(Diagnostic::error(
            ErrorCode::E8503,
            lstr!(en: "cannot drive port '{}' from a test", port.text;
                  tr: "'{}' portu testten sürülemez", port.text),
            LabeledSpan::primary(
                port.span,
                if is_clock {
                    lstr!(en: "this is a clock port"; tr: "bu bir saat portu")
                } else {
                    lstr!(en: "this port is not an input"; tr: "bu port giriş değil")
                },
            ),
            lstr!(en: "only 'in' ports can be written; step() drives the clock itself";
                  tr: "yalnız 'in' portlarına yazılabilir; saati step() kendisi sürer"),
        ));
    }
}

/// Test ifadesi: `dut.port` okumaları `out` port olmalı.
fn check_expr(
    duts: &HashMap<&str, Option<(&SourceFile, &ModuleDecl)>>,
    expr: &TestExpr,
    diags: &mut Vec<Diagnostic>,
) {
    let TestExprKind::PortRead { dut, port } = &expr.kind else {
        return;
    };
    let Some(entry) = duts.get(dut.text.as_str()) else {
        diags.push(undefined_dut(dut));
        return;
    };
    let Some((_, module)) = entry else {
        return; // dış modül varsayımı
    };
    let Some(p) = module.ports.iter().find(|p| p.name.text == port.text) else {
        diags.push(unknown_port(port, &module.name.text));
        return;
    };
    if p.direction != PortDir::Out {
        diags.push(Diagnostic::error(
            ErrorCode::E8504,
            lstr!(en: "cannot read port '{}' in a test", port.text;
                  tr: "'{}' portu testte okunamaz", port.text),
            LabeledSpan::primary(
                port.span,
                lstr!(en: "this port is not an output"; tr: "bu port çıkış değil"),
            ),
            lstr!(en: "assertions observe the design through its 'out' ports";
                  tr: "assert'ler tasarımı 'out' portları üzerinden gözler"),
        ));
    }
}

fn unknown_module(module: &Name) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E8501,
        lstr!(en: "unknown module '{}' in test", module.text;
              tr: "testte bilinmeyen modül: '{}'", module.text),
        LabeledSpan::primary(
            module.span,
            lstr!(en: "no module with this name"; tr: "bu adla modül yok"),
        ),
        lstr!(en: "define the module in this file, or put the tests in '<module>_test.volt' next to '<module>.volt'";
              tr: "modülü bu dosyada tanımlayın ya da testleri '<modül>.volt' yanındaki '<modül>_test.volt' dosyasına koyun"),
    )
}

fn unknown_port(port: &Name, module: &str) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E8502,
        lstr!(en: "module '{module}' has no port '{}'", port.text;
              tr: "'{module}' modülünde '{}' diye port yok", port.text),
        LabeledSpan::primary(port.span, lstr!(en: "unknown port"; tr: "bilinmeyen port")),
        lstr!(en: "check the module's port list for the exact name";
              tr: "tam ad için modülün port listesine bakın"),
    )
}

fn undefined_dut(dut: &Name) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E8506,
        lstr!(en: "'{}' is not defined in this test", dut.text;
              tr: "'{}' bu testte tanımlı değil", dut.text),
        LabeledSpan::primary(
            dut.span,
            lstr!(en: "used before 'let'"; tr: "'let'ten önce kullanım"),
        ),
        lstr!(en: "instantiate it first: let {} = Module {{ }};", dut.text;
              tr: "önce örnekleyin: let {} = Modul {{ }};", dut.text),
    )
}

fn duplicate_dut(name: &Name) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E8506,
        lstr!(en: "a test drives a single instance; '{}' is a second one", name.text;
              tr: "bir test tek örnek sürer; '{}' ikinci örnek", name.text),
        LabeledSpan::primary(
            name.span,
            lstr!(en: "second 'let' here"; tr: "ikinci 'let' burada"),
        ),
        lstr!(en: "split the scenario into separate tests, one per instance";
              tr: "senaryoyu örnek başına ayrı testlere bölün"),
    )
}

fn missing_dut(test: &volt_ast::TestDecl) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E8506,
        lstr!(en: "test '{}' instantiates no module", test.name;
              tr: "'{}' testi hiçbir modül örneklemiyor", test.name),
        LabeledSpan::primary(
            test.name_span,
            lstr!(en: "no 'let' statement in this test"; tr: "bu testte 'let' deyimi yok"),
        ),
        lstr!(en: "start the test with: let dut = Module {{ }};";
              tr: "testi şöyle başlatın: let dut = Modul {{ }};"),
    )
}

fn unknown_builtin(func: &Name) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E8505,
        lstr!(en: "unknown test builtin '{}'", func.text;
              tr: "bilinmeyen test yerleşiği: '{}'", func.text),
        LabeledSpan::primary(
            func.span,
            lstr!(en: "not a test builtin"; tr: "test yerleşiği değil"),
        ),
        lstr!(en: "available: step(n), reset(), assert_eq(a, b), assert_ne(a, b), assert_true(a), assert_false(a)";
              tr: "mevcutlar: step(n), reset(), assert_eq(a, b), assert_ne(a, b), assert_true(a), assert_false(a)"),
    )
}

fn bad_arity(func: &Name, want: usize, got: usize, span: volt_span::Span) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E8505,
        lstr!(en: "'{}' expects {want} argument(s), got {got}", func.text;
              tr: "'{}' {want} argüman bekler, {got} verildi", func.text),
        LabeledSpan::primary(
            span,
            lstr!(en: "wrong number of arguments"; tr: "yanlış argüman sayısı"),
        ),
        lstr!(en: "see 'volt explain E8505' for the builtin signatures";
              tr: "yerleşik imzaları için 'volt explain E8505'"),
    )
}

fn bad_step_arg(arg: &TestExpr) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E8505,
        lstr!(en: "step() expects an integer cycle count >= 1";
              tr: "step() 1 ya da daha büyük bir tamsayı çevrim sayısı bekler"),
        LabeledSpan::primary(
            arg.span,
            lstr!(en: "invalid cycle count"; tr: "geçersiz çevrim sayısı"),
        ),
        lstr!(en: "write it as step(1)"; tr: "step(1) biçiminde yazın"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> SourceFile {
        volt_syntax::parser::parse(volt_span::FileId(0), src).ast
    }

    const COUNTER: &str = "module Counter {\n    in  clk    : clock\n    in  enable : bool\n    out count  : u8\n    reg count_r : u8 = 0\n    on clk { if enable { count_r <= count_r + 1 } }\n    count = count_r\n}\n";

    fn check_one(test_body: &str) -> Vec<&'static str> {
        let src = format!("{COUNTER}\ntest \"t\" {{\n{test_body}\n}}\n");
        let ast = parse(&src);
        check_tests(&[&ast], &ast, false)
            .iter()
            .map(|d| d.code.as_str())
            .collect()
    }

    #[test]
    fn valid_test_produces_no_diagnostics() {
        let codes = check_one(
            "let dut = Counter { };\ndut.enable = true;\nstep(1);\nassert_eq(dut.count, 1);\nreset();\nassert_true(dut.count);\nassert_false(dut.count);\nassert_ne(dut.count, 7);",
        );
        assert!(codes.is_empty(), "beklenmedik tanılar: {codes:?}");
    }

    #[test]
    fn unknown_module_is_e8501() {
        assert_eq!(check_one("let dut = Countr { };"), vec!["E8501"]);
    }

    #[test]
    fn unknown_port_is_e8502() {
        assert_eq!(
            check_one("let dut = Counter { };\ndut.enabel = true;"),
            vec!["E8502"]
        );
    }

    #[test]
    fn write_to_output_is_e8503() {
        assert_eq!(
            check_one("let dut = Counter { };\ndut.count = 3;"),
            vec!["E8503"]
        );
    }

    #[test]
    fn write_to_clock_is_e8503() {
        assert_eq!(
            check_one("let dut = Counter { };\ndut.clk = true;"),
            vec!["E8503"]
        );
    }

    #[test]
    fn read_from_input_is_e8504() {
        assert_eq!(
            check_one("let dut = Counter { };\nassert_eq(dut.enable, 1);"),
            vec!["E8504"]
        );
    }

    #[test]
    fn unknown_builtin_and_bad_arity_are_e8505() {
        assert_eq!(
            check_one("let dut = Counter { };\nwiggle(1);"),
            vec!["E8505"]
        );
        assert_eq!(check_one("let dut = Counter { };\nstep();"), vec!["E8505"]);
        assert_eq!(check_one("let dut = Counter { };\nstep(0);"), vec!["E8505"]);
        assert_eq!(
            check_one("let dut = Counter { };\nassert_eq(dut.count);"),
            vec!["E8505"]
        );
    }

    #[test]
    fn undefined_and_duplicate_dut_are_e8506() {
        // DUT'suz test: hem eksik-örnek hem tanımsız-kullanım tanısı.
        assert_eq!(check_one("dut.enable = true;"), vec!["E8506", "E8506"]);
        assert_eq!(
            check_one("let dut = Counter { };\nlet dut = Counter { };"),
            vec!["E8506"]
        );
        // Farklı ad da olsa ikinci örnek reddedilir (tek DUT kuralı).
        assert_eq!(
            check_one("let a = Counter { };\nlet b = Counter { };"),
            vec!["E8506"]
        );
    }

    #[test]
    fn sibling_assumption_skips_module_existence() {
        // Dosyada modül yok: kardeş dosya varsayımı ile E8501 üretilmez,
        // port denetimleri de yapılamaz.
        let ast = parse("test \"t\" {\n    let dut = Elsewhere { };\n    dut.x = 1;\n    assert_eq(dut.y, 1);\n}\n");
        let diags = check_tests(&[&ast], &ast, true);
        assert!(diags.is_empty(), "beklenmedik tanılar: {diags:?}");
    }

    #[test]
    fn merged_sources_resolve_sibling_modules() {
        let lib = parse(COUNTER);
        let test =
            parse("test \"t\" {\n    let dut = Counter { };\n    dut.enable = true;\n    step(1);\n    assert_eq(dut.count, 1);\n}\n");
        let diags = check_tests(&[&test, &lib], &test, false);
        assert!(diags.is_empty(), "beklenmedik tanılar: {diags:?}");
        // Kaynaklar birleşince bilinmeyen port yine yakalanır.
        let bad = parse("test \"t\" {\n    let dut = Counter { };\n    dut.enabel = true;\n}\n");
        let codes: Vec<_> = check_tests(&[&bad, &lib], &bad, false)
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>();
        assert_eq!(codes, vec!["E8502"]);
    }
}
