//! Test dili genişletmesinin anlamsal denetimi (ADR-0058):
//! kapsam, sayı/dizi ayrımı, `read_hex` (E8507/E8508) ve `load`
//! (E8509/E8510).

use volt_ast::SourceFile;
use volt_hir::{check_tests, check_tests_with_files, TestFileError, TestFileLoader};

fn parse(src: &str) -> SourceFile {
    let parsed = volt_syntax::parser::parse(volt_span::FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "ayrışmalı: {:?}",
        parsed.error_codes()
    );
    parsed.ast
}

const SOC: &str = "\
const RAM_WORDS : u32 = 4

module Core {
    in  clk : clock
    in  we  : bool
    out q   : u8
    reg imem : [u8; 16] = [0; 16]
    reg wide : [bits<96>; 2] = [0; 2]
    reg r    : u8 = 0
    on clk { if we { imem[0] <= 1  r <= imem[1] } }
    q = r
}

module Soc {
    in  clk   : clock
    in  we    : bool
    in  addr  : u2
    out q     : u8
    out busy  : bool
    reg ram   : [u32; RAM_WORDS] = [0; RAM_WORDS]
    reg count : u8 = 0
    let cpu = Core { clk: clk, we: we }
    on clk { if we { ram[addr] <= 7  count <= count + 1 } }
    q = cpu.q
    busy = we
}
";

/// Sabit içerik veren yükleyici; `None` = dosya yok.
struct Files(Option<&'static str>);

impl TestFileLoader for Files {
    fn load(&self, rel_path: &str) -> Result<String, TestFileError> {
        if rel_path.starts_with("..") {
            return Err(TestFileError::OutsideProject);
        }
        self.0
            .map(str::to_string)
            .ok_or_else(|| TestFileError::NotFound("no such file".to_string()))
    }
}

fn codes_with(body: &str, files: Option<&dyn TestFileLoader>) -> Vec<&'static str> {
    let src = format!("{SOC}\ntest \"t\" {{\n    let dut = Soc {{ }};\n{body}\n}}\n");
    let ast = parse(&src);
    check_tests_with_files(&[&ast], &ast, false, files)
        .iter()
        .map(|d| d.code.as_str())
        .collect()
}

fn codes(body: &str) -> Vec<&'static str> {
    codes_with(body, None)
}

// ─── Kapsam ve tipler ─────────────────────────────────────────────

#[test]
fn arrays_loops_and_expressions_are_clean() {
    let found = codes(
        "    let expected = [1, 2, 3];\n    let n = len(expected) - 1;\n    for i in 0..len(expected) {\n        dut.addr = i & 3;\n        step(n + 1);\n        assert_eq(dut.q, expected[i] * 2);\n    }",
    );
    assert!(found.is_empty(), "beklenmedik tanılar: {found:?}");
}

#[test]
fn undefined_name_is_e8506() {
    assert_eq!(codes("    assert_eq(dut.q, missing);"), vec!["E8506"]);
    assert_eq!(codes("    assert_eq(dut.q, missing[0]);"), vec!["E8506"]);
}

#[test]
fn loop_variable_is_not_visible_after_the_loop() {
    assert_eq!(
        codes("    for i in 0..4 {\n        step(1);\n    }\n    dut.addr = i;"),
        vec!["E8506"]
    );
}

#[test]
fn block_local_let_is_not_visible_outside() {
    assert_eq!(
        codes("    for i in 0..4 {\n        let twice = i * 2;\n    }\n    step(twice);"),
        vec!["E8506"]
    );
}

#[test]
fn shadowing_any_visible_name_is_e8506() {
    assert_eq!(codes("    let n = 1;\n    let n = 2;"), vec!["E8506"]);
    assert_eq!(
        codes("    let i = 1;\n    for i in 0..4 {\n        step(1);\n    }"),
        vec!["E8506"]
    );
    assert_eq!(codes("    let dut = 3;"), vec!["E8506"]);
}

#[test]
fn instance_inside_a_loop_is_e8506() {
    let src = format!(
        "{SOC}\ntest \"t\" {{\n    let dut = Soc {{ }};\n    for i in 0..2 {{\n        let other = Core {{ }};\n    }}\n}}\n"
    );
    let ast = parse(&src);
    let found: Vec<_> = check_tests(&[&ast], &ast, false)
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(found, vec!["E8506"]);
}

#[test]
fn array_where_a_number_is_expected_is_e8511() {
    assert_eq!(
        codes("    let a = [1, 2];\n    assert_eq(dut.q, a);"),
        vec!["E8511"]
    );
    assert_eq!(
        codes("    let a = [1, 2];\n    dut.addr = a;"),
        vec!["E8511"]
    );
    assert_eq!(codes("    let a = [1, 2];\n    step(a);"), vec!["E8511"]);
    assert_eq!(
        codes("    let a = [1, 2];\n    for i in 0..a {\n        step(1);\n    }"),
        vec!["E8511"]
    );
}

#[test]
fn number_where_an_array_is_expected_is_e8511() {
    assert_eq!(
        codes("    let n = 4;\n    assert_eq(dut.q, n[0]);"),
        vec!["E8511"]
    );
    assert_eq!(codes("    let n = 4;\n    step(len(n));"), vec!["E8511"]);
    assert_eq!(codes("    step(len(3));"), vec!["E8511"]);
}

#[test]
fn array_literal_rules_are_e8511() {
    assert_eq!(codes("    let a = [];"), vec!["E8511"]);
    assert_eq!(codes("    let a = [1, dut.q];"), vec!["E8511"]);
    // Dizi literali yalnız `let` sağ tarafında geçerlidir.
    assert_eq!(codes("    assert_eq(dut.q, [1, 2]);"), vec!["E8511"]);
}

#[test]
fn constant_index_out_of_bounds_is_e8511() {
    assert_eq!(
        codes("    let a = [1, 2];\n    assert_eq(dut.q, a[2]);"),
        vec!["E8511"]
    );
    assert!(codes("    let a = [1, 2];\n    assert_eq(dut.q, a[1]);").is_empty());
}

#[test]
fn stray_string_and_instance_value_are_e8511() {
    assert_eq!(codes("    dut.addr = \"three\";"), vec!["E8511"]);
    assert_eq!(codes("    step(dut);"), vec!["E8511"]);
}

#[test]
fn unknown_value_builtin_and_bad_arity_are_e8505() {
    assert_eq!(codes("    step(read_csv(\"a.csv\"));"), vec!["E8505"]);
    assert_eq!(
        codes("    let a = [1];\n    step(len(a, a));"),
        vec!["E8505"]
    );
    assert_eq!(codes("    let a = read_hex();"), vec!["E8505"]);
    assert_eq!(codes("    let a = [1];\n    load(dut.ram);"), vec!["E8505"]);
}

#[test]
fn step_zero_literal_is_still_rejected() {
    assert_eq!(codes("    step(0);"), vec!["E8505"]);
}

// ─── read_hex ─────────────────────────────────────────────────────

#[test]
fn read_hex_outside_project_is_e8507_even_without_file_access() {
    assert_eq!(
        codes("    let rom = read_hex(\"../../secret.hex\");"),
        vec!["E8507"]
    );
    assert_eq!(
        codes("    let rom = read_hex(\"/etc/passwd\");"),
        vec!["E8507"]
    );
    assert_eq!(
        codes("    let rom = read_hex(\"C:\\\\Windows\\\\win.ini\");"),
        vec!["E8507"]
    );
}

#[test]
fn read_hex_inside_project_without_file_access_is_clean() {
    let found = codes("    let rom = read_hex(\"sw/hello.hex\");\n    load(dut.ram, rom);");
    assert!(found.is_empty(), "beklenmedik tanılar: {found:?}");
}

#[test]
fn read_hex_missing_file_is_e8507() {
    let files = Files(None);
    assert_eq!(
        codes_with("    let rom = read_hex(\"nope.hex\");", Some(&files)),
        vec!["E8507"]
    );
}

#[test]
fn read_hex_loader_rejection_is_e8507() {
    let files = Files(Some("01\n"));
    assert_eq!(
        codes_with("    let rom = read_hex(\"../up.hex\");", Some(&files)),
        vec!["E8507"]
    );
}

#[test]
fn read_hex_malformed_file_is_e8508_with_line() {
    let files = Files(Some("00000013\n0000G093\n"));
    let src = format!(
        "{SOC}\ntest \"t\" {{\n    let dut = Soc {{ }};\n    let rom = read_hex(\"bad.hex\");\n}}\n"
    );
    let ast = parse(&src);
    let diags = check_tests_with_files(&[&ast], &ast, false, Some(&files));
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code.as_str(), "E8508");
    assert!(diags[0].message.contains("line 2"), "{}", diags[0].message);
}

#[test]
fn read_hex_must_be_bound_with_let_and_take_a_string() {
    assert_eq!(codes("    step(read_hex(\"a.hex\"));"), vec!["E8511"]);
    assert_eq!(codes("    let rom = read_hex(3);"), vec!["E8511"]);
}

// ─── load ─────────────────────────────────────────────────────────

#[test]
fn load_into_top_and_nested_memory_is_clean() {
    let found = codes(
        "    let data = [1, 2, 3, 4];\n    let bytes = [0xFF];\n    load(dut.ram, data);\n    load(dut.cpu.imem, bytes);",
    );
    assert!(found.is_empty(), "beklenmedik tanılar: {found:?}");
}

#[test]
fn load_target_that_is_not_an_array_register_is_e8509() {
    let data = "    let data = [1];\n";
    for target in [
        "dut.busy",     // port
        "dut.count",    // skalar yazmaç
        "dut.nope",     // yok
        "dut.gpu.imem", // örnek yok
        "dut.cpu.q",    // alt modülün portu
        "dut.cpu.wide", // 64 bitten geniş eleman
        "data",         // yol değil
    ] {
        assert_eq!(
            codes(&format!("{data}    load({target}, data);")),
            vec!["E8509"],
            "hedef: {target}"
        );
    }
}

#[test]
fn load_source_longer_than_target_is_e8510() {
    // Boyut düz `const` üzerinden de çözülür (RAM_WORDS = 4).
    assert_eq!(
        codes("    let data = [1, 2, 3, 4, 5];\n    load(dut.ram, data);"),
        vec!["E8510"]
    );
}

#[test]
fn load_value_wider_than_element_is_e8510() {
    assert_eq!(
        codes("    let data = [0x1FF];\n    load(dut.cpu.imem, data);"),
        vec!["E8510"]
    );
}

#[test]
fn load_checks_real_file_contents() {
    let files = Files(Some("@0 1 2 3 4 5\n"));
    assert_eq!(
        codes_with(
            "    let rom = read_hex(\"rom.hex\");\n    load(dut.ram, rom);",
            Some(&files)
        ),
        vec!["E8510"]
    );
}

#[test]
fn load_source_must_be_an_array_name() {
    assert_eq!(codes("    load(dut.ram, 3);"), vec!["E8511"]);
    assert_eq!(
        codes("    let n = 3;\n    load(dut.ram, n);"),
        vec!["E8511"]
    );
}

// ─── İnceleme bulguları ───────────────────────────────────────────

#[test]
fn not_an_array_is_reported_at_the_load_call_not_at_the_register() {
    // Yazmaç kardeş dosyada olabilir; onun span'ı test dosyasının
    // haritasında anlamsızdır. Birincil konum load() hedefidir.
    let src = format!(
        "{SOC}\ntest \"t\" {{\n    let dut = Soc {{ }};\n    let d = [1];\n    load(dut.count, d);\n}}\n"
    );
    let ast = parse(&src);
    let diags = check_tests(&[&ast], &ast, false);
    assert_eq!(diags.len(), 1);
    let span = diags[0].primary_span().expect("birincil konum").span;
    let at = src.find("dut.count").expect("hedef") as u32;
    assert_eq!(span.start, at, "konum load() hedefinde olmalı");
}

#[test]
fn const_element_width_is_checked() {
    let src = "const W : u32 = 12\nmodule M {\n    in clk : clock\n    in we : bool\n    out q : bool\n    reg m : [uint<W>; 4] = [0; 4]\n    on clk { if we { m[0] <= 1 } }\n    q = we\n}\ntest \"t\" {\n    let dut = M { };\n    let d = [0xFFFF];\n    load(dut.m, d);\n}\n";
    let ast = parse(src);
    let codes: Vec<_> = check_tests(&[&ast], &ast, false)
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(codes, vec!["E8510"]);
}

#[test]
fn nested_array_register_is_not_a_load_target() {
    let src = "module M {\n    in clk : clock\n    in we : bool\n    out q : bool\n    reg m2 : [[u8; 2]; 2] = [[0; 2]; 2]\n    on clk { if we { m2[0][0] <= 1 } }\n    q = we\n}\ntest \"t\" {\n    let dut = M { };\n    let d = [1];\n    load(dut.m2, d);\n}\n";
    let ast = parse(src);
    let codes: Vec<_> = check_tests(&[&ast], &ast, false)
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(codes, vec!["E8509"]);
}

#[test]
fn instance_cannot_reuse_a_value_name() {
    let src = format!("{SOC}\ntest \"t\" {{\n    let x = 5;\n    let x = Soc {{ }};\n}}\n");
    let ast = parse(&src);
    let codes: Vec<_> = check_tests(&[&ast], &ast, false)
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(codes, vec!["E8506"]);
}
