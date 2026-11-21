//! Takma ad zinciri ve derleme zamanı sabiti tek yerde (`volt_ast::alias`,
//! `volt_ast::const_eval`; #376, ADR-0132): SV üreticisi (port genişliği
//! dahil), enum/struct düzeni, testbench ve test dili aynı fonksiyonları
//! çağırır. Bu testler ortak fonksiyonların anlamını sabitler — SV
//! üreticisinin port genişliğini hesaplayan eski kopyasınınki.

use volt_ast::alias::{alias_chain_end, MAX_ALIAS_DEPTH};
use volt_ast::const_eval::{
    const_int, eval, width, Binding, Scope, TopLevel, Unknown, MAX_CONST_DEPTH, MAX_TABLE_LEN,
};
use volt_ast::{Expr, Idx, ItemKind, SourceFile, TypeRef, TypeRefKind};
use volt_hir::test_port_width;

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

/// Üst düzey `const name`'in değer ifadesi.
fn const_expr(ast: &SourceFile, name: &str) -> Idx<Expr> {
    ast.items
        .iter()
        .find_map(|&i| match &ast.items_arena[i].kind {
            ItemKind::Const(c) if c.name.text == name => Some(c.value),
            _ => None,
        })
        .unwrap_or_else(|| panic!("const {name}"))
}

/// Üst düzey `const name`'in değeri.
fn const_of(ast: &SourceFile, name: &str) -> Option<i128> {
    const_int(ast, const_expr(ast, name))
}

/// `K0 = K1, ..., K{n-1} = 4`: n sabit; `K0`'ın değer ifadesinden en
/// içteki literale n - 1 ad, literal `n` düzey derinde.
fn chain(n: usize) -> String {
    let mut src = String::new();
    for i in 0..n - 1 {
        src.push_str(&format!("const K{i} : u32 = K{}\n", i + 1));
    }
    src.push_str(&format!("const K{} : u32 = 4\n", n - 1));
    src
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
fn constants_fold_like_the_sv_generators_port_width() {
    // SV üreticisinin port genişliğini hesaplayan `Emitter::eval_const`'ın
    // anlamı: aritmetik taşma ve sıfıra bölme `None`; bit işlemi yalnız
    // negatif olmayan değerde; `<<` değer bitini kaybederse `None`, `>>`
    // 127 ve üstünde 0; mantıksal değer 0/1; `if`, `match` ve sabit dizi
    // indeksi katlanır.
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
const AND : u32 = 6 & 3
const OR : u32 = 4 | 1
const XOR : u32 = 6 ^ 3
const NEGAND : i32 = (0 - 1) & 3
const SHLLOST : u32 = 3 << 126
const SHRFAR : u32 = 1 >> 200
const BOOL : bool = true
const CMP : bool = ADD >= 4
const LOGIC : bool = (1 < 2) && !(3 == 3)
const IMP : bool = false -> false
const IF : u32 = if ADD > 3 { 8 } else { 2 }
const MATCH : u32 = match REM { 0 => 5, 1 | 2 => 9, _ => 0 }
const GUARD : u32 = match REM { 1 if true => 5, _ => 0 }
const UNKEY : u32 = match REM + Q { 1 => 5, _ => 0 }
const ARR : [u32; 3] = [4, 5, 6]
const AT : u32 = ARR[1]
const OUT : u32 = ARR[3]
const REP : [u32; 2] = [7; 2]
const ATREP : u32 = REP[1]
const ALIASED : [u32; 3] = ARR
const ATALIASED : u32 = ALIASED[2]
const HUGE : [u32; 5000] = [1; 5000]
const ATHUGE : u32 = HUGE[0]
const NOTARR : u32 = ADD[0]
const NE : bool = 3 != 4
const LT : bool = 1 < 2
const LTEQ : bool = 2 < 2
const GT : bool = 3 > 2
const GTEQ : bool = 2 > 2
const LE : bool = 2 <= 3
const LEGT : bool = 3 <= 2
const ORL : bool = true || false
const ORBITS : u32 = 5 | 1
const AND0L : u32 = 0 & 3
const AND0R : u32 = 3 & 0
const IFZERO : u32 = if 0 { 4 } else { 8 }
const IFARITH : u32 = if 0 + 1 { 4 } else { 8 }
const NA : u32 = [3, 4]
const ATNA : u32 = NA[1]
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
        ("AND", Some(2)),
        ("OR", Some(5)),
        ("XOR", Some(5)),
        ("NEGAND", None),
        ("SHLLOST", None),
        ("SHRFAR", Some(0)),
        ("BOOL", Some(1)),
        ("CMP", Some(1)),
        ("LOGIC", Some(0)),
        ("IMP", Some(1)),
        ("IF", Some(8)),
        ("MATCH", Some(9)),
        ("GUARD", None),
        ("UNKEY", None),
        ("AT", Some(5)),
        ("OUT", None),
        ("ATREP", Some(7)),
        ("ATALIASED", Some(6)),
        ("ATHUGE", None),
        ("NOTARR", None),
        ("NE", Some(1)),
        ("LT", Some(1)),
        ("LTEQ", Some(0)),
        ("GT", Some(1)),
        ("GTEQ", Some(0)),
        ("LE", Some(1)),
        ("LEGT", Some(0)),
        ("ORL", Some(1)),
        ("ORBITS", Some(5)),
        ("AND0L", Some(0)),
        ("AND0R", Some(0)),
        ("IFZERO", Some(8)),
        ("IFARITH", Some(4)),
        ("ATNA", None),
    ];
    for (name, want) in expect {
        assert_eq!(const_of(&ast, name), want, "{name}");
    }
    assert_eq!(MAX_TABLE_LEN, 4096);
}

#[test]
fn an_enum_constant_and_its_variants_are_match_keys() {
    let ast = parse(
        "\
enum Mode { Slow, Fast }
const M : Mode = Mode::Fast
const SPEED : u32 = match M { Mode::Slow => 1, Mode::Fast => 8 }
const DIRECT : u32 = match Mode::Slow { Mode::Slow => 3, _ => 4 }
const FLAG : u32 = match true { false => 6, _ => 2 }
",
    );
    assert_eq!(const_of(&ast, "SPEED"), Some(8));
    assert_eq!(const_of(&ast, "DIRECT"), Some(3));
    assert_eq!(const_of(&ast, "FLAG"), Some(2));
    // Sınanan ile desen farklı varyant: yalnız adı tutan varyant seçilir.
    let three = parse(
        "\
enum Three { A, B, C }
const WHICH : u32 = match Three::C { Three::B => 1, _ => 2 }
",
    );
    assert_eq!(const_of(&three, "WHICH"), Some(2));
}

#[test]
fn the_depth_limit_is_one_and_it_is_the_sv_generators_64() {
    // SV üreticisinin port genişliği 64 düzeye kadar iniyordu; tek sınır
    // bu (#376: test dili ve struct düzeni 32'de duruyordu, uzun zincirle
    // verilen genişlik SV'de 4 bit, test dilinde bilinmiyordu).
    assert_eq!(MAX_CONST_DEPTH, 64);
}

#[test]
fn constant_nesting_is_limited_to_the_depth_limit() {
    // `-(-( … 1 … ))`: her tekli eksi bir iç içelik.
    let nested = |n: u32| {
        let open = "-(".repeat(n as usize);
        let close = ")".repeat(n as usize);
        parse(&format!("const D : i32 = {open}1{close}\n"))
    };
    let at_limit = nested(MAX_CONST_DEPTH);
    assert_eq!(
        const_of(&at_limit, "D"),
        Some(1),
        "çift sayıda eksi: değer 1"
    );
    let past = nested(MAX_CONST_DEPTH + 1);
    assert_eq!(const_of(&past, "D"), None);
}

#[test]
fn the_reason_for_an_unknown_value_is_told_apart() {
    let limit = MAX_CONST_DEPTH as usize;
    let top = |ast: &SourceFile| eval(ast, &TopLevel(ast), const_expr(ast, "K0"));
    // `K0`'ın değer ifadesi kökte; en içteki literal `n - 1` düzey derinde.
    assert_eq!(top(&parse(&chain(limit + 1))), Ok(4));
    assert_eq!(top(&parse(&chain(limit + 2))), Err(Unknown::TooDeep));
    let other = parse("const S : u32 = 2\nconst X : u32 = S + Y\n");
    assert_eq!(
        eval(&other, &TopLevel(&other), const_expr(&other, "X")),
        Err(Unknown::NotConstant)
    );
}

#[test]
fn a_width_is_a_positive_constant_that_fits_u32() {
    let ast = parse(
        "\
const FOUR : u32 = 2 + 2
const ZERO : u32 = 0
const NEG : i32 = 0 - 3
const BIG : u64 = 1 << 40
const DEEP : u32 = -(-(-(-(4))))
",
    );
    let w = |name| width(&ast, &TopLevel(&ast), const_expr(&ast, name));
    assert_eq!(w("FOUR"), Ok(4));
    assert_eq!(w("ZERO"), Err(Unknown::NotConstant));
    assert_eq!(w("NEG"), Err(Unknown::NotConstant));
    assert_eq!(w("BIG"), Err(Unknown::NotConstant));
    assert_eq!(w("DEEP"), Ok(4));
    let deep = parse(&chain(MAX_CONST_DEPTH as usize + 2));
    assert_eq!(
        width(&deep, &TopLevel(&deep), const_expr(&deep, "K0")),
        Err(Unknown::TooDeep)
    );
}

#[test]
fn a_name_means_what_the_callers_scope_says() {
    // SV üreticisi döngü değişkenini ve gölgelemeyi kendi kapsamıyla
    // bildirir; üst düzey kapsam yalnız `const`'ları bilir.
    struct LoopI;
    impl Scope for LoopI {
        fn bind(&self, _: Idx<Expr>) -> Binding {
            Binding::Value(3)
        }
    }
    struct Nothing;
    impl Scope for Nothing {
        fn bind(&self, _: Idx<Expr>) -> Binding {
            Binding::Other
        }
    }
    let ast = parse("const K : u32 = 10\nconst X : u32 = i + 1\nconst Y : u32 = K\n");
    let x = const_expr(&ast, "X");
    assert_eq!(eval(&ast, &LoopI, x), Ok(4));
    assert_eq!(eval(&ast, &TopLevel(&ast), x), Err(Unknown::NotConstant));
    let y = const_expr(&ast, "Y");
    assert_eq!(eval(&ast, &TopLevel(&ast), y), Ok(10));
    assert_eq!(eval(&ast, &Nothing, y), Err(Unknown::NotConstant));
}

#[test]
fn a_port_width_given_by_a_long_const_chain_is_known_up_to_the_limit() {
    // #376: 33 ve 64 sabitlik zincirde SV `[3:0]` üretiyordu, test dili
    // genişliği bilmiyordu. 65'te SV üreticisi de durur (E2005).
    let port = |n: usize| {
        let src = format!(
            "{}module M {{\n    in a : uint<K0>\n    out q : uint<K0>\n    q = a\n}}\n",
            chain(n)
        );
        let ast = parse(&src);
        test_port_width(&[&ast], "M", "a").map(|w| w.bits)
    };
    assert_eq!(port(33), Some(4));
    assert_eq!(port(64), Some(4));
    assert_eq!(port(65), None);
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

/// `X`'in değeri; `src`'deki ek bildirimlerle.
fn x_of(src: &str) -> Result<i128, Unknown> {
    let ast = parse(src);
    eval(&ast, &TopLevel(&ast), const_expr(&ast, "X"))
}

/// `P0 = P1, ..., P{n} = tail`: `P0`'ın değer ifadesi kökte, `tail` n
/// düzey derinde (her ad bir düzey).
fn names_to(prefix: &str, n: usize, tail: &str) -> String {
    let mut src = String::new();
    for i in 0..n {
        src.push_str(&format!("const {prefix}{i} : u32 = {prefix}{}\n", i + 1));
    }
    src.push_str(&format!("const {prefix}{n} : u32 = {tail}\n"));
    src
}

/// `open` n kez, ortada `inner`, sonra `close` n kez.
fn nest(n: usize, open: &str, inner: &str, close: &str) -> String {
    format!("{}{inner}{}", open.repeat(n), close.repeat(n))
}

#[test]
fn every_path_into_a_value_counts_toward_the_depth_limit() {
    let max = MAX_CONST_DEPTH as usize;
    // Her biçimde en içteki değer n düzey derinde: 64 tamam, 65 aşar.
    let forms: [(&str, &str, &str, &str, i128); 8] = [
        // `if`in seçilen kolu
        ("taken", "if true { ", "4", " } else { 0 }", 4),
        // dizi indeksinin indeksi
        ("index", "ARR[", "0", "]", 0),
        // `!`
        ("not", "!", "true", "", 1),
        // `&&`'in sol ve sağ operandı
        ("and-left", "(", "true", " && true)", 1),
        ("and-right", "(true && ", "true", ")", 1),
        // karşılaştırmanın sol ve sağ operandı
        ("cmp-left", "(", "1", " == 1)", 1),
        ("cmp-right", "(1 == ", "1", ")", 1),
        // `match` gövdesi
        ("match-body", "match 0 { _ => ", "4", " }", 4),
    ];
    for (name, open, inner, close, value) in forms {
        let src = |n: usize| {
            let expr = nest(n, open, inner, close);
            format!("const ARR : [u32; 2] = [0, 0]\nconst X : u32 = {expr}\n")
        };
        let ok = if name == "index" { max - 2 } else { max };
        assert_eq!(x_of(&src(ok)), Ok(value), "{name}: {ok}");
        assert_eq!(
            x_of(&src(max + 1)),
            Err(Unknown::TooDeep),
            "{name}: {}",
            max + 1
        );
    }
}

#[test]
fn a_condition_counts_twice_per_level() {
    // E0 = true, E{k+1} = if E{k} { 1 } else { 0 }: koşul bool'a (bir
    // düzey), bool değil `if` olduğundan sayıya (bir düzey daha) iner.
    let e = |n: usize| {
        let mut x = "true".to_string();
        for _ in 0..n {
            x = format!("if ({x}) {{ 1 }} else {{ 0 }}");
        }
        format!("const X : u32 = {x}\n")
    };
    assert_eq!(x_of(&e(32)), Ok(1));
    assert_eq!(x_of(&e(33)), Err(Unknown::TooDeep));
}

#[test]
fn a_pattern_name_and_an_enum_value_count_toward_the_depth_limit() {
    // Desendeki ad: `match`'in deseni bir düzey, adın değeri bir düzey daha.
    let pat = |n: usize| {
        format!(
            "{}const X : u32 = match 1 {{ P0 => 5, _ => 7 }}\n",
            names_to("P", n, "1")
        )
    };
    assert_eq!(x_of(&pat(62)), Ok(5));
    assert_eq!(x_of(&pat(63)), Err(Unknown::TooDeep));
    // Enum'un açık değeri: sınanan bir düzey, düzen tablosu bir düzey daha.
    let en = |n: usize| {
        format!(
            "{}enum V : u8 {{ A = Q0, B = 9 }}\nconst X : u32 = match V::A {{ V::A => 1, _ => 0 }}\n",
            names_to("Q", n, "3")
        )
    };
    assert_eq!(x_of(&en(61)), Ok(1));
    assert_eq!(x_of(&en(62)), Err(Unknown::TooDeep));
}

#[test]
fn a_constant_array_element_counts_toward_the_depth_limit() {
    // `K{d}` = `ARR[i]`: indeks ifadesi d düzey derinde; eleman, dizinin
    // değeri ve elemanın kendisi üç düzey daha.
    let at = |d: usize, tail: &str| {
        let src = format!(
            "const ARR : [u32; 2] = [4, 5]\nconst VV : u32 = 7\nconst CC : u32 = 2\n\
             const REP : [u32; 2] = [7; 2]\nconst REPV : [u32; 2] = [VV; 2]\n\
             const REPC : [u32; 2] = [7; CC]\nconst ALIASED : [u32; 2] = ARR\n{}",
            names_to("K", d, tail)
        );
        let ast = parse(&src);
        eval(&ast, &TopLevel(&ast), const_expr(&ast, "K0"))
    };
    // Liste ve `[v; N]`: eleman d + 3'te.
    assert_eq!(at(61, "ARR[0]"), Ok(4));
    assert_eq!(at(62, "ARR[0]"), Err(Unknown::TooDeep));
    assert_eq!(at(61, "REP[1]"), Ok(7));
    assert_eq!(at(62, "REP[1]"), Err(Unknown::TooDeep));
    // `[VV; 2]` ve `[7; CC]`: değer ya da sayı bir ad daha derinde.
    assert_eq!(at(60, "REPV[1]"), Ok(7));
    assert_eq!(at(61, "REPV[1]"), Err(Unknown::TooDeep));
    assert_eq!(at(60, "REPC[1]"), Ok(7));
    assert_eq!(at(61, "REPC[1]"), Err(Unknown::TooDeep));
    // Başka bir dizi sabitinin adı: bir düzey daha.
    assert_eq!(at(60, "ALIASED[1]"), Ok(5));
    assert_eq!(at(61, "ALIASED[1]"), Err(Unknown::TooDeep));
}
