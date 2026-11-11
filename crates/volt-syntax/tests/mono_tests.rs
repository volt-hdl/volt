//! Generic örnekleme monomorfizasyonu (ADR-0041) — parser katmanı.

use volt_ast::{
    ArrayLitKind, ExprKind, Idx, ItemKind, ModuleDecl, SourceFile, StmtKind, TypeRefKind,
};
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

fn module<'a>(ast: &'a SourceFile, name: &str) -> &'a ModuleDecl {
    ast.items
        .iter()
        .find_map(|&i| match &ast.items_arena[i].kind {
            ItemKind::Module(m) if m.name.text == name => Some(m),
            _ => None,
        })
        .unwrap_or_else(|| panic!("modül bulunamadı: {name}"))
}

fn int_value(ast: &SourceFile, e: Idx<volt_ast::Expr>) -> Option<u128> {
    match ast.exprs[e].kind {
        ExprKind::IntLit { value, .. } => Some(value),
        _ => None,
    }
}

const GENERIC_FIR: &str = r#"
module Acc<const N: u32, const W: u32> {
    in  clk : clock
    in  x   : sint<W>
    out y   : sint<W>
    invariant: N > 0
    reg buf : [sint<W>; N] = [0; N]
    on clk {
        for i in 1..N { buf[i] <= buf[i - 1] }
        buf[0] <= x
    }
    y = buf[N - 1]
}
module Top {
    in  clk : clock
    in  a   : i16
    out b   : i16
    out c   : i8
    let f = Acc<4, 16> { clk, x: a }
    let g = Acc<2, 8> { clk, x: a as i8 }
    b = f.y
    c = g.y
}
"#;

#[test]
fn two_argument_tuples_produce_two_monomorphs() {
    let r = p(GENERIC_FIR);
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    assert_eq!(module_names(&r.ast), vec!["Acc_4_16", "Acc_2_8", "Top"]);
}

#[test]
fn same_arguments_share_one_monomorph() {
    let src = GENERIC_FIR.replace("Acc<2, 8> { clk, x: a as i8 }", "Acc<4, 16> { clk, x: a }");
    let src = src.replace("c = g.y", "c = g.y as i8");
    let r = p(&src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    assert_eq!(module_names(&r.ast), vec!["Acc_4_16", "Top"]);
}

#[test]
fn template_is_removed_from_items_but_not_from_arena() {
    let r = p(GENERIC_FIR);
    assert!(!module_names(&r.ast).iter().any(|n| n == "Acc"));
    assert_eq!(r.ast.items.len(), 3);
    // Monomorf modülün generics listesi boş.
    assert!(module(&r.ast, "Acc_4_16").generics.is_empty());
}

#[test]
fn instance_path_is_rewritten_and_args_cleared() {
    let r = p(GENERIC_FIR);
    let top = module(&r.ast, "Top");
    let insts: Vec<_> = top
        .body
        .iter()
        .filter_map(|&s| match &r.ast.stmts[s].kind {
            StmtKind::Instance(i) => Some(i),
            _ => None,
        })
        .collect();
    assert_eq!(insts.len(), 2);
    assert_eq!(insts[0].module_path.segments[0].text, "Acc_4_16");
    assert_eq!(insts[1].module_path.segments[0].text, "Acc_2_8");
    assert!(insts.iter().all(|i| i.generic_args.is_empty()));
    assert_eq!(insts[0].bindings.len(), 2);
}

#[test]
fn array_type_parameters_become_literals() {
    let r = p(GENERIC_FIR);
    let m = module(&r.ast, "Acc_4_16");
    let reg = m
        .body
        .iter()
        .find_map(|&s| match &r.ast.stmts[s].kind {
            StmtKind::Reg(reg) => Some(reg),
            _ => None,
        })
        .expect("reg");
    let TypeRefKind::Array { elem, len } = &r.ast.types[reg.ty.expect("tip")].kind else {
        panic!("dizi tipi bekleniyor")
    };
    assert_eq!(int_value(&r.ast, *len), Some(4));
    let TypeRefKind::SIntN(w) = &r.ast.types[*elem].kind else {
        panic!("sint<W> bekleniyor")
    };
    assert_eq!(int_value(&r.ast, *w), Some(16));
    // `[0; N]` başlangıç değeri de ikame edildi.
    let ExprKind::ArrayLit(ArrayLitKind::Repeat { count, .. }) = &r.ast.exprs[reg.init].kind else {
        panic!("tekrar literali bekleniyor")
    };
    assert_eq!(int_value(&r.ast, *count), Some(4));
}

#[test]
fn port_types_are_substituted_per_monomorph() {
    let r = p(GENERIC_FIR);
    for (name, w) in [("Acc_4_16", 16), ("Acc_2_8", 8)] {
        let m = module(&r.ast, name);
        let TypeRefKind::SIntN(e) = &r.ast.types[m.ports[1].ty].kind else {
            panic!("sint<W> bekleniyor")
        };
        assert_eq!(int_value(&r.ast, *e), Some(w));
    }
}

#[test]
fn for_loop_bound_is_substituted() {
    let r = p(GENERIC_FIR);
    let m = module(&r.ast, "Acc_2_8");
    let on = m
        .body
        .iter()
        .find_map(|&s| match &r.ast.stmts[s].kind {
            StmtKind::On(on) => Some(on),
            _ => None,
        })
        .expect("on bloğu");
    let volt_ast::BlockStmt::For(f) = &r.ast.blocks[on.body].stmts[0] else {
        panic!("for bekleniyor")
    };
    assert_eq!(int_value(&r.ast, f.end), Some(2));
    assert_eq!(int_value(&r.ast, f.start), Some(1));
}

#[test]
fn contract_expression_is_substituted() {
    let r = p(GENERIC_FIR);
    let m = module(&r.ast, "Acc_4_16");
    let ExprKind::Binary { lhs, .. } = &r.ast.exprs[m.contracts[0].expr].kind else {
        panic!("ikili ifade bekleniyor")
    };
    assert_eq!(int_value(&r.ast, *lhs), Some(4));
}

#[test]
fn assignment_index_expression_is_substituted() {
    let r = p(GENERIC_FIR);
    let m = module(&r.ast, "Acc_4_16");
    let assign = m
        .body
        .iter()
        .find_map(|&s| match &r.ast.stmts[s].kind {
            StmtKind::Assign(a) => Some(a),
            _ => None,
        })
        .expect("atama");
    let ExprKind::Index { index, .. } = &r.ast.exprs[assign.rhs].kind else {
        panic!("indeks bekleniyor")
    };
    let ExprKind::Binary { lhs, .. } = &r.ast.exprs[*index].kind else {
        panic!("N - 1 bekleniyor")
    };
    assert_eq!(int_value(&r.ast, *lhs), Some(4));
}

#[test]
fn wrong_arity_is_e2003() {
    let src = GENERIC_FIR.replace("Acc<4, 16>", "Acc<4>");
    let r = p(&src);
    assert_eq!(r.error_codes(), vec!["E2003"]);
    // Şablon kullanılmadı → yerinde kalır, Acc_2_8 üretildi.
    assert_eq!(module_names(&r.ast), vec!["Acc_2_8", "Top"]);
}

#[test]
fn non_literal_argument_is_e2008() {
    let src = GENERIC_FIR.replace("Acc<4, 16>", "Acc<TAPS, 16>");
    let r = p(&src);
    assert_eq!(r.error_codes(), vec!["E2008"]);
    // Kaskad bastırma: hatalı örneklemenin argümanları temizlenir,
    // diğer örnekleme yine açılır.
    assert_eq!(module_names(&r.ast), vec!["Acc_2_8", "Top"]);
}

#[test]
fn type_parameter_argument_is_e0003() {
    let src = "module G<T> { in a : bool  out b : bool  b = a }\n\
               module Top { in a : bool  out b : bool  let g = G<u8> { a }  b = g.b }";
    let r = p(src);
    assert_eq!(r.error_codes(), vec!["E0003"]);
}

#[test]
fn arguments_on_non_generic_module_is_e2003() {
    let src = "module Plain { in a : bool  out b : bool  b = a }\n\
               module Top { in a : bool  out b : bool  let g = Plain<8> { a }  b = g.b }";
    let r = p(src);
    assert_eq!(r.error_codes(), vec!["E2003"]);
    let d = &r.diagnostics[0];
    assert!(!d.notes.is_empty() && d.notes.iter().all(|n| !n.text.contains("ADR-")));
}

#[test]
fn uninstantiated_generic_module_is_left_alone() {
    let src = "module Lonely<const N: u32> { in a : bits<N>  out b : bits<N>  b = a }";
    let r = p(src);
    assert!(r.diagnostics.is_empty());
    assert_eq!(module_names(&r.ast), vec!["Lonely"]);
    assert_eq!(module(&r.ast, "Lonely").generics.len(), 1);
}

#[test]
fn nested_generic_instantiation_is_expanded() {
    let src = r#"
module Inner<const W: u32> {
    in a : uint<W>
    out b : uint<W>
    b = a
}
module Outer<const W: u32> {
    in a : uint<W>
    out b : uint<W>
    let i = Inner<W> { a }
    b = i.b
}
module Top {
    in a : u8
    out b : u8
    let o = Outer<8> { a }
    b = o.b
}
"#;
    let r = p(src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    assert_eq!(module_names(&r.ast), vec!["Inner_8", "Outer_8", "Top"]);
    let outer = module(&r.ast, "Outer_8");
    let inst = outer
        .body
        .iter()
        .find_map(|&s| match &r.ast.stmts[s].kind {
            StmtKind::Instance(i) => Some(i),
            _ => None,
        })
        .expect("örnekleme");
    assert_eq!(inst.module_path.segments[0].text, "Inner_8");
}

#[test]
fn delayed_side_table_survives_cloning() {
    let src = r#"
module D<const W: u32> {
    in clk : clock
    in a : Delayed<uint<W>, 2>
    out b : uint<W>
    b = a
}
module Top {
    in clk : clock
    in a : u8
    out b : u8
    let d = D<8> { clk, a }
    b = d.b
}
"#;
    let r = p(src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    let d = module(&r.ast, "D_8");
    let ty = d.ports[1].ty;
    let (n, _) = r
        .ast
        .timing
        .delayed_types
        .get(&ty)
        .expect("Delayed yan tablosu");
    assert_eq!(int_value(&r.ast, *n), Some(2));
    // Klon tip düğümü ikameli genişlik taşıyor.
    let TypeRefKind::UIntN(w) = &r.ast.types[ty].kind else {
        panic!("uint<W> bekleniyor")
    };
    assert_eq!(int_value(&r.ast, *w), Some(8));
}

#[test]
fn builtin_primitive_instances_are_untouched() {
    let src = "module Top { in clk : clock  in a : u8  out b : u8\n\
               let f = SyncFifo<u8, 16> { clk, wr_en: true, rd_en: true, wr_data: a }\n\
               b = f.rd_data }";
    let r = p(src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    let top = module(&r.ast, "Top");
    let inst = top
        .body
        .iter()
        .find_map(|&s| match &r.ast.stmts[s].kind {
            StmtKind::Instance(i) => Some(i),
            _ => None,
        })
        .expect("örnekleme");
    assert_eq!(inst.generic_args.len(), 2);
    assert_eq!(inst.module_path.segments[0].text, "SyncFifo");
}

// ═══ #205: iki farklı şablon aynı monomorf adına iniyor ═══════════════

/// #205'in örneği: `M<1, 8>` ve `M_1<8>` ikisi de `M_1_8` adını kurar.
const ISSUE_205: &str = r#"module M<const A: u32, const B: u32> { in x : uint<B>  out y : uint<B>  y = x + A }
module M_1<const C: u32>             { in x : u8       out y : u8       y = x ^ C }
module Top {
    in  x : u8
    out a : u8
    out b : u8
    let p = M<1, 8> { x: x }
    let q = M_1<8> { x: x }
    a = p.y
    b = q.y
}
"#;

/// `module`'deki `inst` örneğinin bağlandığı modül adı.
fn instance_target(ast: &SourceFile, module_name: &str, inst: &str) -> String {
    module(ast, module_name)
        .body
        .iter()
        .find_map(|&s| match &ast.stmts[s].kind {
            StmtKind::Instance(i) if i.name.text == inst => {
                Some(i.module_path.segments[0].text.clone())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("örnek bulunamadı: {inst}"))
}

/// Öğe listesindeki `name` modülünün adı kaynakta hangi bayttan başlıyor:
/// monomorf, şablonunun ad konumunu taşır (klon span'ı, bağlam ayrı).
fn name_start(ast: &SourceFile, name: &str) -> u32 {
    module(ast, name).name.span.start
}

/// `src`'de `module <template><` başlığındaki şablon adının ilk baytı.
fn template_start(src: &str, template: &str) -> u32 {
    let at = src
        .find(&format!("module {template}<"))
        .unwrap_or_else(|| panic!("şablon yok: {template}"));
    u32::try_from(at + "module ".len()).expect("küçük dosya")
}

/// Her örnek ya kendi şablonunun bir monomorfuna bağlanır ya da (çakışma
/// tanısından sonra) kendi şablonunda kalır; başka şablonun gövdesine
/// ASLA bağlanmaz. Düzeltmeden önce `q` → `M_1_8` = `M`'nin klonu.
fn assert_own_bodies(src: &str, pairs: &[(&str, &str)]) {
    let r = p(src);
    for &(inst, template) in pairs {
        let target = instance_target(&r.ast, "Top", inst);
        assert_eq!(
            name_start(&r.ast, &target),
            template_start(src, template),
            "'{inst}' ({template}) '{target}' modülüne bağlandı; tanılar: {:?}",
            r.error_codes()
        );
    }
}

#[test]
fn issue_205_each_instance_is_bound_to_its_own_template() {
    assert_own_bodies(ISSUE_205, &[("p", "M"), ("q", "M_1")]);
}

#[test]
fn issue_205_each_instance_is_bound_to_its_own_template_in_the_other_order() {
    let src = ISSUE_205.replace(
        "    let p = M<1, 8> { x: x }\n    let q = M_1<8> { x: x }\n",
        "    let q = M_1<8> { x: x }\n    let p = M<1, 8> { x: x }\n",
    );
    assert_own_bodies(&src, &[("p", "M"), ("q", "M_1")]);
}

#[test]
fn issue_205_two_templates_building_one_name_is_e1003() {
    let r = p(ISSUE_205);
    assert_eq!(r.error_codes(), ["E1003"]);
    let d = &r.diagnostics[0];
    assert!(
        d.message
            .contains("'M_1_8' is the generated module name of both M<1, 8> and M_1<8>"),
        "{}",
        d.message
    );
    // Dört konum: iki örnek sitesi (birincil: sonraki) ve iki şablon adı.
    let starts: Vec<(u32, bool)> = d.spans.iter().map(|l| (l.span.start, l.primary)).collect();
    let at = |needle: &str| u32::try_from(ISSUE_205.find(needle).expect(needle)).expect("küçük");
    assert!(starts.contains(&(at("M_1<8> {"), true)), "{starts:?}");
    assert!(starts.contains(&(at("M<1, 8> {"), false)), "{starts:?}");
    assert!(
        starts.contains(&(template_start(ISSUE_205, "M"), false)),
        "{starts:?}"
    );
    assert!(
        starts.contains(&(template_start(ISSUE_205, "M_1"), false)),
        "{starts:?}"
    );
    assert_eq!(starts.len(), 4, "{starts:?}");
    // Help hangi modüllerin adının değişebileceğini söyler.
    let help = d.help.as_deref().unwrap_or_default();
    assert!(help.contains("'M'") && help.contains("'M_1'"), "{help}");
}

/// Çakışma iç içe açılımda (ikinci tur) doğsa da bulunur: `W<8>`'in
/// gövdesindeki `M<1, N>` ikame sonrası `M<1, 8>` olur.
#[test]
fn issue_205_collision_in_a_nested_instantiation_is_e1003() {
    let src = r#"module M<const A: u32, const B: u32> { in x : uint<B>  out y : uint<B>  y = x + A }
module M_1<const C: u32>             { in x : u8       out y : u8       y = x ^ C }
module W<const N: u32> { in x : u8  out y : u8  let i = M<1, N> { x: x }  y = i.y }
module Top {
    in  x : u8
    out a : u8
    out b : u8
    let p = W<8> { x: x }
    let q = M_1<8> { x: x }
    a = p.y
    b = q.y
}
"#;
    let r = p(src);
    assert_eq!(r.error_codes(), ["E1003"]);
    assert_own_bodies(src, &[("q", "M_1")]);
}

/// Aynı şablonun farklı argüman listeleri ayrı adlara iner: argüman sayısı
/// şablonda sabit, ondalık değerde `_` yok (`M<1, 23>` ≠ `M<12, 3>`).
#[test]
fn issue_205_one_template_with_two_argument_lists_builds_two_names() {
    let src = "module M<const A: u32, const B: u32> { in x : u8  out y : u8  y = x + A + B }\n\
               module Top { in x : u8  out a : u8  out b : u8\n\
               let p = M<1, 23> { x: x }\n\
               let q = M<12, 3> { x: x }\n\
               a = p.y  b = q.y }";
    let r = p(src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    assert_eq!(module_names(&r.ast), vec!["M_1_23", "M_12_3", "Top"]);
}
