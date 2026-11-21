//! Takma ad zinciri ve tip/düzen sabiti tek yerde (`volt_ast::alias`,
//! `volt_ast::const_eval`; #376, ADR-0132): SV üreticisi, enum/struct
//! düzeni ve test dili aynı fonksiyonları çağırır. Bu testler ortak
//! fonksiyonların anlamını sabitler — SV üreticisinin eski kopyasınınki.

use volt_ast::alias::{alias_chain_end, MAX_ALIAS_DEPTH};
use volt_ast::const_eval::{const_int, MAX_CONST_DEPTH};
use volt_ast::{Idx, ItemKind, SourceFile, TypeRef, TypeRefKind};

fn parse_any(src: &str) -> SourceFile {
    volt_syntax::parser::parse(volt_span::FileId(0), src).ast
}

fn parse(src: &str) -> SourceFile {
    let parsed = volt_syntax::parser::parse(volt_span::FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "ayrışmalı: {:?}",
        parsed.error_codes()
    );
    parsed.ast
}

/// `module M`'nin `name` portunun tipi.
fn port_ty(ast: &SourceFile, name: &str) -> Idx<TypeRef> {
    ast.items
        .iter()
        .find_map(|&i| match &ast.items_arena[i].kind {
            ItemKind::Module(m) => m.ports.iter().find(|p| p.name.text == name).map(|p| p.ty),
            _ => None,
        })
        .unwrap_or_else(|| panic!("port {name}"))
}

/// Üst düzey `const name`'in değeri.
fn const_of(ast: &SourceFile, name: &str) -> Option<i128> {
    let value = ast
        .items
        .iter()
        .find_map(|&i| match &ast.items_arena[i].kind {
            ItemKind::Const(c) if c.name.text == name => Some(c.value),
            _ => None,
        })
        .unwrap_or_else(|| panic!("const {name}"));
    const_int(ast, value)
}

const ALIASES: &str = "\
type Nib = u4
type Outer = Nib
type G<T> = T

module M {
    in  clk   : clock
    in  plain : u8
    in  one   : Nib
    in  two   : Outer
    in  gen   : G<u4>
    in  myst  : Mystery
    out q     : u8
    q = plain
}
";

#[test]
fn an_alias_chain_ends_at_its_target_type() {
    let ast = parse(ALIASES);
    for port in ["one", "two"] {
        let end = alias_chain_end(&ast, port_ty(&ast, port)).expect("zincir biter");
        assert!(
            matches!(ast.types[end].kind, TypeRefKind::UInt(4)),
            "{port}"
        );
    }
}

#[test]
fn a_type_that_is_not_a_plain_alias_is_its_own_end() {
    let ast = parse(ALIASES);
    // Takma ad değil, generic argümanlı yol, bilinmeyen ad: tipin kendisi.
    for port in ["plain", "gen", "myst"] {
        let ty = port_ty(&ast, port);
        assert_eq!(alias_chain_end(&ast, ty), Ok(ty), "{port}");
    }
}

#[test]
fn a_generic_alias_is_not_followed() {
    // `type H = G` gibi argümansız kullanım bile generic takma adı açmaz.
    let ast = parse("type G<T> = T\nmodule M {\n    in  clk : clock\n    in  a   : G\n    out q   : bool\n    q = false\n}\n");
    let ty = port_ty(&ast, "a");
    assert_eq!(alias_chain_end(&ast, ty), Ok(ty));
}

#[test]
fn a_cycle_or_an_overlong_chain_stops_at_the_depth_limit() {
    // Döngü parser'da E4009 alır; ortak fonksiyon yine de sonlanır.
    let ast = parse_any("type A = B\ntype B = A\nmodule M {\n    in  clk : clock\n    in  a   : A\n    out q   : bool\n    q = false\n}\n");
    assert!(alias_chain_end(&ast, port_ty(&ast, "a")).is_err());
    // Döngüsüz zincir: A0 = A1, ..., A{n} = u4 (n + 1 takma ad). Sınır,
    // SV üreticisinin eski kopyasındaki gibi: en çok MAX_ALIAS_DEPTH - 1
    // takma ad sonuna kadar izlenir.
    let chain = |n: usize| {
        let mut src = String::new();
        for i in 0..n {
            src.push_str(&format!("type A{i} = A{}\n", i + 1));
        }
        src.push_str(&format!("type A{n} = u4\n"));
        src.push_str("module M {\n    in  clk : clock\n    in  a   : A0\n    out q   : bool\n    q = false\n}\n");
        parse_any(&src)
    };
    let fits = chain(MAX_ALIAS_DEPTH - 2);
    let end = alias_chain_end(&fits, port_ty(&fits, "a")).expect("sınır içinde biter");
    assert!(matches!(fits.types[end].kind, TypeRefKind::UInt(4)));
    let long = chain(MAX_ALIAS_DEPTH - 1);
    assert!(alias_chain_end(&long, port_ty(&long, "a")).is_err());
}

#[test]
fn width_constants_fold_like_the_sv_generator() {
    let ast = parse(
        "\
const ADD : u32 = 2 + 2
const SUB : u32 = 10 - 3
const MUL : u32 = 3 * 4
const DIV : u32 = 9 / 2
const REM : u32 = 7 % 3
const SHL : u32 = 1 << 4
const SHR : u32 = 32 >> 2
const NEG : i32 = 0 - (-5)
const REF : u32 = ADD + 1
const DIV0 : u32 = 1 / 0
const AND : u32 = 3 & 1
const BOOL : bool = true
",
    );
    let expect = [
        ("ADD", Some(4)),
        ("SUB", Some(7)),
        ("MUL", Some(12)),
        ("DIV", Some(4)),
        ("REM", Some(1)),
        ("SHL", Some(16)),
        ("SHR", Some(8)),
        ("NEG", Some(5)),
        ("REF", Some(5)),
        ("DIV0", None),
        ("AND", None),
        ("BOOL", None),
    ];
    for (name, want) in expect {
        assert_eq!(const_of(&ast, name), want, "{name}");
    }
}

#[test]
fn constant_nesting_is_limited_to_the_sv_generators_depth() {
    // `-(-( … 1 … ))`: her tekli eksi bir iç içelik. Sınır, SV
    // üreticisinin eski kopyasındaki gibi 32.
    let nested = |n: u32| {
        let open = "-(".repeat(n as usize);
        let close = ")".repeat(n as usize);
        parse(&format!("const D : i32 = {open}1{close}\n"))
    };
    let at_limit = nested(MAX_CONST_DEPTH);
    assert_eq!(const_of(&at_limit, "D"), Some(1), "32 eksi: çift, değer 1");
    let past = nested(MAX_CONST_DEPTH + 1);
    assert_eq!(const_of(&past, "D"), None);
}

#[test]
fn only_a_single_segment_path_names_a_constant() {
    // `m::K` bir üst düzey sabit değildir: ilk parçası (`m`) aynı adlı
    // sabite eşlenmez.
    let ast = parse("const m : u32 = 3\nconst X : u32 = m::K\n");
    assert_eq!(const_of(&ast, "m"), Some(3));
    assert_eq!(const_of(&ast, "X"), None);
}

#[test]
fn a_const_chain_and_both_operands_count_toward_the_depth_limit() {
    let limit = MAX_CONST_DEPTH as usize;
    // K0 = K1, ..., K{n} = 1: en içteki değer n iç içelikte.
    let chain = |n: usize| {
        let mut src = String::new();
        for i in 0..n {
            src.push_str(&format!("const K{i} : u32 = K{}\n", i + 1));
        }
        src.push_str(&format!("const K{n} : u32 = 1\n"));
        parse(&src)
    };
    assert_eq!(const_of(&chain(limit), "K0"), Some(1));
    assert_eq!(const_of(&chain(limit + 1), "K0"), None);
    // Sola iç içe toplam `1 + 1 + … + 1` (n işlem): sol operand derinliği.
    let left = |n: usize| {
        let sum = vec!["1"; n + 1].join(" + ");
        parse(&format!("const L : u32 = {sum}\n"))
    };
    assert_eq!(const_of(&left(limit), "L"), Some(limit as i128 + 1));
    assert_eq!(const_of(&left(limit + 1), "L"), None);
    // Sağa iç içe toplam `1 + (1 + (… + 1))`: sağ operand derinliği.
    let right = |n: usize| {
        let open = "1 + (".repeat(n - 1);
        let close = ")".repeat(n - 1);
        parse(&format!("const R : u32 = {open}1 + 1{close}\n"))
    };
    assert_eq!(const_of(&right(limit), "R"), Some(limit as i128 + 1));
    assert_eq!(const_of(&right(limit + 1), "R"), None);
}
