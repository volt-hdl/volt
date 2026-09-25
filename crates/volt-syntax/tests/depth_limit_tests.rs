//! Ağaç derinliği sınırı (ADR-0080): parser `MAX_DEPTH`'ten derin ağaç
//! üretmez. Sonraki her geçit AST'yi özyinelemeyle yürüdüğünden bu tek
//! nokta, derleyicinin derin girdide yığını taşırmamasının (abort) güvencesidir.
//!
//! Her yapı için: derin girdi → TEK E0018, sınırlı ağaç, < 1 s; sınırın
//! hemen altındaki girdi → E0018 yok. Uçtan uca (tüm geçitler, ayrı
//! süreçte, abort yakalanır) denetim `volt-driver/tests/depth_limit_tests.rs`.

use std::collections::HashMap;
use std::sync::mpsc;
use std::time::Duration;

use volt_ast::{
    ArrayLitKind, Block, BlockStmt, ElseBranch, Expr, ExprKind, GenericArg, Idx, IfStmt,
    LValueSuffix, MatchArm, MatchArmBody, Pattern, PatternArgs, PatternKind, SourceFile, Stmt,
    StmtKind, TypeRef, TypeRefKind,
};
use volt_span::FileId;
use volt_syntax::parser::{parse, ParseResult};
use volt_syntax::MAX_DEPTH;

const M: usize = MAX_DEPTH as usize;

// ═══ Ağaç yüksekliği (yinelemeli; testin kendisi de taşmamalı) ═══════

#[derive(Clone, Copy)]
enum Node<'a> {
    Expr(Idx<Expr>),
    Block(Idx<Block>),
    Type(Idx<TypeRef>),
    Pat(Idx<Pattern>),
    Stmt(Idx<Stmt>),
    If(&'a IfStmt),
    Arm(&'a MatchArm),
    BlockStmt(&'a BlockStmt),
}

/// Bellek anahtarı: arena düğümü kendi indeksiyle, kutulu/iç düğüm adresiyle.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Key {
    Expr(Idx<Expr>),
    Block(Idx<Block>),
    Type(Idx<TypeRef>),
    Pat(Idx<Pattern>),
    Stmt(Idx<Stmt>),
    Inner(u8, usize),
}

impl Node<'_> {
    fn key(&self) -> Key {
        match *self {
            Node::Expr(i) => Key::Expr(i),
            Node::Block(i) => Key::Block(i),
            Node::Type(i) => Key::Type(i),
            Node::Pat(i) => Key::Pat(i),
            Node::Stmt(i) => Key::Stmt(i),
            Node::If(r) => Key::Inner(0, r as *const IfStmt as usize),
            Node::Arm(r) => Key::Inner(1, r as *const MatchArm as usize),
            Node::BlockStmt(r) => Key::Inner(2, r as *const BlockStmt as usize),
        }
    }
}

fn suffix_exprs(s: &LValueSuffix) -> Vec<Idx<Expr>> {
    match s {
        LValueSuffix::Index(e) => vec![*e],
        LValueSuffix::Range { hi, lo } => vec![*hi, *lo],
        LValueSuffix::PartSelect { start, width, .. } => vec![*start, *width],
        LValueSuffix::Field(_) => vec![],
    }
}

fn children<'a>(ast: &'a SourceFile, n: Node<'a>) -> Vec<Node<'a>> {
    let e = Node::Expr;
    match n {
        Node::Expr(i) => match &ast.exprs[i].kind {
            ExprKind::Binary { lhs, rhs, .. } => vec![e(*lhs), e(*rhs)],
            ExprKind::Unary { operand, .. } => vec![e(*operand)],
            ExprKind::Index { base, index } => vec![e(*base), e(*index)],
            ExprKind::Range { base, hi, lo } => vec![e(*base), e(*hi), e(*lo)],
            ExprKind::PartSelect {
                base, start, width, ..
            } => vec![e(*base), e(*start), e(*width)],
            ExprKind::Field { base, .. } => vec![e(*base)],
            ExprKind::Call { callee, args } => std::iter::once(e(*callee))
                .chain(args.iter().map(|a| e(*a)))
                .collect(),
            ExprKind::Cast { expr, ty } => vec![e(*expr), Node::Type(*ty)],
            ExprKind::If {
                cond,
                then_expr,
                else_expr,
            } => vec![e(*cond), e(*then_expr), e(*else_expr)],
            ExprKind::Match { scrutinee, arms } => std::iter::once(e(*scrutinee))
                .chain(arms.iter().map(Node::Arm))
                .collect(),
            ExprKind::StructLit { fields, .. } => {
                fields.iter().filter_map(|f| f.value).map(e).collect()
            }
            ExprKind::ArrayLit(ArrayLitKind::List(xs)) | ExprKind::TupleLit(xs) => {
                xs.iter().map(|x| e(*x)).collect()
            }
            ExprKind::ArrayLit(ArrayLitKind::Repeat { value, count }) => {
                vec![e(*value), e(*count)]
            }
            ExprKind::Concat(parts) => parts
                .iter()
                .flat_map(|(x, t)| [e(*x), Node::Type(*t)])
                .collect(),
            _ => vec![],
        },
        Node::Block(i) => {
            let b = &ast.blocks[i];
            b.stmts
                .iter()
                .map(Node::BlockStmt)
                .chain(b.tail.map(e))
                .collect()
        }
        Node::BlockStmt(s) => match s {
            BlockStmt::NonBlockAssign { lhs, rhs, .. }
            | BlockStmt::BlockAssign { lhs, rhs, .. } => lhs
                .suffixes
                .iter()
                .flat_map(suffix_exprs)
                .map(e)
                .chain([e(*rhs)])
                .collect(),
            BlockStmt::If(s) => vec![Node::If(s)],
            BlockStmt::Match(m) => std::iter::once(e(m.scrutinee))
                .chain(m.arms.iter().map(Node::Arm))
                .collect(),
            BlockStmt::Let(l) => {
                l.ty.map(Node::Type)
                    .into_iter()
                    .chain([e(l.value)])
                    .collect()
            }
            BlockStmt::For(f) => vec![e(f.start), e(f.end), Node::Block(f.body)],
            BlockStmt::Error => vec![],
        },
        Node::If(s) => {
            let mut out = vec![e(s.cond), Node::Block(s.then_block)];
            match &s.else_branch {
                Some(ElseBranch::Block(b)) => out.push(Node::Block(*b)),
                Some(ElseBranch::If(inner)) => out.push(Node::If(inner)),
                None => {}
            }
            out
        }
        Node::Arm(a) => {
            let mut out = vec![Node::Pat(a.pattern)];
            out.extend(a.guard.map(e));
            out.push(match a.body {
                MatchArmBody::Block(b) => Node::Block(b),
                MatchArmBody::Expr(x) => e(x),
            });
            out
        }
        Node::Type(i) => match &ast.types[i].kind {
            TypeRefKind::Bits(x) | TypeRefKind::UIntN(x) | TypeRefKind::SIntN(x) => vec![e(*x)],
            TypeRefKind::Array { elem, len } => vec![Node::Type(*elem), e(*len)],
            TypeRefKind::Tuple(ts) => ts.iter().map(|t| Node::Type(*t)).collect(),
            TypeRefKind::Path { args, .. } => args
                .iter()
                .map(|a| match a {
                    GenericArg::Type(t) => Node::Type(*t),
                    GenericArg::Const(x) => e(*x),
                })
                .collect(),
            _ => vec![],
        },
        Node::Pat(i) => match &ast.patterns[i].kind {
            PatternKind::Literal(x) => vec![e(*x)],
            PatternKind::Tuple(ps) | PatternKind::Or(ps) => {
                ps.iter().map(|p| Node::Pat(*p)).collect()
            }
            PatternKind::Path {
                args: Some(PatternArgs::Tuple(ps)),
                ..
            } => ps.iter().map(|p| Node::Pat(*p)).collect(),
            PatternKind::Path {
                args: Some(PatternArgs::Struct(fs)),
                ..
            } => fs.iter().filter_map(|f| f.pattern).map(Node::Pat).collect(),
            _ => vec![],
        },
        Node::Stmt(i) => match &ast.stmts[i].kind {
            StmtKind::Reg(r) => {
                r.ty.map(Node::Type)
                    .into_iter()
                    .chain([e(r.init)])
                    .collect()
            }
            StmtKind::Let(l) => {
                l.ty.map(Node::Type)
                    .into_iter()
                    .chain([e(l.value)])
                    .collect()
            }
            StmtKind::Wire(w) => vec![Node::Type(w.ty)],
            StmtKind::Instance(inst) => inst
                .bindings
                .iter()
                .filter_map(|b| b.value)
                .map(e)
                .collect(),
            StmtKind::On(on) => vec![Node::Block(on.body)],
            StmtKind::Comb(b) => vec![Node::Block(*b)],
            StmtKind::Assign(a) => a
                .lhs
                .suffixes
                .iter()
                .flat_map(suffix_exprs)
                .map(e)
                .chain([e(a.rhs)])
                .collect(),
            StmtKind::For(f) => vec![e(f.start), e(f.end), Node::Block(f.body)],
            StmtKind::Expr(x) => vec![e(*x)],
            StmtKind::Error => vec![],
        },
    }
}

/// Tüm arenalardaki en yüksek ağaç (düğüm sayısıyla yükseklik).
fn max_tree_height(ast: &SourceFile) -> usize {
    let mut memo: HashMap<Key, usize> = HashMap::new();
    let roots = ast
        .stmts
        .iter_idx()
        .map(|(i, _)| Node::Stmt(i))
        .chain(ast.exprs.iter_idx().map(|(i, _)| Node::Expr(i)))
        .chain(ast.blocks.iter_idx().map(|(i, _)| Node::Block(i)))
        .chain(ast.types.iter_idx().map(|(i, _)| Node::Type(i)))
        .chain(ast.patterns.iter_idx().map(|(i, _)| Node::Pat(i)));
    let mut max = 0;
    for root in roots {
        let mut stack = vec![(root, false)];
        while let Some((n, expanded)) = stack.pop() {
            if memo.contains_key(&n.key()) {
                continue;
            }
            let kids = children(ast, n);
            if expanded {
                let h = 1 + kids.iter().map(|k| memo[&k.key()]).max().unwrap_or(0);
                memo.insert(n.key(), h);
                max = max.max(h);
            } else {
                stack.push((n, true));
                stack.extend(kids.into_iter().map(|k| (k, false)));
            }
        }
    }
    max
}

// ═══ Yardımcılar ══════════════════════════════════════════════════════

/// Blok ↔ `if`/`match` dönüşümünde her iç içelik katı parser sayacında bir,
/// ağaçta en çok üç düğümdür (blok + deyim + başlık).
const HEIGHT_BOUND: usize = 3 * M + 16;

fn parse_timed(src: String) -> ParseResult {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(parse(FileId(0), &src));
    });
    rx.recv_timeout(Duration::from_secs(1))
        .expect("1 s içinde ayrışmalı (panik ya da takılma)")
}

/// Derin girdi: tek E0018, başka tanı yok, ağaç sınırlı.
fn assert_cut(name: &str, src: String) {
    let res = parse_timed(src);
    assert_eq!(res.error_codes(), ["E0018"], "{name}");
    let h = max_tree_height(&res.ast);
    assert!(
        h <= HEIGHT_BOUND,
        "{name}: ağaç yüksekliği {h} > {HEIGHT_BOUND}"
    );
}

/// Sınırın hemen altı: E0018 yok.
fn assert_accepted(name: &str, src: String) {
    let res = parse_timed(src);
    assert!(
        !res.error_codes().contains(&"E0018"),
        "{name}: {:?}",
        res.error_codes()
    );
}

fn module(e: &str) -> String {
    format!("module Top {{\n    in a : u8\n    out y : u8\n    y = {e}\n}}\n")
}

fn generic(e: &str) -> String {
    format!(
        "module G<const N: u32> {{\n    in a : u8\n    out y : u8\n    y = {e}\n}}\n\
         module Top {{\n    in a : u8\n    out y : u8\n    let g = G<1> {{ a }}\n    y = g.y\n}}\n"
    )
}

fn seq(body: &str) -> String {
    format!(
        "module Top {{\n    in clk : clock\n    in a : u8\n    out y : u8\n    reg r : u8 = 0\n    on clk {{\n{body}\n    }}\n    y = r\n}}\n"
    )
}

fn chain(n: usize) -> String {
    vec!["a"; n].join(" + ")
}

/// (ad, derinlik → kaynak): her yapı hem derin hem sınır altı sınanır.
type Case = (&'static str, fn(usize) -> String);

const CASES: &[Case] = &[
    ("sol-derin zincir", |n| module(&chain(n))),
    ("generic şablonda zincir", |n| generic(&chain(n))),
    ("parantez", |n| {
        module(&format!("{}a{}", "(".repeat(n), ")".repeat(n)))
    }),
    ("tekli işleç", |n| module(&format!("{}a", "~".repeat(n)))),
    ("as zinciri", |n| {
        module(&format!("a{}", " as u8".repeat(n)))
    }),
    ("alan zinciri", |n| module(&format!("a{}", ".b".repeat(n)))),
    ("indeks", |n| {
        module(&format!(
            "if {}0{} {{ a }} else {{ a }}",
            "a[".repeat(n),
            "]".repeat(n)
        ))
    }),
    ("if ifadesi", |n| {
        module(&format!(
            "{}a{}",
            "if a == 0 { ".repeat(n),
            " } else { a }".repeat(n)
        ))
    }),
    ("else if ifadesi", |n| {
        module(&format!("{}a", "if a == 0 { a } else ".repeat(n)).replace("else a", "else { a }"))
    }),
    ("else if deyimi", |n| {
        seq(&format!(
            "if a == 0 {{ r <= 0 }}{}",
            " else if a == 1 { r <= 1 }".repeat(n)
        ))
    }),
    ("iç içe blok", |n| {
        seq(&format!(
            "{}r <= a{}",
            "if a != 0 { ".repeat(n),
            " }".repeat(n)
        ))
    }),
    ("iç içe match", |n| {
        seq(&format!(
            "{}r <= a{}",
            "match a { 0 => { ".repeat(n),
            " } _ => { r <= 0 } }".repeat(n)
        ))
    }),
    ("desen", |n| {
        seq(&format!(
            "match a {{ {}0{} => {{ r <= a }} _ => {{ r <= 0 }} }}",
            "(".repeat(n),
            ")".repeat(n)
        ))
    }),
    // İnceleme bulguları: çift E0018 (parser + tip çizgesi), `<`'de kaskad,
    // kesilen `for` sınırında E2021 kaskadı.
    ("takma adda derin dizi tipi", |n| {
        format!(
            "type T = {}u8{}
module Top {{
    in a : u8
    out y : u8
    y = a
}}
",
            "[".repeat(n),
            "; 1]".repeat(n)
        )
    }),
    ("generic argüman zinciri", |n| {
        format!(
            "type T = {}u8{}
module Top {{
    in a : u8
    out y : u8
    y = a
}}
",
            "Foo<".repeat(n),
            ">".repeat(n)
        )
    }),
    ("iç içe for", |n| {
        seq(&format!(
            "{}r <= a{}",
            "for i in 0..1 { ".repeat(n),
            " }".repeat(n)
        ))
    }),
    ("dizi tipi", |n| {
        format!(
            "module Top {{\n    in a : u8\n    out y : u8\n    wire w : {}u8{}\n    y = a\n}}\n",
            "[".repeat(n),
            "; 1]".repeat(n)
        )
    }),
];

// ═══ Testler ══════════════════════════════════════════════════════════

#[test]
fn every_deep_construct_is_cut_with_one_e0018_and_a_bounded_tree() {
    for &(name, gen) in CASES {
        assert_cut(name, gen(20_000));
    }
}

#[test]
fn every_construct_just_below_the_limit_is_accepted() {
    // İfade bir deyimin içinde birkaç kat aşağıda başlar; blok tabanlı
    // yapılar kat başına blok + ifade harcar — yarısı her yapı için güvenli.
    for &(name, gen) in CASES {
        assert_accepted(name, gen(M / 2 - 8));
    }
}

#[test]
fn a_flat_chain_up_to_the_limit_is_one_tree_level_per_link() {
    assert_accepted("zincir", module(&chain(M - 8)));
    let res = parse_timed(module(&chain(M + 8)));
    assert_eq!(res.error_codes(), ["E0018"]);
}

#[test]
fn e0018_is_reported_once_per_file_even_with_several_deep_sites() {
    let src = format!(
        "module A {{\n    in a : u8\n    out y : u8\n    y = {}\n}}\nmodule B {{\n    in a : u8\n    out y : u8\n    y = {}\n}}\n",
        chain(5_000),
        chain(5_000)
    );
    let res = parse_timed(src);
    assert_eq!(res.error_codes(), ["E0018"]);
}

#[test]
fn cut_chain_keeps_the_following_items_parsed() {
    let src = format!(
        "module A {{\n    in a : u8\n    out y : u8\n    y = {}\n}}\nmodule B {{\n    in a : u8\n    out y : u8\n    y = a\n}}\n",
        chain(5_000)
    );
    let res = parse_timed(src);
    assert_eq!(res.error_codes(), ["E0018"]);
    assert!(
        res.ast.module(1).is_some(),
        "kesilen zincirden sonraki modül kaybolmamalı"
    );
}

#[test]
fn else_if_chain_below_the_limit_keeps_its_nested_structure() {
    let src = seq("if a == 0 { r <= 0 } else if a == 1 { r <= 1 } else { r <= 2 }");
    let res = parse_timed(src);
    assert!(res.diagnostics.is_empty(), "{:?}", res.error_codes());
    let ifs: Vec<&BlockStmt> = res
        .ast
        .blocks
        .iter()
        .flat_map(|b| &b.stmts)
        .filter(|s| matches!(s, BlockStmt::If(_)))
        .collect();
    let [BlockStmt::If(outer)] = ifs.as_slice() else {
        panic!("tek dış if bekleniyor");
    };
    let Some(ElseBranch::If(inner)) = &outer.else_branch else {
        panic!("else if iç IfStmt olmalı");
    };
    assert!(matches!(inner.else_branch, Some(ElseBranch::Block(_))));
    assert!(inner.span.start > outer.span.start && inner.span.end == outer.span.end);
}

#[test]
fn type_alias_chain_past_the_limit_is_one_e0018_on_the_crossing_alias() {
    let n = 3_000;
    let mut src = String::from("type T0 = u8\n");
    for i in 1..n {
        src.push_str(&format!("type T{i} = T{}\n", i - 1));
    }
    src.push_str(&format!(
        "module Top {{\n    in a : T{}\n    out y : u8\n    y = a\n}}\n",
        n - 1
    ));
    let res = parse_timed(src);
    assert_eq!(res.error_codes(), ["E0018"]);
    // Takma ad başına iki kat (bildirim + hedef tip): T0 = 2, Tk = 2k + 2.
    let crossing = M / 2;
    assert!(
        res.diagnostics[0]
            .message
            .contains(&format!("'T{crossing}'")),
        "{}",
        res.diagnostics[0].message
    );
}

#[test]
fn short_alias_chains_are_accepted() {
    let mut src = String::from("type T0 = u8\n");
    for i in 1..64 {
        src.push_str(&format!("type T{i} = [T{}; 1]\n", i - 1));
    }
    src.push_str("module Top {\n    in a : u8\n    out y : u8\n    y = a\n}\n");
    let res = parse_timed(src);
    assert!(
        !res.error_codes().contains(&"E0018"),
        "{:?}",
        res.error_codes()
    );
}

/// `parse` kendi derleyici yığınında koşar: çağıranın yığını küçük olsa da
/// (Windows ana iş parçacığı 1 MB; burada 256 KB) sınırdaki girdi taşmaz.
/// Sınır tek başına yetmez — sınırdaki ağacı kurmak debug'da ~1,5 MB ister.
#[test]
fn parse_is_safe_from_a_small_caller_stack() {
    let src = module(&format!("{}a{}", "(".repeat(20_000), ")".repeat(20_000)));
    let codes = std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(move || parse(FileId(0), &src).error_codes())
        .expect("iş parçacığı")
        .join()
        .expect("panik yok");
    assert_eq!(codes, ["E0018"]);
}
