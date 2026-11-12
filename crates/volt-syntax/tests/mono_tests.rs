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

/// Argüman verilmiş ama sayısı yanlış: tanı sayının yanlış olduğunu
/// söyler, argümanın hiç verilmediğini (#249'un metni) değil. Tanı metni
/// kullanıcıya görünen çıktıdır (ADR-0109 §4).
#[test]
fn wrong_arity_with_arguments_says_the_count_is_wrong() {
    let src = GENERIC_FIR.replace("Acc<4, 16>", "Acc<4>");
    let r = p(&src);
    assert_eq!(r.error_codes(), vec!["E2003"]);
    let d = &r.diagnostics[0];
    assert!(
        d.message
            .contains("'Acc' expects 2 generic argument(s), got 1"),
        "{}",
        d.message
    );
    let primary = d.spans.iter().find(|l| l.primary).expect("birincil");
    assert_eq!(primary.label, "wrong generic argument count");
    let help = d.help.as_deref().unwrap_or_default();
    assert_eq!(
        help,
        "give exactly 2 argument(s), one per <const ...> parameter"
    );
    assert!(
        d.notes
            .iter()
            .all(|n| !n.text.contains("no default values")),
        "{:?}",
        d.notes
    );
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

/// İç içe generic örnekleme zinciri tur sınırından (64) uzunsa tek E2003;
/// sessiz bir kesinti değil (ADR-0109 hedefli koşu, #205:
/// `err_depth_exceeded` → `()` kaçıyordu). Her `T<i><N>` gövdesinde
/// `T<i+1><N>` örneklenir, her tur bir yeni monomorf üretir. (Örnekleme
/// argümanında işleç yazılamaz, `R<N + 1>` E0010'dur; modül içi `const`
/// yok: derinlik yalnız ayrı şablonlarla aşılır.)
#[test]
fn generic_instantiation_chain_longer_than_the_round_limit_is_e2003() {
    let mut src = String::new();
    for i in 0..70 {
        src.push_str(&format!(
            "module T{i}<const N: u32> {{ in a : bool  out b : bool  let x = T{}<N> {{ a: a }}  b = x.b }}\n",
            i + 1
        ));
    }
    src.push_str("module T70<const N: u32> { in a : bool  out b : bool  b = a }\n");
    src.push_str("module Top { in a : bool  out b : bool  let x = T0<1> { a: a }  b = x.b }\n");
    let r = p(&src);
    assert_eq!(r.error_codes(), ["E2003"]);
    assert!(
        r.diagnostics[0]
            .message
            .contains("generic instantiation depth exceeded"),
        "{}",
        r.diagnostics[0].message
    );
}

/// Generic örnekleme tanılarının notu kuralı söyler: her argüman demeti
/// ayrı bir modüldür. Not boş ya da ilgisiz bir metin olamaz (ADR-0109
/// hedefli koşu, #205: `adr_note` → `String::new()` / `"xyzzy"` kaçıyordu).
#[test]
fn generic_instantiation_diagnostics_say_each_argument_tuple_is_a_module() {
    let not_generic = "module Plain { in a : bool  out b : bool  b = a }\n\
                       module Top { in a : bool  out b : bool  let g = Plain<8> { a }  b = g.b }";
    let type_arg = "module G<T> { in a : bool  out b : bool  b = a }\n\
                    module Top { in a : bool  out b : bool  let g = G<u8> { a }  b = g.b }";
    let arity = GENERIC_FIR.replace("Acc<4, 16>", "Acc<4>");
    for src in [not_generic, type_arg, arity.as_str()] {
        let r = p(src);
        assert_eq!(r.diagnostics.len(), 1, "{:?}", r.error_codes());
        let notes: Vec<&str> = r.diagnostics[0]
            .notes
            .iter()
            .map(|n| n.text.as_str())
            .collect();
        assert!(
            notes
                .iter()
                .any(|n| n.contains("monomorphised per argument tuple")),
            "{notes:?}"
        );
    }
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
    // Kaskad bastırma: çakışan örneğin generic argümanları temizlenir, yolu
    // kendi şablonunda kalır (diğer mono hatalarıyla aynı).
    let q = module(&r.ast, "Top")
        .body
        .iter()
        .find_map(|&s| match &r.ast.stmts[s].kind {
            StmtKind::Instance(i) if i.name.text == "q" => Some(i),
            _ => None,
        })
        .expect("q");
    assert!(q.generic_args.is_empty());
    assert_eq!(q.module_path.segments[0].text, "M_1");
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
    // Adı ilk kuran `q` (birinci tur) kaynakta SONRA gelir; ileti açılış
    // sırasına değil kaynak sırasına bakar: W'nin gövdesindeki `M<1, N>`
    // önce adlandırılır, birincil etiket `q`'da.
    let d = &r.diagnostics[0];
    assert!(
        d.message
            .contains("'M_1_8' is the generated module name of both M<1, 8> and M_1<8>"),
        "{}",
        d.message
    );
    let primary = d.spans.iter().find(|l| l.primary).expect("birincil");
    let q_site = u32::try_from(src.find("M_1<8> {").expect("q")).expect("küçük");
    assert_eq!(primary.span.start, q_site);
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

// ═══ #249: generic örnek argümansız yazılamaz ═════════════════════════

/// #249'un sessiz yanlış biçimi: `M<1>` `M_1` adını kurar; `M_1<5>`
/// şablon `M_1`'i öğe listesinden çıkarır. Argümansız `r = M_1 { }`
/// düzeltmeden önce `M_1` adlı modüle, yani `M<1>`'in gövdesine bağlanıyordu.
const ISSUE_249: &str = r#"
module M<const A: u32> { in x : u8  out y : u8  y = x + A }
module M_1<const C: u32> { in x : u8  out y : u8  y = x ^ C }
module Top {
    in  x : u8
    out a : u8
    out b : u8
    out c : u8
    let p = M<1> { x: x }
    let q = M_1<5> { x: x }
    let r = M_1 { x: x }
    a = p.y
    b = q.y
    c = r.y
}
"#;

/// Birincil etiketin başladığı bayt (ilk tanı).
fn primary_start(r: &ParseResult) -> u32 {
    r.diagnostics[0]
        .spans
        .iter()
        .find(|l| l.primary)
        .expect("birincil")
        .span
        .start
}

/// `needle`'ın `src`'deki ilk baytı.
fn at(src: &str, needle: &str) -> u32 {
    u32::try_from(src.find(needle).unwrap_or_else(|| panic!("yok: {needle}"))).expect("küçük")
}

/// Generic parametrelerin varsayılan değeri yok (gramerde `GenericParam`
/// `= değer` almaz): argümansız örnek 0 argüman verir, şablon 1 bekler —
/// ADR-0041'in sayı kuralı, E2003, örneğin modül adında.
#[test]
fn issue_249_generic_instance_without_arguments_is_e2003() {
    let src = "module M_1<const C: u32> { in x : u8  out y : u8  y = x ^ C }
module Top { in x : u8  out b : u8
    let q = M_1 { x: x }
    b = q.y }";
    let r = p(src);
    assert_eq!(r.error_codes(), ["E2003"]);
    let d = &r.diagnostics[0];
    assert!(
        d.message
            .contains("'M_1' expects 1 generic argument(s), got 0"),
        "{}",
        d.message
    );
    assert_eq!(primary_start(&r), at(src, "M_1 { x"));
    // Öneri yazılışı ve parametrenin adını söyler.
    let help = d.help.as_deref().unwrap_or_default();
    assert!(help.contains("M_1<8>") && help.contains("'C'"), "{help}");
}

/// Argümansız örnek hiçbir monomorfa bağlanmaz: yolu şablonun adında
/// kalır (`M_1`), E2003 kendi sitesinde; `p` ve `q` kendi gövdelerinde.
#[test]
fn issue_249_instance_without_arguments_is_not_bound_to_another_expansion() {
    let r = p(ISSUE_249);
    assert_eq!(r.error_codes(), ["E2003"]);
    assert_eq!(primary_start(&r), at(ISSUE_249, "M_1 { x"));
    assert_own_bodies(ISSUE_249, &[("p", "M"), ("q", "M_1")]);
}

#[test]
fn issue_249_rule_does_not_depend_on_the_order() {
    let src = ISSUE_249.replace(
        "    let p = M<1> { x: x }\n    let q = M_1<5> { x: x }\n    let r = M_1 { x: x }\n",
        "    let r = M_1 { x: x }\n    let q = M_1<5> { x: x }\n    let p = M<1> { x: x }\n",
    );
    let r = p(&src);
    assert_eq!(r.error_codes(), ["E2003"]);
    assert_eq!(primary_start(&r), at(&src, "M_1 { x"));
    assert_own_bodies(&src, &[("p", "M"), ("q", "M_1")]);
}

/// Bir monomorfun gövdesindeki argümansız örnek de E2003 (iç içe açılım).
#[test]
fn issue_249_instance_without_arguments_in_an_expansion_is_e2003() {
    let src = "module In<const C: u32> { in x : u8  out y : u8  y = x ^ C }
module Out<const W: u32> { in x : u8  out y : u8
    let i = In { x: x }
    y = i.y }
module Top { in x : u8  out b : u8
    let o = Out<4> { x: x }
    b = o.y }";
    let r = p(src);
    assert_eq!(r.error_codes(), ["E2003"]);
    assert_eq!(primary_start(&r), at(src, "In { x"));
}

/// Generic olmayan modül argümansız örneklenir (güvenli ikiz).
#[test]
fn issue_249_plain_module_without_arguments_is_still_an_instance() {
    let src = "module M<const A: u32> { in x : u8  out y : u8  y = x + A }
module M_2 { in x : u8  out y : u8  y = x ^ 2 }
module Top { in x : u8  out a : u8  out b : u8
    let p = M<1> { x: x }
    let q = M_2 { x: x }
    a = p.y  b = q.y }";
    let r = p(src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    assert_eq!(module_names(&r.ast), vec!["M_1", "M_2", "Top"]);
    assert_eq!(instance_target(&r.ast, "Top", "q"), "M_2");
}

// ═══ #250: yalnız harf büyüklüğüyle ayrılan modül adları ══════════════

/// Her modül kendi adını taşıyan dosyaya yazılır; büyük/küçük harf
/// duyarsız dosya sisteminde `Foo.sv` ile `foo.sv` tek dosyadır, SVA
/// dosyası her yerde küçük harfli addır. E1003, sonraki adda.
#[test]
fn issue_250_module_names_that_differ_only_in_case_are_e1003() {
    let src = "module Foo { in x : u8  out y : u8  y = x }
module foo { in x : u8  out y : u8  y = x }";
    let r = p(src);
    assert_eq!(r.error_codes(), ["E1003"]);
    let d = &r.diagnostics[0];
    assert!(
        d.message
            .contains("module names 'Foo' and 'foo' differ only in letter case"),
        "{}",
        d.message
    );
    assert_eq!(primary_start(&r), at(src, "foo {"));
    let starts: Vec<u32> = d.spans.iter().map(|l| l.span.start).collect();
    assert_eq!(starts.len(), 2, "{starts:?}");
    assert!(starts.contains(&at(src, "Foo {")), "{starts:?}");
    let help = d.help.as_deref().unwrap_or_default();
    assert!(help.contains("'Foo'") && help.contains("'foo'"), "{help}");
}

#[test]
fn issue_250_rule_does_not_depend_on_the_order() {
    let src = "module foo { in x : u8  out y : u8  y = x }
module Foo { in x : u8  out y : u8  y = x }";
    let r = p(src);
    assert_eq!(r.error_codes(), ["E1003"]);
    assert!(
        r.diagnostics[0]
            .message
            .contains("module names 'foo' and 'Foo' differ only in letter case"),
        "{}",
        r.diagnostics[0].message
    );
    assert_eq!(primary_start(&r), at(src, "Foo {"));
}

/// Üç ad: ilk ada karşı her sonraki ad için bir tanı.
#[test]
fn issue_250_three_names_report_each_later_name_once() {
    let src = "module Foo { in x : u8  out y : u8  y = x }
module foo { in x : u8  out y : u8  y = x }
module FOO { in x : u8  out y : u8  y = x }";
    let r = p(src);
    assert_eq!(r.error_codes(), ["E1003", "E1003"]);
    let primaries: Vec<u32> = r
        .diagnostics
        .iter()
        .map(|d| {
            d.spans
                .iter()
                .find(|l| l.primary)
                .expect("birincil")
                .span
                .start
        })
        .collect();
    assert_eq!(primaries, [at(src, "foo {"), at(src, "FOO {")]);
}

/// Generic örneğin ürettiği ad da modül adıdır: `M<8>` → `M_8`, `m_8`
/// ile yalnız harf büyüklüğünde ayrılır. Tanı örnek sitesini gösterir.
#[test]
fn issue_250_generated_module_name_that_differs_only_in_case_is_e1003() {
    let src = "module M<const A: u32> { in x : u8  out y : u8  y = x + A }
module m_8 { in x : u8  out y : u8  y = x }
module Top { in x : u8  out a : u8  out b : u8
    let p = M<8> { x: x }
    let q = m_8 { x: x }
    a = p.y  b = q.y }";
    let r = p(src);
    assert_eq!(r.error_codes(), ["E1003"]);
    let d = &r.diagnostics[0];
    assert!(
        d.message
            .contains("module names 'm_8' and 'M_8' differ only in letter case"),
        "{}",
        d.message
    );
    // Birincil: sonraki ad, `M<8>` örneğinin sitesi (m_8'in bildirimi önce).
    assert_eq!(primary_start(&r), at(src, "M<8> {"));
    let starts: Vec<u32> = d.spans.iter().map(|l| l.span.start).collect();
    assert!(starts.contains(&at(src, "m_8 {")), "{starts:?}");
    // Help generic modülü adıyla söyler.
    let help = d.help.as_deref().unwrap_or_default();
    assert!(help.contains("'M'") && help.contains("'m_8'"), "{help}");
}

/// İki generic örnek: `M<8>` → `M_8`, `m<8>` → `m_8`.
#[test]
fn issue_250_two_generated_names_that_differ_only_in_case_are_e1003() {
    let src = "module M<const A: u32> { in x : u8  out y : u8  y = x + A }
module m<const A: u32> { in x : u8  out y : u8  y = x ^ A }
module Top { in x : u8  out a : u8  out b : u8
    let p = M<8> { x: x }
    let q = m<8> { x: x }
    a = p.y  b = q.y }";
    let r = p(src);
    assert_eq!(r.error_codes(), ["E1003"]);
    assert_eq!(primary_start(&r), at(src, "m<8> {"));
}

/// Etiketler iki adı söyler: bildirilen modülde `module '<ad>'`, generic
/// örnekte örneğin yazılışı ve kurduğu ad. Etiket metni kullanıcıya
/// görünen çıktıdır (ADR-0109 §4: yalnız tanı metnini değiştiren mutant
/// eşdeğer değildir).
#[test]
fn issue_250_labels_name_the_declared_module_and_the_instance() {
    let label = |r: &ParseResult, start: u32| -> String {
        r.diagnostics[0]
            .spans
            .iter()
            .find(|l| l.span.start == start)
            .unwrap_or_else(|| panic!("{start} konumunda etiket yok"))
            .label
            .clone()
    };
    let plain = "module Foo { in x : u8  out y : u8  y = x }
module foo { in x : u8  out y : u8  y = x }";
    let r = p(plain);
    assert_eq!(r.error_codes(), ["E1003"]);
    assert_eq!(label(&r, at(plain, "foo {")), "module 'foo'");
    assert_eq!(label(&r, at(plain, "Foo {")), "module 'Foo'");

    let generated = "module M<const A: u32> { in x : u8  out y : u8  y = x + A }
module m_8 { in x : u8  out y : u8  y = x }
module Top { in x : u8  out a : u8  out b : u8
    let p = M<8> { x: x }
    let q = m_8 { x: x }
    a = p.y  b = q.y }";
    let r = p(generated);
    assert_eq!(r.error_codes(), ["E1003"]);
    assert_eq!(label(&r, at(generated, "M<8> {")), "M<8> becomes 'M_8'");
    assert_eq!(label(&r, at(generated, "m_8 {")), "module 'm_8'");
    assert_eq!(
        label(&r, template_start(generated, "M")),
        "generic module 'M'"
    );
}

/// Hiç örneklenmeyen generic modül birimde kalır; adı da sayılır
/// (sv-mapping.md §9'daki tam ad kuralıyla aynı küme).
#[test]
fn issue_250_never_instantiated_generic_module_name_counts() {
    let src = "module foo<const A: u32> { in x : u8  out y : u8  y = x + A }
module Foo { in x : u8  out y : u8  y = x }";
    let r = p(src);
    assert_eq!(r.error_codes(), ["E1003"]);
    assert_eq!(primary_start(&r), at(src, "Foo {"));
}

/// Güvenli ikiz: adlar harf büyüklüğünden fazlasıyla ayrılıyor; örneklenen
/// şablonun adı (`bar`) öğe listesinden çıkar, dosya yazmaz, sayılmaz.
#[test]
fn issue_250_names_that_differ_beyond_case_are_accepted() {
    let src = "module Foo { in x : u8  out y : u8  y = x }
module Foo_2 { in x : u8  out y : u8  y = x }
module bar<const B: u32> { in x : u8  out y : u8  y = x + B }
module Bar { in x : u8  out y : u8  y = x }
module Top { in x : u8  out a : u8  out b : u8  out c : u8  out d : u8
    let p = Foo { x: x }
    let q = Foo_2 { x: x }
    let u = bar<1> { x: x }
    let v = Bar { x: x }
    a = p.y  b = q.y  c = u.y  d = v.y }";
    let r = p(src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    assert_eq!(
        module_names(&r.ast),
        vec!["Foo", "Foo_2", "bar_1", "Bar", "Top"]
    );
}

/// Aynı ad iki kez: çözümleyicinin E1003'ü (aynı kapsamda çift tanım);
/// harf denetimi ayrıştırıcıda ikinci bir tanı eklemez.
#[test]
fn issue_250_identical_names_are_left_to_the_resolver() {
    let src = "module Foo { in x : u8  out y : u8  y = x }
module Foo { in x : u8  out y : u8  y = x }";
    let r = p(src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
}

/// Bildirilen `M_8` ile `M<8>`'in kurduğu `M_8` aynı ad (çözümleyiciye
/// kalır); `m_8` ise ikisinden de yalnız harf büyüklüğüyle ayrılır.
/// Bildirilen `M_8` bir örneklemenin ürünü değildir: tanı onu kendi
/// bildiriminde gösterir, `M<8>` örneğinde değil. Kaynakta ilk ad
/// bildirilen `M_8`'dir; tanı sonraki ad `m_8`'de.
#[test]
fn issue_250_declared_module_with_a_generated_name_keeps_its_declaration_site() {
    let src = "module M<const A: u32> { in x : u8  out y : u8  y = x + A }
module M_8 { in x : u8  out y : u8  y = x }
module m_8 { in x : u8  out y : u8  y = x }
module Top { in x : u8  out a : u8  out b : u8  out c : u8
    let p = M<8> { x: x }
    let q = M_8 { x: x }
    let r = m_8 { x: x }
    a = p.y  b = q.y  c = r.y }";
    let r = p(src);
    assert_eq!(r.error_codes(), ["E1003"]);
    let d = &r.diagnostics[0];
    assert!(
        d.message
            .contains("module names 'M_8' and 'm_8' differ only in letter case"),
        "{}",
        d.message
    );
    assert_eq!(primary_start(&r), at(src, "m_8 {"));
    let labels: Vec<(u32, &str)> = d
        .spans
        .iter()
        .map(|l| (l.span.start, l.label.as_str()))
        .collect();
    assert_eq!(
        labels,
        [
            (at(src, "m_8 {"), "module 'm_8'"),
            (at(src, "M_8 {"), "module 'M_8'"),
        ]
    );
}

/// İki `@mmio` modülü: `first` ve `second` adlarıyla.
fn two_mmio(first: &str, second: &str) -> String {
    format!(
        "@mmio(base = 0x4000_0000, bus = AXI4Lite)
module {first} {{
    in  clk : clock
    out y   : bool
    @reg(offset = 0x00, access = ReadWrite)
    a : {{ b : bool, @reserved : bits<31> }}
    y = regs.a.b
}}
@mmio(base = 0x4001_0000, bus = AXI4Lite)
module {second} {{
    in  clk : clock
    out z   : bool
    @reg(offset = 0x00, access = ReadWrite)
    c : {{ d : bool, @reserved : bits<31> }}
    z = regs.c.d
}}"
    )
}

/// `GpioRegs` ve `GPIORegs` aynı sürücü dosyalarını yazar (`gpio_regs`):
/// `@mmio` açılımının E1014'ü raporlar; harf denetimi ikinci tanı eklemez.
#[test]
fn issue_250_mmio_pair_with_one_driver_stem_is_reported_once_as_e1014() {
    let r = p(&two_mmio("GpioRegs", "GPIORegs"));
    assert_eq!(r.error_codes(), ["E1014"]);
}

/// `AbC` → `ab_c`, `abc` → `abc`: sürücü kökleri ayrı, E1014 yok; ama
/// `rtl/AbC.sv` ile `rtl/abc.sv` harf duyarsız dosya sisteminde tek
/// dosyadır: E1003.
#[test]
fn issue_250_mmio_pair_with_two_driver_stems_is_e1003() {
    let src = two_mmio("AbC", "abc");
    let r = p(&src);
    assert_eq!(r.error_codes(), ["E1003"]);
    assert_eq!(primary_start(&r), at(&src, "abc {"));
}

/// #207: generic argüman literal olarak, değeriyle ikame edilir; 2^127 ve
/// üstü `i128::MAX`'e doyurulmaz. Düzeltmeden önce 2^127 + 1 klonda
/// 2^127 - 1 oluyordu. Parametre `uint<128>`: argüman ona sığar (#262'nin
/// denetimi geçer); 128 bitlik tamsayının desteği ayrıdır (sonraki
/// aşamada E0003), burada yalnız ayrıştırıcının ikamesi.
#[test]
fn issue_207_wide_generic_argument_is_substituted_with_its_value() {
    let src = r#"
module G<const K: uint<128>> {
    out o : bits<128>
    o = K as bits<128>
}
module Top {
    out a : bits<128>
    out b : bits<128>
    let g = G<0x8000_0000_0000_0000_0000_0000_0000_0001> { }
    let h = G<0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF> { }
    a = g.o
    b = h.o
}
"#;
    let r = p(src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    let substituted = |name: &str| {
        let m = module(&r.ast, name);
        let assign = m
            .body
            .iter()
            .find_map(|&s| match &r.ast.stmts[s].kind {
                StmtKind::Assign(a) => Some(a),
                _ => None,
            })
            .expect("atama");
        let ExprKind::Cast { expr, .. } = &r.ast.exprs[assign.rhs].kind else {
            panic!("K as bits<128> bekleniyor")
        };
        int_value(&r.ast, *expr)
    };
    let names = module_names(&r.ast);
    let g = names
        .iter()
        .find(|n| n.contains("170141183460469231731687303715884105729"));
    let h = names
        .iter()
        .find(|n| n.contains("340282366920938463463374607431768211455"));
    let (Some(g), Some(h)) = (g, h) else {
        panic!("klon adları argümanın değerini taşımalı: {names:?}")
    };
    assert_eq!(substituted(g), Some((1u128 << 127) + 1));
    assert_eq!(substituted(h), Some(u128::MAX));
}

/// #262: generic argüman parametrenin tipine sığmalı (E2010): `uN` için
/// 0..2^N, `iN` için 0..2^(N-1) (argüman negatif olamaz, E2008),
/// `uint<N>`/`sint<N>` N literalken aynı. Önceden denetlenmiyordu:
/// `G<2^80 - 1>` (`const K: u64`) kabul ediliyor, donanım 80 bitlik değer
/// görüyordu.
#[test]
fn issue_262_generic_argument_must_fit_the_parameter_type() {
    let src = |ty: &str, arg: &str| {
        format!(
            "module G<const K: {ty}> {{
    out o : bool
    o = true
}}
module Top {{
    out o : bool
    let g = G<{arg}> {{ }}
    o = g.o
}}
"
        )
    };
    for (ty, arg) in [
        ("u64", "0xFFFF_FFFF_FFFF_FFFF_FFFF"),
        ("u8", "256"),
        ("i8", "128"),
        ("u1", "2"),
        ("uint<4>", "16"),
        ("sint<4>", "8"),
    ] {
        let r = p(&src(ty, arg));
        assert_eq!(r.error_codes(), ["E2010"], "{ty} = {arg}");
    }
    for (ty, arg) in [
        ("u64", "0xFFFF_FFFF_FFFF_FFFF"),
        ("u8", "255"),
        ("i8", "127"),
        ("u1", "1"),
        ("uint<4>", "15"),
        ("sint<4>", "7"),
        ("u32", "0"),
    ] {
        let r = p(&src(ty, arg));
        assert!(
            r.diagnostics.is_empty(),
            "{ty} = {arg}: {:?}",
            r.error_codes()
        );
    }
}

/// #262: genişliği önceki bir const parametre olan `uint<W>`/`sint<W>`
/// parametresi de denetlenir; W'nin değeri o örneklemenin argümanıdır.
/// Önceden genişlik literal değilse denetim yoktu: `G<4, 16>`
/// (`const K: uint<W>`) kabul ediliyor, `K as bits<8>` 16 yazıyordu.
#[test]
fn issue_262_parameter_width_from_an_earlier_parameter_is_checked() {
    let src = |ty: &str, w: &str, k: &str| {
        format!(
            "module G<const W: u32, const K: {ty}> {{
    out o : bits<8>
    o = K as bits<8>
}}
module Top {{
    out o : bits<8>
    let g = G<{w}, {k}> {{ }}
    o = g.o
}}
"
        )
    };
    for (ty, w, k) in [
        ("uint<W>", "4", "16"),
        ("sint<W>", "4", "8"),
        ("uint<W>", "1", "2"),
    ] {
        let r = p(&src(ty, w, k));
        assert_eq!(r.error_codes(), ["E2010"], "{ty} W={w} K={k}");
    }
    for (ty, w, k) in [
        ("uint<W>", "4", "15"),
        ("sint<W>", "4", "7"),
        ("uint<W>", "8", "255"),
    ] {
        let r = p(&src(ty, w, k));
        assert!(
            r.diagnostics.is_empty(),
            "{ty} W={w} K={k}: {:?}",
            r.error_codes()
        );
    }
}
