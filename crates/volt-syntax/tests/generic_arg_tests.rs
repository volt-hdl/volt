//! Generic argümanda negatif değer ve sabit ifade (#471, ADR-0041).
//!
//! Gramer argümanı bir ifade sayar (grammar-full.ebnf: `GenericArg = Type |
//! Expr`) ve argüman bir derleme zamanı sabitidir (ADR-0041). Önce örnekleme
//! ileri bakışı `<` ile `>` arasında yalnız ad, literal ve virgül kabul
//! ediyordu: `M<-1>`, `M<(-1)>`, `M<0 - 1>` ifade yoluna düşüp E0010 alıyordu
//! (`M < -1 > { ... }` bir karşılaştırma zinciri sayılıyordu); adlı bir sabit
//! (`M<NEG>`) monomorfizasyonda E2008 alıyordu, çünkü yalnız literal kabul
//! ediliyordu. Beklenenler elle: dört biçim de -1'dir ve tek monomorfu
//! adlandırır; adı negatif değer için `n` ve değerin büyüklüğüdür
//! (`Scaled<-1>` → `Scaled_n1`). Gerçek karşılaştırma zinciri E0010 kalır
//! (operator-precedence.md §2.1).

use volt_ast::{ExprKind, ItemKind, SourceFile, StmtKind, UnOp};
use volt_span::FileId;
use volt_syntax::parser::{parse, ParseResult};

fn p(src: &str) -> ParseResult {
    parse(FileId(0), src)
}

fn module_names(ast: &SourceFile) -> Vec<String> {
    ast.items
        .iter()
        .filter_map(|&i| match &ast.items_arena[i].kind {
            ItemKind::Module(m) => Some(m.name.text.clone()),
            _ => None,
        })
        .collect()
}

/// `module_name` modülündeki örneklerin (ad, hedef modül), sırayla.
fn instances(ast: &SourceFile, module_name: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for &i in &ast.items {
        let ItemKind::Module(m) = &ast.items_arena[i].kind else {
            continue;
        };
        if m.name.text != module_name {
            continue;
        }
        for &s in &m.body {
            if let StmtKind::Instance(inst) = &ast.stmts[s].kind {
                let target = inst
                    .module_path
                    .segments
                    .iter()
                    .map(|n| n.text.as_str())
                    .collect::<Vec<_>>()
                    .join("::");
                out.push((inst.name.text.clone(), target));
            }
        }
    }
    out
}

/// `module_name` monomorfundaki `y = ...` atamasının sağ tarafı.
fn assigned_value(ast: &SourceFile, module_name: &str) -> ExprKind {
    for &i in &ast.items {
        let ItemKind::Module(m) = &ast.items_arena[i].kind else {
            continue;
        };
        if m.name.text != module_name {
            continue;
        }
        for &s in &m.body {
            if let StmtKind::Assign(a) = &ast.stmts[s].kind {
                return ast.exprs[a.rhs].kind.clone();
            }
        }
    }
    panic!("{module_name} içinde atama yok");
}

fn scaled(args: &[&str]) -> String {
    let lets: String = args
        .iter()
        .enumerate()
        .map(|(k, a)| format!("    let s{k} = Scaled<{a}> {{ x: x }}\n    o{k} = s{k}.y\n"))
        .collect();
    let outs: String = (0..args.len())
        .map(|k| format!("    out o{k} : Trit\n"))
        .collect();
    format!(
        "const NEG : Trit = -1
const ONE : Trit = 1

module Scaled<const S : Trit> {{
    in  x : i8
    out y : Trit
    y = S
}}

module Top {{
    in  x : i8
{outs}{lets}}}
"
    )
}

/// `-1`, `(-1)`, `0 - 1` ve adlı sabit `NEG` aynı değerdir: tanı yok, tek
/// monomorf `Scaled_n1`, dört örnek de ona bağlı. Önce E0010 + E0001 (ilk
/// üçü) ve E2008 (adlı sabit).
#[test]
fn minus_one_in_every_form_names_one_monomorph() {
    let r = p(&scaled(&["-1", "(-1)", "0 - 1", "NEG"]));
    assert_eq!(r.error_codes(), Vec::<&str>::new());
    assert_eq!(module_names(&r.ast), ["Scaled_n1", "Top"]);
    let targets: Vec<String> = instances(&r.ast, "Top")
        .into_iter()
        .map(|(_, t)| t)
        .collect();
    assert_eq!(targets, ["Scaled_n1"; 4]);
    // Gövdede S'nin yerinde -1: `-` ve büyüklüğü 1 olan literal.
    match assigned_value(&r.ast, "Scaled_n1") {
        ExprKind::Unary {
            op: UnOp::Neg,
            operand,
        } => assert!(
            matches!(r.ast.exprs[operand].kind, ExprKind::IntLit { value: 1, .. }),
            "{:?}",
            r.ast.exprs[operand].kind
        ),
        other => panic!("-1 bekleniyor: {other:?}"),
    }
}

/// Pozitif biçimler ve adlı sabit de değeriyle ayrışır: `(1)`, `1 - 0`,
/// `ONE`, `(ONE)`, `0 + 1`, `2 / 2` → `Scaled_1`; `1 - 1`, `ONE - 1`, `2 %
/// 2` → `Scaled_0`; -1 ile +1 iki ayrı monomorftur.
#[test]
fn constant_expressions_are_evaluated() {
    let r = p(&scaled(&[
        "(1)",
        "1 - 0",
        "ONE",
        "(ONE)",
        "0 + 1",
        "2 / 2",
        "1 - 1",
        "ONE - 1",
        "2 % 2",
        "-1",
        "2 * 0 - 1",
    ]));
    assert_eq!(r.error_codes(), Vec::<&str>::new());
    assert_eq!(
        module_names(&r.ast),
        ["Scaled_1", "Scaled_0", "Scaled_n1", "Top"]
    );
    let targets: Vec<String> = instances(&r.ast, "Top")
        .into_iter()
        .map(|(_, t)| t)
        .collect();
    assert_eq!(
        targets,
        [
            "Scaled_1",
            "Scaled_1",
            "Scaled_1",
            "Scaled_1",
            "Scaled_1",
            "Scaled_1",
            "Scaled_0",
            "Scaled_0",
            "Scaled_0",
            "Scaled_n1",
            "Scaled_n1"
        ]
    );
}

fn signed(arg: &str, ty: &str) -> String {
    format!(
        "module P<const K : {ty}> {{
    out y : {ty}
    y = K
}}

module Top {{
    out y : {ty}
    let p = P<{arg}> {{ }}
    y = p.y
}}
"
    )
}

/// İşaretli parametre (#262'nin aralığı alt uçla): i8'e -1 ve -128 sığar,
/// -129 sığmaz (E2010); işaretsiz parametreye negatif argüman sığmaz.
#[test]
fn a_negative_argument_is_checked_against_the_parameter_type() {
    for (arg, ty, codes, name) in [
        ("-1", "i8", vec![], Some("P_n1")),
        ("-128", "i8", vec![], Some("P_n128")),
        ("(-128)", "i8", vec![], Some("P_n128")),
        ("-129", "i8", vec!["E2010"], None),
        ("127", "i8", vec![], Some("P_127")),
        ("128", "i8", vec!["E2010"], None),
        ("-1", "u8", vec!["E2010"], None),
        ("0 - 1", "u8", vec!["E2010"], None),
        // 129 bit: alt sınırın büyüklüğü 2^128, u128'e sığmaz; her negatif
        // `i128` değeri sığar.
        ("-1", "i129", vec![], Some("P_n1")),
    ] {
        let r = p(&signed(arg, ty));
        assert_eq!(r.error_codes(), codes, "P<{arg}> : {ty}");
        if let Some(name) = name {
            assert_eq!(module_names(&r.ast), [name, "Top"], "P<{arg}> : {ty}");
        }
    }
}

/// E2010'un iletisi alt sınırı söyler (ADR-0125: ne oldu, ne yapılabilir).
#[test]
fn the_out_of_range_message_names_the_minimum() {
    let r = p(&signed("-129", "i8"));
    let d = &r.diagnostics[0];
    assert_eq!(
        d.message,
        "generic argument -129 does not fit in the parameter K : i8 of P (minimum -128)"
    );
    assert_eq!(
        d.help.as_deref(),
        Some("use a value from -128 to 127, or declare K with a wider type")
    );
    let r = p(&signed("-1", "u8"));
    assert_eq!(
        r.diagnostics[0].message,
        "generic argument -1 does not fit in the parameter K : u8 of P (minimum 0)"
    );
    assert_eq!(
        r.diagnostics[0].help.as_deref(),
        Some("use a value from 0 to 255, or declare K with a signed type")
    );
}

/// Genişliği önceki bir parametre olan parametre (#262): genişlik negatifse
/// aralığı bilinmez, ayrıştırıcı denetlemez (genişliği tip denetimi
/// reddeder). `W = 2` için K = 3 sığar, K = 4 E2010.
#[test]
fn a_width_from_a_negative_argument_is_not_checked_here() {
    let src = |args: &str| {
        format!(
            "module Q<const W : i8, const K : uint<W>> {{
    out y : bool
    y = true
}}

module Top {{
    out y : bool
    let q = Q<{args}> {{ }}
    y = q.y
}}
"
        )
    };
    assert_eq!(p(&src("-1, 4")).error_codes(), Vec::<&str>::new());
    assert_eq!(p(&src("2, 3")).error_codes(), Vec::<&str>::new());
    assert_eq!(p(&src("2, 4")).error_codes(), vec!["E2010"]);
}

/// Adlı argüman ancak bir kök const'a bağlanıyorsa sabittir: aynı adlı port
/// onu gölgeler (name-resolution.md §4), sinyal sabit değildir: E2008.
/// Tanımsız ad da E2008 kalır.
#[test]
fn a_name_that_is_not_a_const_is_still_e2008() {
    let shadowed = "const NEG : Trit = -1

module Scaled<const S : Trit> {
    out y : Trit
    y = S
}

module Top {
    in  NEG : Trit
    out y   : Trit
    let s = Scaled<NEG> { }
    y = s.y
}
";
    assert_eq!(p(shadowed).error_codes(), vec!["E2008"]);
    // Modül düzeyi bir bildirim de (`let`) kök const'u gölgeler.
    let let_shadowed = shadowed.replace("in  NEG : Trit", "in  q : Trit\n    let NEG = q");
    assert_eq!(p(&let_shadowed).error_codes(), vec!["E2008"]);
    let undefined = shadowed.replace("in  NEG : Trit", "in  other : Trit");
    let undefined = undefined.replace("const NEG : Trit = -1", "");
    assert_eq!(p(&undefined).error_codes(), vec!["E2008"]);
    let d = &p(&undefined).diagnostics[0];
    assert_eq!(d.message, "Scaled S must be a compile-time constant");
}

/// İç içe örnekleme: `A<-1>`'in gövdesindeki `B<S>` ikameden sonra `B<-1>`
/// olur ve `B_n1`'e açılır.
#[test]
fn a_negative_argument_reaches_a_nested_instance() {
    let src = "module B<const S : Trit> {
    out y : Trit
    y = S
}

module A<const S : Trit> {
    out y : Trit
    let b = B<S> { }
    y = b.y
}

module Top {
    out y : Trit
    let a = A<-1> { }
    y = a.y
}
";
    let r = p(src);
    assert_eq!(r.error_codes(), Vec::<&str>::new());
    assert_eq!(module_names(&r.ast), ["B_n1", "A_n1", "Top"]);
}

/// Gerçek karşılaştırma zinciri E0010 kalır: `>`'dan sonra `{` gelmeyen
/// `a < b > c` ve `a < -1 > b` (operator-precedence.md §2.1).
#[test]
fn a_real_comparison_chain_is_still_e0010() {
    for chain in ["a < b > c", "a < -1 > b", "a < (b - 1) > c", "a < 1 > b"] {
        let src = format!(
            "module M {{
    in  a : i8
    in  b : i8
    in  c : i8
    out y : bool
    let v = {chain}
    y = v
}}
"
        );
        let r = p(&src);
        assert!(
            r.error_codes().contains(&"E0010"),
            "{chain}: {:?}",
            r.error_codes()
        );
    }
}
