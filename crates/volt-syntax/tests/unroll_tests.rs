//! Modül seviyesi `for` açılımı (ADR-0056) — parser katmanı.
//!
//! Gövde yineleme başına klonlanır: örnek/`let` adları `_<i>` soneki
//! alır, döngü değişkeni literale iner, sabit aritmetik katlanır, her
//! yineleme benzersiz `Span.ctx` taşır ve `generate` yan tablosuna
//! yazılır. Sınır tanıları (E2005/E2027/E2028) ve gövde kısıtları
//! (E0003) da parse içinde üretilir.

use std::collections::HashSet;

use volt_ast::{ExprKind, ItemKind, LValueSuffix, ModuleDecl, SourceFile, StmtKind};
use volt_span::FileId;
use volt_syntax::parser::{parse, ParseResult};

fn p(src: &str) -> ParseResult {
    parse(FileId(0), src)
}

fn module<'a>(ast: &'a SourceFile, name: &str) -> &'a ModuleDecl {
    ast.items
        .iter()
        .find_map(|&i| match &ast.items_arena[i].kind {
            ItemKind::Module(m) if m.name.text == name => Some(m),
            _ => None,
        })
        .unwrap_or_else(|| panic!("modül bulunamadı: {name}"))
}

/// Modül gövdesindeki örnek adları (sırayla).
fn instance_names(ast: &SourceFile, m: &ModuleDecl) -> Vec<String> {
    m.body
        .iter()
        .filter_map(|&s| match &ast.stmts[s].kind {
            StmtKind::Instance(i) => Some(i.name.text.clone()),
            _ => None,
        })
        .collect()
}

fn let_names(ast: &SourceFile, m: &ModuleDecl) -> Vec<String> {
    m.body
        .iter()
        .filter_map(|&s| match &ast.stmts[s].kind {
            StmtKind::Let(l) => Some(l.name.text.clone()),
            _ => None,
        })
        .collect()
}

fn has_for(ast: &SourceFile, m: &ModuleDecl) -> bool {
    m.body
        .iter()
        .any(|&s| matches!(ast.stmts[s].kind, StmtKind::For(_)))
}

const PE: &str = "module Pe { in clk : clock\n in a : u8\n out c : u8\n reg r : u8 = 0\n on clk { r <= a }\n c = r }\n";

#[test]
fn instances_in_for_are_unrolled_with_index_suffix() {
    let src = format!(
        "{PE}module Top {{ in clk : clock\n in bus : [u8; 4]\n out res : [u8; 4]\n for i in 0..4 {{ let pe = Pe {{ clk: clk, a: bus[i] }}\n res[i] = pe.c }} }}"
    );
    let r = p(&src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    let top = module(&r.ast, "Top");
    assert!(!has_for(&r.ast, top), "for gövdesi açılmış olmalı");
    assert_eq!(
        instance_names(&r.ast, top),
        ["pe_0", "pe_1", "pe_2", "pe_3"]
    );
    // 4 örnek + 4 atama
    assert_eq!(top.body.len(), 8);
}

#[test]
fn loop_variable_is_substituted_as_literal_in_bindings_and_lhs() {
    let src = format!(
        "{PE}module Top {{ in clk : clock\n in bus : [u8; 2]\n out res : [u8; 2]\n for i in 0..2 {{ let pe = Pe {{ clk: clk, a: bus[i] }}\n res[i] = pe.c }} }}"
    );
    let r = p(&src);
    let top = module(&r.ast, "Top");
    // İkinci yineleme: bağlama `bus[1]`, atama `res[1] = pe_1.c`.
    let StmtKind::Instance(inst) = &r.ast.stmts[top.body[2]].kind else {
        panic!("örnek bekleniyor")
    };
    let a = inst
        .bindings
        .iter()
        .find(|b| b.port_name.text == "a")
        .unwrap();
    let ExprKind::Index { index, .. } = &r.ast.exprs[a.value.unwrap()].kind else {
        panic!("indeks bekleniyor")
    };
    assert!(matches!(
        r.ast.exprs[*index].kind,
        ExprKind::IntLit { value: 1, .. }
    ));
    let StmtKind::Assign(asg) = &r.ast.stmts[top.body[3]].kind else {
        panic!("atama bekleniyor")
    };
    let LValueSuffix::Index(i) = &asg.lhs.suffixes[0] else {
        panic!()
    };
    assert!(matches!(
        r.ast.exprs[*i].kind,
        ExprKind::IntLit { value: 1, .. }
    ));
    let ExprKind::Field { base, field } = &r.ast.exprs[asg.rhs].kind else {
        panic!("alan erişimi bekleniyor")
    };
    assert_eq!(field.text, "c");
    let ExprKind::Path(path) = &r.ast.exprs[*base].kind else {
        panic!()
    };
    assert_eq!(path.segments[0].text, "pe_1");
}

#[test]
fn let_wires_in_for_get_iteration_suffix_and_references_follow() {
    let src = "module M { in x : [u8; 3]\n out y : [u8; 3]\n for i in 0..3 { let t : u8 = x[i] + 1\n y[i] = t } }";
    let r = p(src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    let m = module(&r.ast, "M");
    assert_eq!(let_names(&r.ast, m), ["t_0", "t_1", "t_2"]);
    let StmtKind::Assign(asg) = &r.ast.stmts[m.body[5]].kind else {
        panic!()
    };
    let ExprKind::Path(path) = &r.ast.exprs[asg.rhs].kind else {
        panic!()
    };
    assert_eq!(path.segments[0].text, "t_2");
}

#[test]
fn nested_for_names_outer_index_first_and_folds_index_arithmetic() {
    let src = format!(
        "{PE}module Grid {{ in clk : clock\n in bus : [u8; 6]\n out res : [u8; 6]\n for y in 0..2 {{ for x in 0..3 {{ let pe = Pe {{ clk: clk, a: bus[y * 3 + x] }}\n res[y * 3 + x] = pe.c }} }} }}"
    );
    let r = p(&src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    let g = module(&r.ast, "Grid");
    assert_eq!(
        instance_names(&r.ast, g),
        ["pe_0_0", "pe_0_1", "pe_0_2", "pe_1_0", "pe_1_1", "pe_1_2"]
    );
    // Son atama: res[1*3+2] → katlanmış literal 5.
    let StmtKind::Assign(asg) = &r.ast.stmts[*g.body.last().unwrap()].kind else {
        panic!()
    };
    let LValueSuffix::Index(i) = &asg.lhs.suffixes[0] else {
        panic!()
    };
    assert!(
        matches!(r.ast.exprs[*i].kind, ExprKind::IntLit { value: 5, .. }),
        "indeks katlanmalı: {:?}",
        r.ast.exprs[*i].kind
    );
}

#[test]
fn each_iteration_gets_a_unique_ctx_recorded_in_generate_table() {
    let src = "module M { in x : [u8; 4]\n out y : [u8; 4]\n for i in 0..4 { y[i] = x[i] } }";
    let r = p(src);
    let m = module(&r.ast, "M");
    let ctxs: HashSet<u16> = m.body.iter().map(|&s| r.ast.stmts[s].span.ctx).collect();
    assert_eq!(ctxs.len(), 4, "her yineleme ayrı ctx taşımalı");
    assert!(!ctxs.contains(&0));
    assert_eq!(r.ast.generate.iterations.len(), 4);
    for (k, &s) in m.body.iter().enumerate() {
        let chain = r.ast.generate.chain(r.ast.stmts[s].span.ctx);
        assert_eq!(chain, vec![("i".to_string(), k as i128)]);
    }
}

#[test]
fn nested_iteration_chain_lists_outer_then_inner() {
    let src = "module M { in x : [u8; 6]\n out y : [u8; 6]\n for a in 0..2 { for b in 0..3 { y[a * 3 + b] = x[a * 3 + b] } } }";
    let r = p(src);
    let m = module(&r.ast, "M");
    // 2 dış + 6 iç yineleme.
    assert_eq!(r.ast.generate.iterations.len(), 8);
    let last = r.ast.stmts[*m.body.last().unwrap()].span.ctx;
    assert_eq!(
        r.ast.generate.chain(last),
        vec![("a".to_string(), 1), ("b".to_string(), 2)]
    );
    assert!(r.ast.generate.outermost_span(last).is_some());
}

#[test]
fn span_budget_measurement_4x4_mesh_uses_28_contexts() {
    // ADR-0056 §span bütçesi: examples/systolic deseni — iki tek katlı
    // döngü (4 + 4) ve bir iç içe döngü (4 dış + 16 iç) = 28 ctx; u16
    // bütçesinin (65535) %0,04'ü. Sentetik span gerekmez: klonlar kaynak
    // konumunu korur, yalnız ctx değişir.
    let src = format!(
        "{PE}module Mesh {{ in clk : clock\n in a : [u8; 4]\n out c : [u8; 16]\n wire l : [u8; 20]\n wire m : [u8; 20]\n for y in 0..4 {{ l[y * 5] = a[y] }}\n for x in 0..4 {{ m[x] = a[x] }}\n for y in 0..4 {{ for x in 0..4 {{ let pe = Pe {{ clk: clk, a: l[y * 5 + x] }}\n l[y * 5 + x + 1] = pe.c\n c[y * 4 + x] = pe.c }} }} }}"
    );
    let r = p(&src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    assert_eq!(r.ast.generate.iterations.len(), 28);
    let mesh = module(&r.ast, "Mesh");
    assert_eq!(instance_names(&r.ast, mesh).len(), 16);
    // Üretilen bildirim sayısı: 4 + 4 + 16 × 3 = 56 modül deyimi + 2 wire.
    assert_eq!(mesh.body.len(), 58);
    // Bildirim span'leri (isim span'i + ctx) çakışmaz.
    let decl_spans: HashSet<_> = mesh
        .body
        .iter()
        .filter_map(|&s| match &r.ast.stmts[s].kind {
            StmtKind::Instance(i) => Some(i.name.span),
            _ => None,
        })
        .collect();
    assert_eq!(decl_spans.len(), 16);
}

#[test]
fn const_and_generic_bounds_are_folded() {
    let src = format!(
        "const N : u32 = 3\n{PE}module G<const K: u32> {{ in clk : clock\n in bus : [u8; K]\n out res : [u8; K]\n for i in 0..K {{ let pe = Pe {{ clk: clk, a: bus[i] }}\n res[i] = pe.c }} }}\nmodule Top {{ in clk : clock\n in bus : [u8; 2]\n out res : [u8; 2]\n out r2 : [u8; N]\n let g = G<2> {{ clk: clk, bus: bus }}\n res = g.res\n for i in 0..N {{ r2[i] = bus[0] }} }}"
    );
    let r = p(&src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    // Monomorf G_2 içindeki for, ikame sonrası 2 yinelemeye açılır.
    let g2 = module(&r.ast, "G_2");
    assert_eq!(instance_names(&r.ast, g2), ["pe_0", "pe_1"]);
    assert!(!has_for(&r.ast, g2));
    let top = module(&r.ast, "Top");
    assert!(!has_for(&r.ast, top));
}

#[test]
fn generic_instantiation_inside_for_is_monomorphised() {
    let src = "module W<const K: u32> { in clk : clock\n in a : uint<K>\n out c : uint<K>\n c = a }\nmodule Top { in clk : clock\n in x : [u8; 2]\n out y : [u8; 2]\n for i in 0..2 { let w = W<8> { clk: clk, a: x[i] }\n y[i] = w.c } }";
    let r = p(src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    let top = module(&r.ast, "Top");
    assert_eq!(instance_names(&r.ast, top), ["w_0", "w_1"]);
    for &s in &top.body {
        if let StmtKind::Instance(i) = &r.ast.stmts[s].kind {
            assert_eq!(i.module_path.segments[0].text, "W_8");
        }
    }
}

#[test]
fn non_constant_bound_is_e2021_and_loop_is_dropped() {
    let r = p(
        "module M { in n : u8\n in x : [u8; 4]\n out y : [u8; 4]\n for i in 0..n { y[i] = x[i] } }",
    );
    assert_eq!(r.error_codes(), vec!["E2021"]);
    let m = module(&r.ast, "M");
    assert!(
        m.body.is_empty(),
        "hatalı döngü düşürülmeli (kaskad bastırma)"
    );
}

#[test]
fn reversed_range_is_e2028() {
    let r = p("module M { in x : [u8; 4]\n out y : [u8; 4]\n for i in 4..0 { y[i] = x[i] } }");
    assert_eq!(r.error_codes(), vec!["E2028"]);
}

#[test]
fn unroll_limit_is_e2027() {
    let r = p("module M { in x : u8\n out y : u8\n for i in 0..5000 { y = x } }");
    assert_eq!(r.error_codes(), vec!["E2027"]);
}

#[test]
fn empty_range_produces_nothing() {
    let r = p("module M { in x : u8\n out y : u8\n for i in 2..2 { y = x }\n y = x }");
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    assert_eq!(module(&r.ast, "M").body.len(), 1);
}

#[test]
fn if_and_match_in_module_for_are_e0003_once_with_folded_iterations() {
    // Her yineleme aynı E0003'ü üretir; ADR-0068 sonrası bir kez raporlanır,
    // ikinci yineleme `folded_ctxs`'te.
    let r = p("module M { in c : bool\n in x : u8\n out y : [u8; 2]\n for i in 0..2 { if c { y[i] = x } } }");
    assert_eq!(r.error_codes(), vec!["E0003"]);
    assert_eq!(r.diagnostics[0].folded_ctxs.len(), 1);
    let r = p("module M { in c : bool\n in x : u8\n out y : [u8; 2]\n for i in 0..2 { match c { _ => { y[i] = x } } } }");
    assert_eq!(r.error_codes(), vec!["E0003"]);
    assert_eq!(r.diagnostics[0].folded_ctxs.len(), 1);
}

#[test]
fn nonblocking_in_module_for_is_already_e0007_at_parse() {
    // Gövde kombinasyonel bağlamdır: `<=` parser'da E0007 (mevcut kural).
    let r = p("module M { in x : u8\n out y : [u8; 2]\n for i in 0..2 { y[i] <= x } }");
    assert!(r.error_codes().contains(&"E0007"), "{:?}", r.error_codes());
}

#[test]
fn negative_start_substitutes_negated_literal() {
    let r = p("module M { in x : i8\n out y : [i8; 2]\n for i in -1..1 { y[i + 1] = x } }");
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    let m = module(&r.ast, "M");
    // i = -1 → `-1 + 1`: negatif literal katlanmaz, Unary Neg kalır; SV hesaplar.
    let StmtKind::Assign(asg) = &r.ast.stmts[m.body[0]].kind else {
        panic!()
    };
    let LValueSuffix::Index(i) = &asg.lhs.suffixes[0] else {
        panic!()
    };
    assert!(matches!(r.ast.exprs[*i].kind, ExprKind::Binary { .. }));
    // i = 0 → `0 + 1` katlanır.
    let StmtKind::Assign(asg) = &r.ast.stmts[m.body[1]].kind else {
        panic!()
    };
    let LValueSuffix::Index(i) = &asg.lhs.suffixes[0] else {
        panic!()
    };
    assert!(matches!(
        r.ast.exprs[*i].kind,
        ExprKind::IntLit { value: 1, .. }
    ));
}

#[test]
fn block_level_for_is_left_to_the_emitter() {
    // on/comb içindeki for parser'da açılmaz (ADR-0041 yolu korunur).
    let r = p("module M { in clk : clock\n out y : u8\n reg r : [u8; 2] = [0; 2]\n on clk { for i in 0..2 { r[i] <= 1 } }\n y = r[0] }");
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    assert!(r.ast.generate.iterations.is_empty());
}

// ═══ Özdeş tanı katlama (ADR-0068) ═══════════════════════════════════

#[test]
fn nested_for_with_non_constant_inner_bound_reports_e2021_once_with_folded_copies() {
    // Fuzz bulgusu 2: iç `for`un sınırı sabit değil → her (dış, iç)
    // yineleme çifti aynı E2021'i üretirdi (WIDTH²). Artık tek tanı,
    // katlanan kopyaların ctx'leri tanıda.
    let r = p("const N : u32 = 50\nmodule M { in a : u8\n out o : u8\n for i in 0..N { for j in 0..N { for k in 0..a { o = a } } } }");
    let e2005: Vec<_> = r
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == "E2021")
        .collect();
    assert_eq!(e2005.len(), 1, "{:?}", r.error_codes());
    assert_eq!(e2005[0].folded_ctxs.len(), 50 * 50 - 1);
    assert_eq!(r.diagnostics.len(), 1, "{:?}", r.error_codes());
}

#[test]
fn iteration_specific_diagnostics_are_not_folded_together() {
    // `j in i..1`: i = 0 boş-değil, i = 1 boş, i = 2..4 ters aralık — mesaj
    // yineleme değerini taşır ("2..1", "3..1", "4..1"), üç AYRI E2028.
    let r = p("module M { in a : u8\n out o : u8\n for i in 0..5 { for j in i..1 { o = a } } }");
    let e2028: Vec<String> = r
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == "E2028")
        .map(|d| d.message.clone())
        .collect();
    assert_eq!(e2028.len(), 3, "{e2028:?}");
    assert!(e2028.iter().all(|m| m.contains("..1")), "{e2028:?}");
    assert!(r.diagnostics.iter().all(|d| d.folded_ctxs.is_empty()));
}

#[test]
fn unsupported_body_statement_is_reported_once_per_source_statement() {
    // Gövdede iki ayrı `if` → iki ayrı kaynak deyimi → iki E0003; her biri
    // 8 yinelemeden 7 katlanmış kopya taşır.
    let r = p("module M { in a : bool\n out o : u8\n for i in 0..8 { if a { o = 1 }\n if !a { o = 2 } } }");
    let e0003: Vec<_> = r
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == "E0003")
        .collect();
    assert_eq!(e0003.len(), 2, "{:?}", r.error_codes());
    assert!(e0003.iter().all(|d| d.folded_ctxs.len() == 7));
}

#[test]
fn for_inside_generic_clone_records_the_clone_ctx_as_parent() {
    // Monomorf klondaki `for` yinelemesinin kökü klonun ctx'idir (0 değil):
    // tanı notu "N generic örneklemede" diyebilsin.
    let r = p("module W<const K : u32> { in a : u8\n out o : u8\n for i in 0..2 { o = a } }\nmodule T { in a : u8\n out o : u8\n let w = W<3> { a: a }\n o = w.o }");
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    let iters: Vec<_> = r.ast.generate.iterations.values().collect();
    assert_eq!(iters.len(), 2);
    assert!(
        iters.iter().all(|it| it.parent != 0),
        "kök klon ctx'i olmalı: {iters:?}"
    );
    assert!(iters
        .iter()
        .all(|it| !r.ast.generate.iterations.contains_key(&it.parent)));
}

#[test]
fn unroll_node_budget_stops_expansion_with_a_single_e2027() {
    // Gövde ~200 düğüm × 2000 yineleme = 400k > MAX_UNROLL_NODES (262 144):
    // açılım bütçede durur, tek E2027, kalan döngüler açılmaz (kaskad yok).
    let terms = vec!["a"; 100].join(" + ");
    let src = format!(
        "module M {{ in a : u8\n out o : [u8; 2000]\n out p : [u8; 4]\n for i in 0..2000 {{ o[i] = {terms}\n o[i] = {terms} }}\n for j in 0..4 {{ p[j] = a }} }}"
    );
    let r = p(&src);
    assert_eq!(r.error_codes(), vec!["E2027"], "{:?}", r.error_codes());
    let d = &r.diagnostics[0];
    assert!(
        d.message.contains("node budget") || d.message.contains("düğüm bütçesi"),
        "{}",
        d.message
    );
    assert!(
        d.folded_ctxs.is_empty(),
        "tek tanı, kaskad yok: {:?}",
        d.folded_ctxs
    );
    assert!(
        r.ast.stmts.len() + r.ast.exprs.len() < 300_000,
        "AST bütçede kalmalı: {}",
        r.ast.stmts.len() + r.ast.exprs.len()
    );
}
