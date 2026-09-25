//! Açılım düğüm bütçesi tüm yazımları sayar (ADR-0068 §6 — üçüncü fuzz
//! bulgusu).
//!
//! İlk bütçe yalnız `stmts + exprs` büyümesini sayıyordu; `Cloner` desen,
//! tip ve blok arenalarını da kopyalar, generic monomorf klonları hiç
//! sayılmıyordu. Her sonda sayılmayan TEK bir arenayı şişirir: eski
//! kuralda E2027 gelmez, AST yüz binlerce düğüme büyür.

use volt_span::FileId;
use volt_syntax::parser::{parse, ParseResult};

/// Bütçe (262 144) + kaynak payı; bu sınırın üstü bütçe deliğidir.
const AST_CEILING: usize = 300_000;

fn p(src: &str) -> ParseResult {
    parse(FileId(0), src)
}

/// Açılımın yazdığı bütün arenalar.
fn total_nodes(r: &ParseResult) -> usize {
    let a = &r.ast;
    a.stmts.len() + a.exprs.len() + a.types.len() + a.patterns.len() + a.blocks.len()
}

fn assert_budget_error(r: &ParseResult, what: &str) {
    assert_budget_error_within(r, what, AST_CEILING);
}

/// `ceiling`: bütçe + kaynağın kendi düğümleri (büyük şablonlu sondalar).
fn assert_budget_error_within(r: &ParseResult, what: &str, ceiling: usize) {
    assert_eq!(
        r.error_codes(),
        vec!["E2027"],
        "{what}: {:?}",
        r.error_codes()
    );
    let msg = &r.diagnostics[0].message;
    assert!(
        msg.contains("node budget") || msg.contains("düğüm bütçesi"),
        "{what}: {msg}"
    );
    assert!(
        total_nodes(r) < ceiling,
        "{what}: AST bütçede kalmalı, {} düğüm",
        total_nodes(r)
    );
}

#[test]
fn pattern_nodes_count_against_the_unroll_budget() {
    // 4096 yineleme × 101 desen (`_ | _ | …`), ifade yineleme başına 4:
    // eski kural (deyim + ifade ≈ 20 k) bütçeye hiç yaklaşmazdı.
    let alts = vec!["_"; 100].join(" | ");
    let src = format!(
        "module M {{ in a : u8\n out o : [u8; 4096]\n for i in 0..4096 {{ o[i] = match a {{ {alts} => a }} }} }}"
    );
    let r = p(&src);
    assert_budget_error(&r, "desen");
    assert!(
        r.ast.patterns.len() < AST_CEILING,
        "{}",
        r.ast.patterns.len()
    );
}

#[test]
fn type_nodes_count_against_the_unroll_budget() {
    // Demet tipi ifade içermez: 4096 × 101 tip düğümü, ifade sayısı sabit.
    let ty = format!("({})", vec!["u8"; 100].join(", "));
    let src = format!(
        "module M {{ in a : u8\n out o : u8\n for i in 0..4096 {{ let x : {ty} = a }}\n o = a }}"
    );
    let r = p(&src);
    assert_budget_error(&r, "tip");
    assert!(r.ast.types.len() < AST_CEILING, "{}", r.ast.types.len());
}

#[test]
fn generic_instantiation_clones_count_against_the_same_budget() {
    // B<4096> 4096 ayrı monomorf ister (B<0>..B<4095>); her klon ~200
    // düğüm. Mono klonu hiçbir bütçeye düşmüyordu (u16 ctx doyunca bile
    // klonlamaya devam ederdi).
    let terms = vec!["a"; 100].join(" + ");
    let src = format!(
        "module B<const K : u32> {{ in a : u8\n out o : u8\n for i in 0..K {{ let x = B<i> {{ a: a }} }}\n o = {terms} }}\n\
         module T {{ in a : u8\n out o : u8\n let b = B<4096> {{ a: a }}\n o = b.o }}"
    );
    let r = p(&src);
    assert_budget_error(&r, "mono");
}

#[test]
fn budget_is_shared_by_all_modules_of_the_unit() {
    // Tek başına sığan iki açılım (her biri ~150 k düğüm) birlikte aşar:
    // bütçe modül başına değil derleme birimi başına.
    let terms = vec!["a"; 18].join(" + ");
    let body = |name: &str| {
        format!(
            "module {name} {{ in a : u8\n out o : [u8; 4096]\n for i in 0..4096 {{ o[i] = {terms} }} }}\n"
        )
    };
    let one = p(&body("A"));
    assert!(one.diagnostics.is_empty(), "{:?}", one.error_codes());
    let both = p(&format!("{}{}", body("A"), body("C")));
    assert_budget_error(&both, "iki modül");
}

#[test]
fn loop_skipped_after_a_generic_clone_overran_the_budget_is_reported() {
    // T'nin açılımı bütçenin altında kalır (~215 k), ardından V<1> klonu
    // (~60 k) bütçeyi aşar; V_1'in `for`u açılmadan atlanır. Atlanan
    // döngü sessiz kalamaz (eksik donanım): tek E2027 şart.
    let t_terms = vec!["a"; 29].join(" + ");
    // Geniş gövde (2000 × ~30 düğüm); tek uzun zincir klonda derin özyineleme olurdu.
    let v_terms = vec!["a"; 15].join(" + ");
    let v_lets: String = (0..2000)
        .map(|k| format!("let x{k} = {v_terms}\n"))
        .collect();
    let src = format!(
        "module V<const K : u32> {{ in a : u8\n out o : u8\n out p : [u8; 2]\n for j in 0..2 {{ p[j] = a }}\n {v_lets} o = a }}\n\
         module T {{ in a : u8\n out o : [u8; 3500]\n out q : u8\n for i in 0..3500 {{ o[i] = {t_terms} }}\n let v = V<1> {{ a: a }}\n q = v.o }}"
    );
    let r = p(&src);
    // Kaynaktaki V şablonu (~60 k) + son klon (~60 k) bütçenin üstünde kalır.
    assert_budget_error_within(&r, "atlanan döngü", AST_CEILING + 130_000);
}

#[test]
fn body_with_recovery_nodes_is_unrolled_once() {
    // Hata kurtarmanın bıraktığı gövde bir kez açılır (ADR-0068 §6): tanı
    // zaten verildi, kopyaları katlanırdı ve parse hatası anlamsal
    // aşamaları durdurur.
    let r = p("module M { in a : u8\n out o : [u8; 100]\n for i in 0..100 { o[i] = a + } }");
    assert!(!r.diagnostics.is_empty());
    assert!(!r.error_codes().contains(&"E2027"), "{:?}", r.error_codes());
    assert_eq!(r.ast.generate.iterations.len(), 1);
    // Aynı gövde hatasızken bütün yinelemeler açılır.
    let ok = p("module M { in a : u8\n out o : [u8; 100]\n for i in 0..100 { o[i] = a + a } }");
    assert!(ok.diagnostics.is_empty(), "{:?}", ok.error_codes());
    assert_eq!(ok.ast.generate.iterations.len(), 100);
}

#[test]
fn nested_loops_around_a_recovered_body_expand_once_each() {
    // Küçültülmüş fuzz girdisinin biçimi: üç iç içe süslüsüz `for`, gövdede
    // kurtarma deseni. Önce 65 534 yineleme, şimdi her seviye bir kez.
    let r = p("const W : u32 = 54;\nmodule for 0..W for 0..W for 0..W match e F([[[[[[[[");
    assert!(
        r.ast.generate.iterations.len() <= 3,
        "{}",
        r.ast.generate.iterations.len()
    );
    assert!(total_nodes(&r) < 1_000, "{}", total_nodes(&r));
}

#[test]
fn copied_identifier_bytes_count_against_the_budget() {
    // Düğüm sayısı küçük, bayt büyük: 3 900 karakterlik tek ad her
    // yinelemede kopyalanır (ad + yeniden ad + kaynak adı). Yalnız düğüm
    // sayılınca 52 225 yineleme açılıp 459 MB kuruluyordu.
    let name = "x".repeat(3900);
    let src = format!(
        "const W : u32 = 54;\nmodule M {{ in a : u8\n out o : u8\n for i in 0..W {{ for j in 0..W {{ for k in 0..W {{ let {name} = a }} }} }} }}"
    );
    let r = p(&src);
    assert_budget_error(&r, "uzun ad");
    let copied: usize = r.ast.generate.source_names.len() * 2 * name.len();
    assert!(copied < 64 << 20, "{} MB ad kopyası", copied >> 20);
}
