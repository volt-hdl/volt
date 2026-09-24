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

use crate::sim_const::TestConsts;
use crate::sim_expr::{Scope, VarKind};
use crate::sim_load;
use crate::sim_port;
use crate::testdata::TestFileLoader;

/// dut adı → Some(modül) | None (varlığı varsayılan dış modül).
pub(crate) type DutMap<'a> = HashMap<&'a str, Option<(&'a SourceFile, &'a ModuleDecl)>>;

/// Test gövdesinin tanıdığı deyim yerleşikleri: (ad, argüman sayısı).
pub const TEST_BUILTINS: &[(&str, usize)] = &[
    ("step", 1),
    ("reset", 0),
    ("assert_eq", 2),
    ("assert_ne", 2),
    ("assert_true", 1),
    ("assert_false", 1),
    ("load", 2),
];

/// Değer döndüren yerleşikler (ADR-0058): (ad, argüman sayısı).
pub const TEST_VALUE_BUILTINS: &[(&str, usize)] = &[("read_hex", 1), ("len", 1)];

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
///
/// Dosya erişimi yoktur: `read_hex` yolları yalnız sözcüksel olarak
/// denetlenir (bkz. [`check_tests_with_files`]).
pub fn check_tests(
    sources: &[&SourceFile],
    tests_from: &SourceFile,
    assume_external_modules: bool,
) -> Vec<Diagnostic> {
    check_tests_with_files(sources, tests_from, assume_external_modules, None)
}

/// [`check_tests`] + veri dosyası erişimi (ADR-0058): `files` verilirse
/// `read_hex` dosyaları okunur, biçimi (E8508) ve `load` boyutları
/// (E8510) gerçek içerikle denetlenir.
pub fn check_tests_with_files(
    sources: &[&SourceFile],
    tests_from: &SourceFile,
    assume_external_modules: bool,
    files: Option<&dyn TestFileLoader>,
) -> Vec<Diagnostic> {
    let modules = collect_modules(sources);
    let consts = TestConsts::new(sources);
    let mut diags = Vec::new();

    for idx in &tests_from.items {
        let ItemKind::Test(test) = &tests_from.items_arena[*idx].kind else {
            continue;
        };
        // Bir test tek DUT sürer (ADR-0033): Verilator modeli test
        // yürütülebiliri başına tek üst modülle kurulur.
        if !test
            .stmts
            .iter()
            .any(|s| matches!(s, TestStmt::LetDut { .. }))
        {
            diags.push(missing_dut(test));
        }
        // Struct portu yolları yaprak biçimine (ADR-0077).
        let stmts = crate::sim_struct::expand_struct_tests(sources, &test.stmts, Some(&mut diags));
        let mut checker = Checker {
            modules: &modules,
            assume_external_modules,
            files,
            scope: Scope::with_consts(consts.clone(), assume_external_modules),
            diags: &mut diags,
        };
        checker.check_block(&stmts, 0);
    }
    diags
}

/// Tek testin deyim yürüyücüsü: kapsamı taşır, tanıları biriktirir.
struct Checker<'a, 'd> {
    modules: &'d HashMap<String, (&'a SourceFile, &'a ModuleDecl)>,
    assume_external_modules: bool,
    files: Option<&'d dyn TestFileLoader>,
    scope: Scope<'a>,
    diags: &'d mut Vec<Diagnostic>,
}

impl<'a> Checker<'a, '_> {
    /// `depth`: `for` iç içelik derinliği (0 = test gövdesi).
    fn check_block(&mut self, stmts: &'a [TestStmt], depth: usize) {
        self.scope.push();
        for stmt in stmts {
            self.check_stmt(stmt, depth);
        }
        self.scope.pop();
    }

    fn check_stmt(&mut self, stmt: &'a TestStmt, depth: usize) {
        match stmt {
            TestStmt::LetDut { name, module, .. } => self.check_let_dut(name, module, depth),
            TestStmt::SetPort {
                dut, port, value, ..
            } => {
                check_set_port(&self.scope.duts, dut, port, self.diags);
                self.scope.expect_scalar(value, self.diags);
                sim_port::check_set_port_value(
                    &self.scope.duts,
                    &self.scope.consts,
                    dut,
                    port,
                    value,
                    self.diags,
                );
            }
            TestStmt::LetVar { name, value, .. } => {
                let kind = self.scope.let_value(value, self.files, self.diags);
                // Tek atamalı dil: sabit ifade bağlanan ad da sabittir.
                let constant = self.scope.consts.eval(value);
                self.define(name, kind, constant);
            }
            TestStmt::For {
                var,
                start,
                end,
                body,
                ..
            } => {
                self.scope.expect_scalar(start, self.diags);
                self.scope.expect_scalar(end, self.diags);
                self.scope.push();
                self.define(var, VarKind::Scalar, None); // sayaç koşuda değişir
                self.check_block(body, depth + 1);
                self.scope.pop();
            }
            TestStmt::Call { span, func, args } => self.check_call(*span, func, args),
        }
    }

    fn check_let_dut(&mut self, name: &'a Name, module: &Name, depth: usize) {
        if depth > 0 {
            self.diags.push(dut_inside_loop(name));
            return;
        }
        if !self.scope.duts.is_empty() {
            self.diags.push(duplicate_dut(name));
            return;
        }
        if self.scope.is_defined(&name.text) {
            self.diags.push(duplicate_name(name));
            return;
        }
        match self.modules.get(&module.text) {
            Some(found) => {
                self.scope.duts.insert(&name.text, Some(*found));
            }
            None if self.assume_external_modules => {
                self.scope.duts.insert(&name.text, None);
            }
            None => self.diags.push(unknown_module(module)),
        }
    }

    fn define(&mut self, name: &Name, kind: VarKind, constant: Option<u64>) {
        if self.scope.is_defined(&name.text) {
            self.diags.push(duplicate_name(name));
            // İki bağlamadan hangisinin kastedildiği belirsiz: eski değerle
            // sahte E8512 üretmemek için ad "bilinmiyor"a iner.
            self.scope.consts.bind(&name.text, None);
            return;
        }
        self.scope.define(&name.text, kind, constant);
    }

    fn check_call(&mut self, span: volt_span::Span, func: &Name, args: &'a [TestExpr]) {
        let Some((_, arity)) = TEST_BUILTINS.iter().find(|(name, _)| *name == func.text) else {
            self.diags.push(unknown_builtin(func));
            return;
        };
        if args.len() != *arity {
            self.diags.push(bad_arity(func, *arity, args.len(), span));
            return;
        }
        match func.text.as_str() {
            "step" => match &args[0].kind {
                TestExprKind::Int(0) => self.diags.push(bad_step_arg(&args[0])),
                _ => self.scope.expect_scalar(&args[0], self.diags),
            },
            "load" => {
                sim_load::check_load(&self.scope, self.modules, &args[0], &args[1], self.diags)
            }
            "assert_eq" | "assert_ne"
                if args
                    .iter()
                    .any(|a| crate::sim_struct::is_whole_struct_arg(&self.scope.duts, a)) =>
            {
                // Bütün-struct karşılaştırması (ADR-0077).
                let scope = &self.scope;
                crate::sim_struct::check_struct_compare(
                    &scope.duts,
                    args,
                    &mut |e, d| scope.expect_scalar(e, d),
                    self.diags,
                );
            }
            _ => {
                for arg in args {
                    self.scope.expect_scalar(arg, self.diags);
                }
                if matches!(func.text.as_str(), "assert_eq" | "assert_ne") {
                    sim_port::check_assert_compare(
                        &self.scope.duts,
                        &self.scope.consts,
                        args,
                        self.diags,
                    );
                }
            }
        }
    }
}

/// `dut.port = v` sol tarafı: dut tanımlı, port var, yönü `in`,
/// tipi `clock` değil.
fn check_set_port(duts: &DutMap<'_>, dut: &Name, port: &Name, diags: &mut Vec<Diagnostic>) {
    let Some(entry) = duts.get(dut.text.as_str()) else {
        diags.push(undefined_dut(dut));
        return;
    };
    let Some((src, module)) = entry else {
        return; // dış modül varsayımı: port denetimi yapılamaz
    };
    let Some(p) = crate::sim_struct::find_port(src, module, &port.text) else {
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

/// `dut.port` okuması: dut tanımlı, port var ve yönü `out`.
pub(crate) fn check_port_read(
    duts: &DutMap<'_>,
    dut: &Name,
    port: &Name,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(entry) = duts.get(dut.text.as_str()) else {
        diags.push(undefined_dut(dut));
        return;
    };
    let Some((src, module)) = entry else {
        return; // dış modül varsayımı
    };
    let Some(p) = crate::sim_struct::find_port(src, module, &port.text) else {
        diags.push(unknown_port(port, &module.name.text));
        return;
    };
    // Bütün struct portu sayı değildir (ADR-0077): alan ya da literal.
    if let Some(layout) = crate::sim_struct::struct_port_layout(src, module, &port.text) {
        diags.push(crate::sim_expr::type_mismatch(
            port.span,
            lstr!(en: "port '{}' is struct '{}', not a number; read a field (dut.{}.<field>) or compare it with a literal in assert_eq", port.text, layout.name, port.text;
                  tr: "'{}' portu '{}' struct'ı, sayı değil; bir alanını okuyun (dut.{}.<alan>) ya da assert_eq içinde bir literalle karşılaştırın", port.text, layout.name, port.text),
        ));
        return;
    }
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

pub(crate) fn undefined_dut(dut: &Name) -> Diagnostic {
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

fn dut_inside_loop(name: &Name) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E8506,
        lstr!(en: "instance '{}' is created inside a loop", name.text;
              tr: "'{}' örneği döngü içinde oluşturuluyor", name.text),
        LabeledSpan::primary(
            name.span,
            lstr!(en: "'let' of an instance inside 'for'"; tr: "'for' içinde örnek 'let'i"),
        ),
        lstr!(en: "a test drives a single instance; create it once at the top of the test";
              tr: "bir test tek örnek sürer; onu testin başında bir kez oluşturun"),
    )
}

fn duplicate_name(name: &Name) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E8506,
        lstr!(en: "'{}' is already defined in this test", name.text;
              tr: "'{}' bu testte zaten tanımlı", name.text),
        LabeledSpan::primary(
            name.span,
            lstr!(en: "second definition here"; tr: "ikinci tanım burada"),
        ),
        lstr!(en: "test names cannot be shadowed; pick another name";
              tr: "test adları gölgelenemez; başka bir ad seçin"),
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

pub(crate) fn unknown_builtin(func: &Name) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E8505,
        lstr!(en: "unknown test builtin '{}'", func.text;
              tr: "bilinmeyen test yerleşiği: '{}'", func.text),
        LabeledSpan::primary(
            func.span,
            lstr!(en: "not a test builtin"; tr: "test yerleşiği değil"),
        ),
        lstr!(en: "statements: step(n), reset(), assert_eq(a, b), assert_ne(a, b), assert_true(a), assert_false(a), load(dut.mem, data); values: read_hex(\"file\"), len(array)";
              tr: "deyimler: step(n), reset(), assert_eq(a, b), assert_ne(a, b), assert_true(a), assert_false(a), load(dut.mem, veri); değerler: read_hex(\"dosya\"), len(dizi)"),
    )
}

pub(crate) fn bad_arity(func: &Name, want: usize, got: usize, span: volt_span::Span) -> Diagnostic {
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

    const ENUM_DUT: &str = "enum State { Idle, Run, Done }
enum Mode { A, B }
module Fsm {
    in  clk : clock
    in  cmd : State
    out st  : State
    reg s : State = State::Idle
    on clk { s <= cmd }
    st = s
}
";

    fn check_enum(test_body: &str) -> Vec<&'static str> {
        let src = format!(
            "{ENUM_DUT}
test \"t\" {{
{test_body}
}}
"
        );
        let ast = parse(&src);
        check_tests(&[&ast], &ast, false)
            .iter()
            .map(|d| d.code.as_str())
            .collect()
    }

    #[test]
    fn enum_variants_are_test_values() {
        let codes = check_enum(
            "let dut = Fsm { };
dut.cmd = State::Run;
step(1);
assert_eq(dut.st, State::Run);
assert_ne(dut.st, State::Done);",
        );
        assert!(codes.is_empty(), "{codes:?}");
    }

    #[test]
    fn unknown_variant_is_e8511_and_unknown_enum_e8506() {
        assert_eq!(
            check_enum(
                "let dut = Fsm { };
assert_eq(dut.st, State::Stop);"
            ),
            ["E8511"]
        );
        assert_eq!(
            check_enum(
                "let dut = Fsm { };
assert_eq(dut.st, Phase::Go);"
            ),
            ["E8506"]
        );
    }

    #[test]
    fn comparing_an_enum_port_with_another_enum_is_e8511() {
        assert_eq!(
            check_enum(
                "let dut = Fsm { };
assert_eq(dut.st, Mode::A);"
            ),
            ["E8511"]
        );
    }

    #[test]
    fn enum_values_from_named_constants_are_test_values() {
        // Bulgu: açık değer `const` ya da aritmetik olunca enum test
        // dilinde görünmüyordu (yanlış E8506).
        let src = "const K : u4 = 2
enum S : u4 { A = K, B = 1 << 3 }
module Fsm {
    in  clk : clock
    in  cmd : S
    out st  : S
    reg s : S = S::A
    on clk { s <= cmd }
    st = s
}
test \"t\" {
let dut = Fsm { };
dut.cmd = S::B;
step(1);
assert_eq(dut.st, S::B);
}
";
        let ast = parse(src);
        let codes: Vec<&str> = check_tests(&[&ast], &ast, false)
            .iter()
            .map(|d| d.code.as_str())
            .collect();
        assert!(codes.is_empty(), "{codes:?}");
        let consts = crate::sim_const::TestConsts::new(&[&ast]);
        assert_eq!(consts.variant("S", "A"), Some(2));
        assert_eq!(consts.variant("S", "B"), Some(8));
    }

    #[test]
    fn integer_into_enum_port_is_allowed_but_width_checked() {
        // Geçersiz kod enjeksiyonu testte bilerek yapılabilir (ADR-0074).
        assert!(check_enum(
            "let dut = Fsm { };
dut.cmd = 3;"
        )
        .is_empty());
        assert_eq!(
            check_enum(
                "let dut = Fsm { };
dut.cmd = 4;"
            ),
            ["E8512"]
        );
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
