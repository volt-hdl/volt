//! Çoklu dosya derleme birimi (ADR-0042): import görünürlüğü çözücüye
//! kadar iner mi? `parse_unit` + `check_imports` + `analyze_unit`.

use std::collections::HashMap;

use volt_hir::{analyze_unit, check_imports, AnalysisResult, UnitInfo};
use volt_span::FileId;
use volt_syntax::parse_unit;

const LIB: &str = "package lib;\n\npub module Ticker {\n    in  clk    : clock\n    in  enable : bool\n    out count  : u8\n    reg r : u8 = 0\n    on clk { if enable { r <= r + 1 } }\n    count = r\n}\n\nmodule Hidden {\n    in  clk : clock\n    in  a   : bool\n    out b   : bool\n    b = a\n}\n";

fn top(uses: &str, target: &str) -> String {
    format!(
        "{uses}\nmodule Top {{\n    in  clk    : clock\n    in  enable : bool\n    out count  : u8\n    let c = {target} {{ clk: clk, enable: enable }}\n    count = c.count\n}}\n"
    )
}

/// Dosyalar: (paket, kaynak). Ana dosya SONDA (bağımlılık sırası).
fn analyze(files: &[(&str, &str)]) -> (Vec<&'static str>, AnalysisResult) {
    let sources: Vec<(FileId, &str)> = files
        .iter()
        .enumerate()
        .map(|(i, (_, src))| (FileId(i as u32), *src))
        .collect();
    let parsed = parse_unit(&sources);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.error_codes());
    let mut info = UnitInfo::default();
    for (i, (pkg, _)) in files.iter().enumerate() {
        info.add(
            FileId(i as u32),
            pkg.split("::").map(str::to_string).collect(),
        );
    }
    let imports = check_imports(&parsed.ast, &info);
    let mut codes: Vec<&'static str> = imports
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    let scopes: HashMap<FileId, _> = imports.scopes;
    let result = analyze_unit(&parsed.ast, &scopes);
    codes.extend(
        result
            .diagnostics
            .iter()
            .filter(|d| !d.code.is_warning())
            .map(|d| d.code.as_str()),
    );
    (codes, result)
}

#[test]
fn public_item_imported_from_other_file_resolves() {
    let main = top("use lib::Ticker;", "Ticker");
    let (codes, _) = analyze(&[("lib", LIB), ("main", &main)]);
    assert!(codes.is_empty(), "{codes:?}");
}

#[test]
fn unimported_item_from_other_file_is_e1001() {
    let main = top("", "Ticker");
    let (codes, _) = analyze(&[("lib", LIB), ("main", &main)]);
    assert!(codes.contains(&"E1001"), "{codes:?}");
}

#[test]
fn private_item_via_use_is_e1004_and_not_resolved() {
    let main = "use lib::Hidden;\nmodule Top {\n    in  clk : clock\n    in  a : bool\n    out b : bool\n    let h = Hidden { clk: clk, a: a }\n    b = h.b\n}\n";
    let (codes, _) = analyze(&[("lib", LIB), ("main", main)]);
    assert!(codes.contains(&"E1004"), "{codes:?}");
}

#[test]
fn alias_import_is_usable_under_its_local_name() {
    let main = top("use lib::Ticker as Cnt;", "Cnt");
    let (codes, _) = analyze(&[("lib", LIB), ("main", &main)]);
    assert!(codes.is_empty(), "{codes:?}");
}

#[test]
fn alias_does_not_expose_the_original_name() {
    let main = top("use lib::Ticker as Cnt;", "Ticker");
    let (codes, _) = analyze(&[("lib", LIB), ("main", &main)]);
    assert!(codes.contains(&"E1001"), "{codes:?}");
}

#[test]
fn glob_import_resolves_public_items() {
    let main = top("use lib::*;", "Ticker");
    let (codes, _) = analyze(&[("lib", LIB), ("main", &main)]);
    assert!(codes.is_empty(), "{codes:?}");
}

#[test]
fn glob_import_does_not_expose_private_items() {
    let main = "use lib::*;\nmodule Top {\n    in  clk : clock\n    in  a : bool\n    out b : bool\n    let h = Hidden { clk: clk, a: a }\n    b = h.b\n}\n";
    let (codes, _) = analyze(&[("lib", LIB), ("main", main)]);
    assert!(codes.contains(&"E1001"), "{codes:?}");
}

#[test]
fn three_level_chain_resolves_transitively() {
    // mid imports lib; main imports mid — main must NOT see Ticker without its own use.
    let mid = "package mid;\nuse lib::Ticker;\npub module Pair {\n    in  clk    : clock\n    in  enable : bool\n    out count  : u8\n    let c = Ticker { clk: clk, enable: enable }\n    count = c.count\n}\n";
    let main = top("use mid::Pair;", "Pair");
    let (codes, _) = analyze(&[("lib", LIB), ("mid", mid), ("main", &main)]);
    assert!(codes.is_empty(), "{codes:?}");
    let leaky = top("use mid::Pair;", "Ticker");
    let (codes, _) = analyze(&[("lib", LIB), ("mid", mid), ("main", &leaky)]);
    assert!(codes.contains(&"E1001"), "{codes:?}");
}

#[test]
fn diagnostics_point_at_the_right_file() {
    let main = top("", "Ticker");
    let (_, result) = analyze(&[("lib", LIB), ("main", &main)]);
    let e1001 = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E1001")
        .expect("E1001");
    assert_eq!(e1001.primary_span().map(|s| s.span.file), Some(FileId(1)));
}

#[test]
fn ambiguous_import_across_two_packages_is_e1010() {
    let other = LIB.replace("package lib;", "package other;");
    let main = top("use lib::Ticker;\nuse other::Ticker;", "Ticker");
    let (codes, _) = analyze(&[("lib", LIB), ("other", &other), ("main", &main)]);
    assert!(codes.contains(&"E1010"), "{codes:?}");
}

#[test]
fn generic_module_defined_in_other_file_is_instantiable() {
    // Monomorphised `Delay_4_8` inherits the visibility of `Delay` (mono_base).
    let lib = "package lib;
pub module Delay<const N: u32, const W: u32> {
    in  clk : clock
    in  x   : uint<W>
    out y   : uint<W>
    reg line : [uint<W>; N] = [0; N]
    on clk {
        for i in 1..N { line[i] <= line[i - 1] }
        line[0] <= x
    }
    y = line[N - 1]
}
";
    let main = "use lib::Delay;
module Top {
    in  clk : clock
    in  x   : u8
    out a   : u8
    let d4 = Delay<4, 8> { clk, x }
    a = d4.y
}
";
    let (codes, _) = analyze(&[("lib", lib), ("main", main)]);
    assert!(codes.is_empty(), "{codes:?}");
}
