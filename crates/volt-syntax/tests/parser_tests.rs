//! Volt parser birim testleri — F0 kapsamı.
//!
//! Öncelik vektörleri: docs/spec/operator-precedence.md §4 (BAĞLAYICI).
//! Kurtarma senaryoları: docs/spec/error-recovery.md §4, §8.

use volt_ast::{
    AttrArg, BlockContext, BlockStmt, ClockEdge, ContractKind, DomainKey, DomainValue, ElseBranch,
    Expr, ExprKind, GenericArg, GenericParamKind, Idx, IntSuffix, ItemKind, LValueSuffix,
    MatchArmBody, NumBase, OnTrigger, PatternArgs, PatternKind, PortDir, ResetPolarity, ResetSync,
    SourceFile, StmtKind, TypeRefKind, UseTree, VariantData, Visibility,
};
use volt_span::FileId;
use volt_syntax::parser::{parse, parse_expr, ParseResult};

fn p(src: &str) -> ParseResult {
    parse(FileId(0), src)
}

fn codes(src: &str) -> Vec<&'static str> {
    p(src).error_codes()
}

/// Hatasız ayrışması beklenen ifadeyi S-ifade dökümüne çevirir —
/// öncelik vektörleri bununla doğrulanır.
fn expr_clean_sexp(src: &str) -> String {
    let (result, root) = parse_expr(FileId(0), src);
    assert!(
        result.diagnostics.is_empty(),
        "beklenmeyen tanılar ({src}): {:?}",
        result.error_codes()
    );
    dump(&result.ast, root)
}

fn dump(ast: &SourceFile, idx: Idx<Expr>) -> String {
    match &ast.exprs[idx].kind {
        ExprKind::IntLit { value, .. } => value.to_string(),
        ExprKind::BoolLit(b) => b.to_string(),
        ExprKind::Path(path) => path
            .segments
            .iter()
            .map(|n| n.text.clone())
            .collect::<Vec<_>>()
            .join("::"),
        ExprKind::Binary { op, lhs, rhs } => {
            format!("({} {} {})", op.symbol(), dump(ast, *lhs), dump(ast, *rhs))
        }
        ExprKind::Unary { op, operand } => format!("({} {})", op.symbol(), dump(ast, *operand)),
        ExprKind::Index { base, index } => {
            format!("(index {} {})", dump(ast, *base), dump(ast, *index))
        }
        ExprKind::Range { base, hi, lo } => format!(
            "(range {} {} {})",
            dump(ast, *base),
            dump(ast, *hi),
            dump(ast, *lo)
        ),
        ExprKind::PartSelect {
            base,
            start,
            width,
            ascending,
        } => format!(
            "({} {} {} {})",
            if *ascending { "+:" } else { "-:" },
            dump(ast, *base),
            dump(ast, *start),
            dump(ast, *width)
        ),
        ExprKind::Field { base, field } => {
            format!("(field {} {})", dump(ast, *base), field.text)
        }
        ExprKind::Call { callee, args } => {
            let args: Vec<String> = args.iter().map(|a| dump(ast, *a)).collect();
            format!("(call {} {})", dump(ast, *callee), args.join(" "))
        }
        ExprKind::Cast { expr, ty } => {
            format!("(as {} {})", dump(ast, *expr), type_dump(ast, *ty))
        }
        ExprKind::If {
            cond,
            then_expr,
            else_expr,
        } => format!(
            "(if {} {} {})",
            dump(ast, *cond),
            dump(ast, *then_expr),
            dump(ast, *else_expr)
        ),
        ExprKind::StringLit(s) => format!("{s:?}"),
        ExprKind::Match { scrutinee, arms } => {
            format!("(match {} {} kol)", dump(ast, *scrutinee), arms.len())
        }
        ExprKind::StructLit { path, fields } => format!(
            "(struct {} {} alan)",
            path.segments
                .iter()
                .map(|n| n.text.clone())
                .collect::<Vec<_>>()
                .join("::"),
            fields.len()
        ),
        ExprKind::ArrayLit(volt_ast::ArrayLitKind::List(elems)) => {
            let elems: Vec<String> = elems.iter().map(|e| dump(ast, *e)).collect();
            format!("[{}]", elems.join(" "))
        }
        ExprKind::ArrayLit(volt_ast::ArrayLitKind::Repeat { value, count }) => {
            format!("[{}; {}]", dump(ast, *value), dump(ast, *count))
        }
        ExprKind::TupleLit(elems) => {
            let elems: Vec<String> = elems.iter().map(|e| dump(ast, *e)).collect();
            format!("(tuple {})", elems.join(" "))
        }
        ExprKind::Todo { .. } => "todo!".to_string(),
        ExprKind::Error => "<err>".to_string(),
    }
}

fn type_dump(ast: &SourceFile, idx: Idx<volt_ast::TypeRef>) -> String {
    match &ast.types[idx].kind {
        TypeRefKind::Bool => "bool".into(),
        TypeRefKind::Clock => "clock".into(),
        TypeRefKind::Reset(_) => "reset".into(),
        TypeRefKind::UInt(n) => format!("u{n}"),
        TypeRefKind::SInt(n) => format!("i{n}"),
        TypeRefKind::Bits(e) => format!("bits<{}>", dump(ast, *e)),
        TypeRefKind::Trit => "Trit".into(),
        TypeRefKind::Array { elem, len } => {
            format!("[{}; {}]", type_dump(ast, *elem), dump(ast, *len))
        }
        TypeRefKind::Tuple(elems) => {
            let elems: Vec<String> = elems.iter().map(|t| type_dump(ast, *t)).collect();
            format!("({})", elems.join(", "))
        }
        TypeRefKind::Path { path, args } => {
            let name = path
                .segments
                .iter()
                .map(|n| n.text.clone())
                .collect::<Vec<_>>()
                .join("::");
            if args.is_empty() {
                name
            } else {
                format!("{name}<{} arg>", args.len())
            }
        }
        TypeRefKind::Error => "<err>".into(),
    }
}

// ═══ Öncelik vektörleri (operator-precedence.md §4 — 15 vektör) ═══

#[test]
fn prec_01_mul_binds_tighter_than_add() {
    assert_eq!(expr_clean_sexp("a + b * c"), "(+ a (* b c))");
}

#[test]
fn prec_02_mul_on_left_of_add() {
    assert_eq!(expr_clean_sexp("a * b + c"), "(+ (* a b) c)");
}

#[test]
fn prec_03_sub_left_assoc() {
    assert_eq!(expr_clean_sexp("a - b - c"), "(- (- a b) c)");
}

#[test]
fn prec_04_and_binds_tighter_than_or() {
    assert_eq!(expr_clean_sexp("a && b || c"), "(|| (&& a b) c)");
}

#[test]
fn prec_05_or_with_and_on_rhs() {
    assert_eq!(expr_clean_sexp("a || b && c"), "(|| a (&& b c))");
}

#[test]
fn prec_06_bitand_binds_tighter_than_eq() {
    assert_eq!(expr_clean_sexp("a & MASK == 0"), "(== (& a MASK) 0)");
}

#[test]
fn prec_07_shift_looser_than_add() {
    // Yapı spec §4 vektörüne uyar; aynı ifade spec §5 gereği W0010
    // uyarısı ürettiği için burada yalnız uyarıya izin verilir.
    let (result, root) = parse_expr(FileId(0), "a << 2 + 1");
    assert_eq!(dump(&result.ast, root), "(<< a (+ 2 1))");
    assert_eq!(result.error_codes(), vec!["W0010"]);
}

#[test]
fn prec_08_not_binds_tighter_than_and() {
    assert_eq!(expr_clean_sexp("!a && b"), "(&& (! a) b)");
}

#[test]
fn prec_09_neg_binds_tighter_than_add() {
    assert_eq!(expr_clean_sexp("-a + b"), "(+ (- a) b)");
}

#[test]
fn prec_10_cast_binds_tighter_than_add() {
    assert_eq!(expr_clean_sexp("a as u16 + b"), "(+ (as a u16) b)");
}

#[test]
fn prec_11_not_of_index() {
    assert_eq!(expr_clean_sexp("!a[0]"), "(! (index a 0))");
}

#[test]
fn prec_12_field_chain_left_assoc() {
    assert_eq!(expr_clean_sexp("a.b.c"), "(field (field a b) c)");
}

#[test]
fn prec_13_paren_overrides() {
    assert_eq!(expr_clean_sexp("(a + b) * c"), "(* (+ a b) c)");
}

#[test]
fn prec_14_lt_chain_is_e0010() {
    let (result, _) = parse_expr(FileId(0), "a < b < c");
    assert!(
        result.error_codes().contains(&"E0010"),
        "{:?}",
        result.error_codes()
    );
}

#[test]
fn prec_15_eq_chain_is_e0010() {
    let (result, _) = parse_expr(FileId(0), "a == b == c");
    assert!(
        result.error_codes().contains(&"E0010"),
        "{:?}",
        result.error_codes()
    );
}

// ═══ İmplikasyon operatörü (operator-precedence.md §4, ADR-0034) ══

#[test]
fn prec_16_implication_right_assoc() {
    assert_eq!(expr_clean_sexp("a -> b -> c"), "(-> a (-> b c))");
}

#[test]
fn prec_17_or_binds_tighter_than_implication() {
    assert_eq!(expr_clean_sexp("a || b -> c"), "(-> (|| a b) c)");
}

#[test]
fn impl_rhs_or_binds_tighter() {
    assert_eq!(expr_clean_sexp("a -> b || c"), "(-> a (|| b c))");
}

#[test]
fn impl_not_binds_tighter() {
    assert_eq!(expr_clean_sexp("!a -> b"), "(-> (! a) b)");
}

#[test]
fn impl_and_binds_tighter() {
    assert_eq!(expr_clean_sexp("a && b -> c"), "(-> (&& a b) c)");
}

#[test]
fn impl_comparison_operand() {
    assert_eq!(expr_clean_sexp("a == 0 -> b < c"), "(-> (== a 0) (< b c))");
}

#[test]
fn impl_paren_overrides_right_assoc() {
    assert_eq!(expr_clean_sexp("(a -> b) -> c"), "(-> (-> a b) c)");
}

#[test]
fn fn_return_arrow_unaffected_by_implication() {
    // fn imzasındaki '->' dönüş tipidir; gövdedeki '->' implikasyondur.
    let result = p("fn imp(a: bool, b: bool) -> bool { a -> b }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let ItemKind::Fn(f) = &result.ast.items_arena[result.ast.items[0]].kind else {
        panic!("fn bekleniyor")
    };
    assert!(f.return_ty.is_some(), "dönüş tipi korunmalı");
    let block = &result.ast.blocks[f.body];
    let tail = block.tail.expect("son ifade olmalı");
    assert!(matches!(
        result.ast.exprs[tail].kind,
        ExprKind::Binary {
            op: volt_ast::BinOp::Imp,
            ..
        }
    ));
}

#[test]
fn fn_contract_implication_with_return_arrow() {
    // Aynı imzada hem dönüş '->' hem kontrat implikasyonu sorunsuz.
    let result = p("fn g(a: bool, b: bool) -> bool requires: a -> b { b }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
}

#[test]
fn module_contract_implication_parses() {
    let result = p(
        "module M {\n    in  clk : clock\n    in  a : bool\n    out q : bool\n\n    \
                    invariant: !a -> q\n\n    q = a\n}",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
}

#[test]
fn impl_missing_rhs_recovers() {
    let (result, _) = parse_expr(FileId(0), "a ->");
    assert!(!result.diagnostics.is_empty(), "sağ operand eksik");
}

// ═══ Ek ifade testleri ════════════════════════════════════════════

#[test]
fn expr_cast_applies_before_unary_neg() {
    // operator-precedence.md §2.5: -a as i16 → -(a as i16)
    assert_eq!(expr_clean_sexp("-a as i16"), "(- (as a i16))");
}

#[test]
fn expr_double_not() {
    assert_eq!(expr_clean_sexp("!!a"), "(! (! a))");
}

#[test]
fn expr_binary_minus_then_unary_minus() {
    assert_eq!(expr_clean_sexp("a - -b"), "(- a (- b))");
}

#[test]
fn expr_bit_range() {
    assert_eq!(expr_clean_sexp("a[7:4]"), "(range a 7 4)");
}

#[test]
fn expr_index_chain() {
    assert_eq!(expr_clean_sexp("a[0][1]"), "(index (index a 0) 1)");
}

#[test]
fn expr_call_with_args() {
    assert_eq!(expr_clean_sexp("f(a, b)"), "(call f a b)");
}

#[test]
fn expr_int_suffix_parsed() {
    let (result, root) = parse_expr(FileId(0), "42u8");
    match &result.ast.exprs[root].kind {
        ExprKind::IntLit {
            value,
            suffix,
            base,
        } => {
            assert_eq!(*value, 42);
            assert_eq!(*suffix, Some(IntSuffix::U8));
            assert_eq!(*base, NumBase::Dec);
        }
        other => panic!("IntLit bekleniyor: {other:?}"),
    }
}

#[test]
fn expr_hex_value() {
    let (result, root) = parse_expr(FileId(0), "0xFF");
    match &result.ast.exprs[root].kind {
        ExprKind::IntLit { value, base, .. } => {
            assert_eq!(*value, 255);
            assert_eq!(*base, NumBase::Hex);
        }
        other => panic!("IntLit bekleniyor: {other:?}"),
    }
}

#[test]
fn expr_binary_with_underscore() {
    let (result, root) = parse_expr(FileId(0), "0b1010_1010");
    match &result.ast.exprs[root].kind {
        ExprKind::IntLit { value, base, .. } => {
            assert_eq!(*value, 170);
            assert_eq!(*base, NumBase::Bin);
        }
        other => panic!("IntLit bekleniyor: {other:?}"),
    }
}

#[test]
fn expr_bool_literal() {
    assert_eq!(expr_clean_sexp("true"), "true");
    assert_eq!(expr_clean_sexp("false"), "false");
}

#[test]
fn expr_if_else() {
    assert_eq!(expr_clean_sexp("if c { 1 } else { 2 }"), "(if c 1 2)");
}

#[test]
fn expr_if_else_if_chain() {
    assert_eq!(
        expr_clean_sexp("if a { 1 } else if b { 2 } else { 3 }"),
        "(if a 1 (if b 2 3))"
    );
}

#[test]
fn expr_if_missing_else_is_e0008() {
    let (result, root) = parse_expr(FileId(0), "if c { 1 }");
    assert!(
        result.error_codes().contains(&"E0008"),
        "{:?}",
        result.error_codes()
    );
    // Ağaç yine üretilmeli — else dalı Error
    assert_eq!(dump(&result.ast, root), "(if c 1 <err>)");
}

#[test]
fn expr_missing_rhs_recovers_with_error_node() {
    let (result, root) = parse_expr(FileId(0), "a + ");
    assert!(
        result.error_codes().contains(&"E0011"),
        "{:?}",
        result.error_codes()
    );
    assert_eq!(dump(&result.ast, root), "(+ a <err>)");
}

#[test]
fn expr_path_with_coloncolon() {
    assert_eq!(expr_clean_sexp("State::Idle"), "State::Idle");
}

// ═══ counter.volt tam AST ═════════════════════════════════════════

fn counter_src() -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/counter.volt"
    );
    std::fs::read_to_string(path).expect("counter.volt okunamalı")
}

#[test]
fn counter_full_ast_structure() {
    let result = p(&counter_src());
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    assert_eq!(result.ast.items.len(), 1);

    let module = result.ast.module(0).expect("modül bekleniyor");
    assert_eq!(module.name.text, "Counter");
    assert_eq!(module.ports.len(), 3);
    assert_eq!(module.body.len(), 3); // reg, on, assign

    // Portlar: in clk : clock, in enable : bool, out count : u8
    assert_eq!(module.ports[0].name.text, "clk");
    assert_eq!(module.ports[0].direction, PortDir::In);
    assert!(matches!(
        result.ast.types[module.ports[0].ty].kind,
        TypeRefKind::Clock
    ));
    assert_eq!(module.ports[1].name.text, "enable");
    assert!(matches!(
        result.ast.types[module.ports[1].ty].kind,
        TypeRefKind::Bool
    ));
    assert_eq!(module.ports[2].name.text, "count");
    assert_eq!(module.ports[2].direction, PortDir::Out);
    assert!(matches!(
        result.ast.types[module.ports[2].ty].kind,
        TypeRefKind::UInt(8)
    ));

    // Deyim türleri sırayla: Reg, On, Assign
    assert!(matches!(
        result.ast.stmts[module.body[0]].kind,
        StmtKind::Reg(_)
    ));
    assert!(matches!(
        result.ast.stmts[module.body[1]].kind,
        StmtKind::On(_)
    ));
    assert!(matches!(
        result.ast.stmts[module.body[2]].kind,
        StmtKind::Assign(_)
    ));
}

#[test]
fn counter_reg_details() {
    let result = p(&counter_src());
    let module = result.ast.module(0).unwrap();
    let StmtKind::Reg(reg) = &result.ast.stmts[module.body[0]].kind else {
        panic!("reg bekleniyor")
    };
    assert_eq!(reg.name.text, "count_r");
    assert!(reg.domain.is_none());
    assert!(matches!(
        result.ast.types[reg.ty.expect("tip bekleniyor")].kind,
        TypeRefKind::UInt(8)
    ));
    assert!(matches!(
        result.ast.exprs[reg.init].kind,
        ExprKind::IntLit { value: 0, .. }
    ));
}

#[test]
fn counter_on_block_structure() {
    let result = p(&counter_src());
    let module = result.ast.module(0).unwrap();
    let StmtKind::On(on) = &result.ast.stmts[module.body[1]].kind else {
        panic!("on bekleniyor")
    };
    let OnTrigger::Clock(name) = &on.trigger else {
        panic!("saat tetiği bekleniyor")
    };
    assert_eq!(name.text, "clk");

    let block = &result.ast.blocks[on.body];
    assert_eq!(block.stmts.len(), 1);
    let BlockStmt::If(if_stmt) = &block.stmts[0] else {
        panic!("if bekleniyor")
    };
    assert!(if_stmt.else_branch.is_none());

    let then = &result.ast.blocks[if_stmt.then_block];
    assert_eq!(then.stmts.len(), 1);
    let BlockStmt::NonBlockAssign { lhs, rhs, .. } = &then.stmts[0] else {
        panic!("nonblocking atama bekleniyor")
    };
    assert_eq!(lhs.base.text, "count_r");
    assert_eq!(dump(&result.ast, *rhs), "(+ count_r 1)");
}

#[test]
fn counter_final_assign() {
    let result = p(&counter_src());
    let module = result.ast.module(0).unwrap();
    let StmtKind::Assign(assign) = &result.ast.stmts[module.body[2]].kind else {
        panic!("assign bekleniyor")
    };
    assert_eq!(assign.lhs.base.text, "count");
    assert_eq!(dump(&result.ast, assign.rhs), "count_r");
}

#[test]
fn counter_module_doc_attached() {
    let result = p(&counter_src());
    let item = &result.ast.items_arena[result.ast.items[0]];
    let doc = item.doc.as_ref().expect("doc bekleniyor");
    assert!(doc.contains("8-bit"), "doc: {doc}");
}

// ═══ Modül ve domain ══════════════════════════════════════════════

#[test]
fn module_empty() {
    let result = p("module M {}");
    assert!(result.diagnostics.is_empty());
    let module = result.ast.module(0).unwrap();
    assert_eq!(module.name.text, "M");
    assert!(module.ports.is_empty());
    assert!(module.body.is_empty());
}

#[test]
fn module_closing_name_matches() {
    let result = p("module M { } module M");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    assert_eq!(result.ast.items.len(), 1);
    assert_eq!(
        result
            .ast
            .module(0)
            .unwrap()
            .closing_name
            .as_ref()
            .unwrap()
            .text,
        "M"
    );
}

#[test]
fn module_closing_name_mismatch_is_e0004() {
    assert!(codes("module A { } module B").contains(&"E0004"));
}

#[test]
fn two_modules_not_confused_with_closing_name() {
    let result = p("module A {} module B {}");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    assert_eq!(result.ast.items.len(), 2);
    assert_eq!(result.ast.module(0).unwrap().name.text, "A");
    assert_eq!(result.ast.module(1).unwrap().name.text, "B");
}

#[test]
fn module_missing_name_still_parses_body() {
    let result = p("module { in a : u8 }");
    assert!(result.error_codes().contains(&"E0001"));
    let module = result.ast.module(0).expect("modül düğümü yine üretilmeli");
    assert_eq!(module.ports.len(), 1);
}

#[test]
fn domain_decl_fields() {
    let result = p("domain Fast { clock = posedge, reset = sync active_high }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let domain = result.ast.domain(0).unwrap();
    assert_eq!(domain.name.text, "Fast");
    assert_eq!(domain.fields.len(), 2);
    assert_eq!(domain.fields[0].key, DomainKey::Clock);
    assert!(matches!(
        domain.fields[0].value,
        DomainValue::ClockEdge(ClockEdge::Posedge)
    ));
    assert_eq!(domain.fields[1].key, DomainKey::Reset);
    match &domain.fields[1].value {
        DomainValue::Reset(spec) => {
            assert_eq!(spec.sync, ResetSync::Sync);
            assert_eq!(spec.polarity, ResetPolarity::ActiveHigh);
        }
        other => panic!("reset spec bekleniyor: {other:?}"),
    }
}

#[test]
fn domain_negedge_and_async_low() {
    let result = p("domain D { clock = negedge, reset = async active_low }");
    assert!(result.diagnostics.is_empty());
    let domain = result.ast.domain(0).unwrap();
    assert!(matches!(
        domain.fields[0].value,
        DomainValue::ClockEdge(ClockEdge::Negedge)
    ));
    match &domain.fields[1].value {
        DomainValue::Reset(spec) => {
            assert_eq!(spec.sync, ResetSync::Async);
            assert_eq!(spec.polarity, ResetPolarity::ActiveLow);
        }
        other => panic!("reset spec bekleniyor: {other:?}"),
    }
}

#[test]
fn domain_unknown_key_is_w0020() {
    let result = p("domain D { hiz = posedge }");
    assert!(result.error_codes().contains(&"W0020"));
    let domain = result.ast.domain(0).unwrap();
    assert!(matches!(domain.fields[0].key, DomainKey::Unknown(_)));
}

// ═══ Portlar ve tipler ════════════════════════════════════════════

#[test]
fn port_with_domain_annotation() {
    let result = p("module M { in data : u8 @Fast }");
    assert!(result.diagnostics.is_empty());
    let module = result.ast.module(0).unwrap();
    assert_eq!(module.ports[0].domain.as_ref().unwrap().text, "Fast");
}

#[test]
fn port_inout_direction() {
    let result = p("module M { inout pad : bool }");
    assert!(result.diagnostics.is_empty());
    assert_eq!(
        result.ast.module(0).unwrap().ports[0].direction,
        PortDir::InOut
    );
}

#[test]
fn port_missing_type_recovers_next_port_and_stmt() {
    // ast-nodes.md §14 kurtarma testi
    let result = p("module Foo { in a : ; out b : u8  b = a }");
    assert_eq!(result.diagnostics.len(), 1, "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    assert_eq!(module.ports.len(), 2);
    assert!(matches!(
        result.ast.types[module.ports[0].ty].kind,
        TypeRefKind::Error
    ));
    assert!(matches!(
        result.ast.types[module.ports[1].ty].kind,
        TypeRefKind::UInt(8)
    ));
}

#[test]
fn all_integer_port_types() {
    let result = p("module M { in a : u8 in b : u16 in c : u32 in d : u64 \
         in e : i8 in f : i16 in g : i32 in h : i64 }");
    assert!(result.diagnostics.is_empty());
    let module = result.ast.module(0).unwrap();
    let widths: Vec<_> = module
        .ports
        .iter()
        .map(|p| match result.ast.types[p.ty].kind {
            TypeRefKind::UInt(n) => (false, n),
            TypeRefKind::SInt(n) => (true, n),
            _ => panic!("tam sayı tipi bekleniyor"),
        })
        .collect();
    assert_eq!(
        widths,
        vec![
            (false, 8),
            (false, 16),
            (false, 32),
            (false, 64),
            (true, 8),
            (true, 16),
            (true, 32),
            (true, 64)
        ]
    );
}

#[test]
fn bits_type_with_const_expr() {
    let result = p("module M { in a : bits<8> in b : bits<4 + 4> }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let TypeRefKind::Bits(n) = result.ast.types[module.ports[0].ty].kind else {
        panic!("bits bekleniyor")
    };
    assert!(matches!(
        result.ast.exprs[n].kind,
        ExprKind::IntLit { value: 8, .. }
    ));
    let TypeRefKind::Bits(n2) = result.ast.types[module.ports[1].ty].kind else {
        panic!("bits bekleniyor")
    };
    assert_eq!(dump(&result.ast, n2), "(+ 4 4)");
}

#[test]
fn trit_and_reset_types() {
    let result = p("module M { in t : Trit in r : reset }");
    assert!(result.diagnostics.is_empty());
    let module = result.ast.module(0).unwrap();
    assert!(matches!(
        result.ast.types[module.ports[0].ty].kind,
        TypeRefKind::Trit
    ));
    assert!(matches!(
        result.ast.types[module.ports[1].ty].kind,
        TypeRefKind::Reset(None)
    ));
}

// ═══ Deyimler ═════════════════════════════════════════════════════

#[test]
fn reg_with_explicit_domain() {
    let result = p("module M { reg(clk) r : u8 = 0 }");
    assert!(result.diagnostics.is_empty());
    let module = result.ast.module(0).unwrap();
    let StmtKind::Reg(reg) = &result.ast.stmts[module.body[0]].kind else {
        panic!()
    };
    assert_eq!(reg.domain.as_ref().unwrap().text, "clk");
}

#[test]
fn reg_without_type_annotation() {
    let result = p("module M { reg r = 0 }");
    assert!(result.diagnostics.is_empty());
    let module = result.ast.module(0).unwrap();
    let StmtKind::Reg(reg) = &result.ast.stmts[module.body[0]].kind else {
        panic!()
    };
    assert!(reg.ty.is_none());
}

#[test]
fn let_with_type_in_module_body() {
    let result = p("module M { let x : u16 = 5 }");
    assert!(result.diagnostics.is_empty());
    let module = result.ast.module(0).unwrap();
    let StmtKind::Let(decl) = &result.ast.stmts[module.body[0]].kind else {
        panic!()
    };
    assert_eq!(decl.name.text, "x");
    assert!(matches!(
        result.ast.types[decl.ty.unwrap()].kind,
        TypeRefKind::UInt(16)
    ));
}

#[test]
fn on_reset_trigger() {
    let result = p("module M { in clk : clock reg r = 0 on clk.reset { r <= 0 } }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let StmtKind::On(on) = &result.ast.stmts[module.body[1]].kind else {
        panic!()
    };
    let OnTrigger::Reset(name) = &on.trigger else {
        panic!("reset tetiği bekleniyor")
    };
    assert_eq!(name.text, "clk");
}

#[test]
fn module_level_assign_with_semicolon() {
    let result = p("module M { out o : u8 o = 1; }");
    assert!(result.diagnostics.is_empty());
}

#[test]
fn lvalue_index_assign() {
    let result = p("module M { in clk : clock on clk { a[3] <= 1 } }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let StmtKind::On(on) = &result.ast.stmts[module.body[0]].kind else {
        panic!()
    };
    let BlockStmt::NonBlockAssign { lhs, .. } = &result.ast.blocks[on.body].stmts[0] else {
        panic!()
    };
    assert_eq!(lhs.base.text, "a");
    assert!(matches!(lhs.suffixes[0], LValueSuffix::Index(_)));
}

#[test]
fn lvalue_range_assign() {
    let result = p("module M { in clk : clock on clk { a[7:4] <= 1 } }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let StmtKind::On(on) = &result.ast.stmts[module.body[0]].kind else {
        panic!()
    };
    let BlockStmt::NonBlockAssign { lhs, .. } = &result.ast.blocks[on.body].stmts[0] else {
        panic!()
    };
    assert!(matches!(lhs.suffixes[0], LValueSuffix::Range { .. }));
}

#[test]
fn lvalue_field_assign() {
    let result = p("module M { in clk : clock on clk { a.b <= 1 } }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let StmtKind::On(on) = &result.ast.stmts[module.body[0]].kind else {
        panic!()
    };
    let BlockStmt::NonBlockAssign { lhs, .. } = &result.ast.blocks[on.body].stmts[0] else {
        panic!()
    };
    match &lhs.suffixes[0] {
        LValueSuffix::Field(name) => assert_eq!(name.text, "b"),
        other => panic!("alan soneki bekleniyor: {other:?}"),
    }
}

#[test]
fn if_else_if_chain_in_block() {
    let result =
        p("module M { in clk : clock on clk { if a { x <= 1 } else if b { x <= 2 } else { x <= 3 } } }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let StmtKind::On(on) = &result.ast.stmts[module.body[0]].kind else {
        panic!()
    };
    let BlockStmt::If(if_stmt) = &result.ast.blocks[on.body].stmts[0] else {
        panic!()
    };
    let Some(ElseBranch::If(elif)) = &if_stmt.else_branch else {
        panic!("else-if bekleniyor")
    };
    assert!(matches!(elif.else_branch, Some(ElseBranch::Block(_))));
}

// ═══ E0006 / E0007 blok bağlamı ═══════════════════════════════════

#[test]
fn e0006_eq_in_sequential_block() {
    let result = p("module M { in clk : clock on clk { r = r + 1 } }");
    assert!(
        result.error_codes().contains(&"E0006"),
        "{:?}",
        result.error_codes()
    );
    // Ayrıştırma '<=' gibi devam etmeli
    let module = result.ast.module(0).unwrap();
    let StmtKind::On(on) = &result.ast.stmts[module.body[0]].kind else {
        panic!()
    };
    assert!(matches!(
        result.ast.blocks[on.body].stmts[0],
        BlockStmt::NonBlockAssign { .. }
    ));
}

#[test]
fn e0007_le_in_combinational_block() {
    // F1: comb gerçek bir blok — E0003 YOK, yalnız E0007
    let result = p("module M { comb { r <= 1 } }");
    let codes = result.error_codes();
    assert!(!codes.contains(&"E0003"), "comb artık gerçek: {codes:?}");
    assert!(codes.contains(&"E0007"), "'<=' E0007 üretmeli: {codes:?}");
}

#[test]
fn eq_in_combinational_is_clean() {
    // comb gerçek blok; '=' bağlam açısından doğru — tanı YOK
    let result = p("module M { comb { r = 1 } }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
}

// ═══ Bağlamsal anahtar kelimeler: sync/async (ADR-0023) ═══════════

#[test]
fn sync_call_is_normal_call_expr() {
    // 13_cdc_correct_bridge.volt deseni — sync() sıradan çağrı
    let result = p("module M { in f : bool out s : bool s = sync(f, clk) }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let StmtKind::Assign(assign) = &result.ast.stmts[module.body[0]].kind else {
        panic!("atama bekleniyor")
    };
    assert!(matches!(
        result.ast.exprs[assign.rhs].kind,
        ExprKind::Call { .. }
    ));
}

#[test]
fn sync_and_async_are_valid_identifiers() {
    // ADR-0023: reset = konumu dışında sıradan Ident
    let result = p("module M { let sync = 1 let async = 2 }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
}

#[test]
fn reset_sync_spec_still_parses_in_domain() {
    let result = p("domain D { clock = posedge reset = async active_low }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let domain = result.ast.domain(0).unwrap();
    let DomainValue::Reset(spec) = &domain.fields[1].value else {
        panic!("ResetSpec bekleniyor")
    };
    assert_eq!(spec.sync, ResetSync::Async);
    assert_eq!(spec.polarity, ResetPolarity::ActiveLow);
}

#[test]
fn sync_in_non_reset_domain_field_is_literal() {
    // 'reset =' dışında sync bir Ident/ifadedir, ResetSpec değil
    let result = p("domain D { frequency = sync }");
    let domain = result.ast.domain(0).unwrap();
    assert!(matches!(domain.fields[0].value, DomainValue::Literal(_)));
}

// ═══ package / use / pub ══════════════════════════════════════════

#[test]
fn package_decl_parsed() {
    let result = p("package cip::alt_sistem;\nmodule M { }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let pkg = result.ast.package.as_ref().expect("package bekleniyor");
    let names: Vec<&str> = pkg.path.segments.iter().map(|s| s.text.as_str()).collect();
    assert_eq!(names, ["cip", "alt_sistem"]);
}

#[test]
fn duplicate_package_is_error() {
    let result = p("package a; package b;");
    assert!(result.error_codes().contains(&"E0001"));
}

#[test]
fn use_decl_variants() {
    let result = p("use a::b;\nuse c::*;\nuse d::{e, f::g};\nuse h::i as j;\nmodule M { }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    assert_eq!(result.ast.uses.len(), 4);
    assert!(result.ast.uses[0].tree.is_none());
    assert!(matches!(result.ast.uses[1].tree, Some(UseTree::Glob)));
    let Some(UseTree::List(paths)) = &result.ast.uses[2].tree else {
        panic!("liste bekleniyor")
    };
    assert_eq!(paths.len(), 2);
    let Some(UseTree::Alias(alias)) = &result.ast.uses[3].tree else {
        panic!("takma ad bekleniyor")
    };
    assert_eq!(alias.text, "j");
}

#[test]
fn pub_visibility_recorded() {
    let result = p("pub module M { } module N { }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    assert_eq!(
        result.ast.items_arena[result.ast.items[0]].visibility,
        Visibility::Public
    );
    assert_eq!(
        result.ast.items_arena[result.ast.items[1]].visibility,
        Visibility::Private
    );
}

// ═══ fn öğesi ═════════════════════════════════════════════════════

#[test]
fn fn_with_params_return_and_tail() {
    let result = p("fn parity(x: u8, y: u8) -> bool { x ^ y == 0 }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let ItemKind::Fn(f) = &result.ast.items_arena[result.ast.items[0]].kind else {
        panic!("fn bekleniyor")
    };
    assert_eq!(f.name.text, "parity");
    assert_eq!(f.params.len(), 2);
    assert_eq!(f.params[0].name.text, "x");
    assert!(f.return_ty.is_some());
    let block = &result.ast.blocks[f.body];
    assert_eq!(block.context, BlockContext::Function);
    assert!(block.tail.is_some(), "son ifade dönüş değeri olmalı");
}

#[test]
fn fn_with_let_bindings_before_tail() {
    let result = p("fn f(a: u8) -> u8 { let b = a + 1 let c = b * 2 c }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let ItemKind::Fn(f) = &result.ast.items_arena[result.ast.items[0]].kind else {
        panic!()
    };
    let block = &result.ast.blocks[f.body];
    assert_eq!(block.stmts.len(), 2);
    assert!(block.tail.is_some());
}

#[test]
fn fn_with_contracts() {
    let result = p("fn div(a: u8, b: u8) -> u8 requires: b != 0 ensures: true { a / b }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let ItemKind::Fn(f) = &result.ast.items_arena[result.ast.items[0]].kind else {
        panic!()
    };
    assert_eq!(f.contracts.len(), 2);
    assert_eq!(f.contracts[0].kind, ContractKind::Requires);
    assert_eq!(f.contracts[1].kind, ContractKind::Ensures);
}

#[test]
fn fn_generic_params() {
    let result = p("fn f<const N: u32>(a: bits<N>) -> bool { a[0] }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let ItemKind::Fn(f) = &result.ast.items_arena[result.ast.items[0]].kind else {
        panic!()
    };
    assert_eq!(f.generics.len(), 1);
    assert!(matches!(f.generics[0].kind, GenericParamKind::Const { .. }));
}

// ═══ struct / enum / type / const / extern ════════════════════════

#[test]
fn struct_with_fields() {
    let result = p("struct Nokta { x: u8, y: u8 }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let ItemKind::Struct(s) = &result.ast.items_arena[result.ast.items[0]].kind else {
        panic!("struct bekleniyor")
    };
    assert_eq!(s.name.text, "Nokta");
    assert!(!s.is_port);
    assert_eq!(s.fields.len(), 2);
    assert_eq!(s.fields[1].name.text, "y");
}

#[test]
fn struct_port_modifier() {
    let result = p("struct port Eksen { veri: u8 }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let ItemKind::Struct(s) = &result.ast.items_arena[result.ast.items[0]].kind else {
        panic!()
    };
    assert!(s.is_port);
    assert_eq!(s.name.text, "Eksen");
}

#[test]
fn enum_with_repr_and_discriminants() {
    let result = p("enum Durum : bits<2> { Bekle = 0, Calis = 1, Bitti }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let ItemKind::Enum(e) = &result.ast.items_arena[result.ast.items[0]].kind else {
        panic!("enum bekleniyor")
    };
    assert!(e.repr.is_some());
    assert_eq!(e.variants.len(), 3);
    assert!(e.variants[0].discriminant.is_some());
    assert!(e.variants[2].discriminant.is_none());
}

#[test]
fn enum_tuple_and_struct_variants() {
    let result = p("enum Paket { Bos, Veri(u8, u8), Cerceve { bas: u8 } }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let ItemKind::Enum(e) = &result.ast.items_arena[result.ast.items[0]].kind else {
        panic!()
    };
    assert!(matches!(e.variants[0].data, VariantData::Unit));
    let VariantData::Tuple(tys) = &e.variants[1].data else {
        panic!("tuple varyant bekleniyor")
    };
    assert_eq!(tys.len(), 2);
    let VariantData::Struct(fields) = &e.variants[2].data else {
        panic!("struct varyant bekleniyor")
    };
    assert_eq!(fields.len(), 1);
}

#[test]
fn type_alias_basic() {
    let result = p("type Kelime = bits<32>;");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let ItemKind::TypeAlias(t) = &result.ast.items_arena[result.ast.items[0]].kind else {
        panic!("type alias bekleniyor")
    };
    assert_eq!(t.name.text, "Kelime");
    assert!(matches!(
        result.ast.types[t.target].kind,
        TypeRefKind::Bits(_)
    ));
}

#[test]
fn const_item_with_expr_value() {
    // 20_const_and_generate.volt deseni
    let result = p("const WIDTH : u32 = 8;\nconst DEPTH : u32 = WIDTH * 2;");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let ItemKind::Const(c) = &result.ast.items_arena[result.ast.items[1]].kind else {
        panic!("const bekleniyor")
    };
    assert_eq!(c.name.text, "DEPTH");
    assert!(matches!(
        result.ast.exprs[c.value].kind,
        ExprKind::Binary { .. }
    ));
}

#[test]
fn extern_module_with_ports() {
    let result = p("extern module SvIp { in clk : clock out veri : u8 }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let ItemKind::Extern(e) = &result.ast.items_arena[result.ast.items[0]].kind else {
        panic!("extern bekleniyor")
    };
    assert_eq!(e.name.text, "SvIp");
    assert_eq!(e.ports.len(), 2);
    assert_eq!(e.ports[1].direction, PortDir::Out);
}

#[test]
fn extern_body_rejects_statements() {
    let result = p("extern module X { reg r : u8 = 0 }");
    assert!(result.error_codes().contains(&"E0001"));
}

// ═══ Tipler: yol, dizi, tuple, parametreli reset ══════════════════

#[test]
fn widened_types_parse_as_builtin_ints() {
    // ADR-0031: u9/u17/u33 gibi tipler artık doğrudan UInt/SInt olur.
    let result = p("module M { in a : u8 out sum : u9 sum = a + a }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    assert_eq!(type_dump(&result.ast, module.ports[1].ty), "u9");
    assert!(matches!(
        result.ast.types[module.ports[1].ty].kind,
        TypeRefKind::UInt(9)
    ));
}

#[test]
fn arbitrary_width_types_parse_direct() {
    // ADR-0031: u1..u64 / i1..i64 tüm aralık doğrudan UInt/SInt.
    let result = p("module M { in a : u3 in b : u10 in c : u17 in d : i5 out y : u1 y = 0 }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let kinds: Vec<_> = module
        .ports
        .iter()
        .map(|p| type_dump(&result.ast, p.ty))
        .collect();
    assert_eq!(kinds, ["u3", "u10", "u17", "i5", "u1"]);
    assert!(matches!(
        result.ast.types[module.ports[3].ty].kind,
        TypeRefKind::SInt(5)
    ));
}

#[test]
fn width_above_64_stays_path_type() {
    // 64 üstü genişlik SV eşlemesine sahip değil — eski Path (widened)
    // yolu korunur, typeck aileyi tanır (ADR-0031).
    let result = p("module M { in a : u128 out y : u8 y = 0 }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    assert!(matches!(
        result.ast.types[module.ports[0].ty].kind,
        TypeRefKind::Path { .. }
    ));
}

#[test]
fn array_and_tuple_types() {
    let result = p("type Dizi = [u8; 4];\ntype Cift = (u8, bool);");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let ItemKind::TypeAlias(a) = &result.ast.items_arena[result.ast.items[0]].kind else {
        panic!()
    };
    assert_eq!(type_dump(&result.ast, a.target), "[u8; 4]");
    let ItemKind::TypeAlias(c) = &result.ast.items_arena[result.ast.items[1]].kind else {
        panic!()
    };
    assert_eq!(type_dump(&result.ast, c.target), "(u8, bool)");
}

#[test]
fn parameterized_reset_type() {
    let result = p("module M { in rst : reset(async, active_low) }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let TypeRefKind::Reset(Some(spec)) = &result.ast.types[module.ports[0].ty].kind else {
        panic!("parametreli reset bekleniyor")
    };
    assert_eq!(spec.sync, ResetSync::Async);
}

#[test]
fn nested_generic_args_split_shr() {
    // 'Fifo<Entry<8>>' — lexer '>>' üretir, parser ikiye böler
    let result = p("type X = Fifo<Entry<8>>;");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let ItemKind::TypeAlias(t) = &result.ast.items_arena[result.ast.items[0]].kind else {
        panic!()
    };
    assert_eq!(type_dump(&result.ast, t.target), "Fifo<1 arg>");
}

// ═══ Generics (modül) ═════════════════════════════════════════════

#[test]
fn module_generic_params_const_and_bound() {
    let result = p("module M<const N: u32, T: Tasiyici> { in a : bits<N> }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    assert_eq!(module.generics.len(), 2);
    assert!(matches!(
        module.generics[0].kind,
        GenericParamKind::Const { .. }
    ));
    let GenericParamKind::Type { name, bounds } = &module.generics[1].kind else {
        panic!("tip parametresi bekleniyor")
    };
    assert_eq!(name.text, "T");
    assert_eq!(bounds.len(), 1);
}

#[test]
fn generic_bounds_multiple() {
    let result = p("module M<T: A + B + C> { }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let GenericParamKind::Type { bounds, .. } = &module.generics[0].kind else {
        panic!()
    };
    assert_eq!(bounds.len(), 3);
}

// ═══ Kontratlar ═══════════════════════════════════════════════════

#[test]
fn module_contracts_parsed() {
    let result = p("module M { in a : u8 requires: a > 0 ensures: a < 200 a = a }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    assert_eq!(module.contracts.len(), 2);
    assert_eq!(module.contracts[0].kind, ContractKind::Requires);
}

#[test]
fn all_contract_kinds_parse() {
    let result = p("module M { requires: 1 ensures: 1 invariant: 1 cover: 1 assert: 1 assume: 1 }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let kinds: Vec<ContractKind> = module.contracts.iter().map(|c| c.kind).collect();
    assert_eq!(
        kinds,
        [
            ContractKind::Requires,
            ContractKind::Ensures,
            ContractKind::Invariant,
            ContractKind::Cover,
            ContractKind::Assert,
            ContractKind::Assume,
        ]
    );
}

// ═══ Nitelikler (yalnız ayrıştırma, yorum yok) ════════════════════

#[test]
fn known_attribute_parsed_without_warning() {
    let result = p("@budget(lut = 5000) module M { }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let item = &result.ast.items_arena[result.ast.items[0]];
    assert_eq!(item.attrs.len(), 1);
    assert_eq!(item.attrs[0].name.text, "budget");
    let AttrArg::Named { name, .. } = &item.attrs[0].args[0] else {
        panic!("adlandırılmış argüman bekleniyor")
    };
    assert_eq!(name.text, "lut");
}

#[test]
fn unknown_attribute_is_w0020() {
    let result = p("@bilinmeyen module M { }");
    assert!(result.error_codes().contains(&"W0020"));
    // Uyarıya rağmen öğe ayrıştırılır ve nitelik saklanır
    let item = &result.ast.items_arena[result.ast.items[0]];
    assert_eq!(item.attrs.len(), 1);
    assert!(result.ast.module(0).is_some());
}

#[test]
fn attribute_positional_arg() {
    let result = p("@synthesis_target(asic) module M { }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let item = &result.ast.items_arena[result.ast.items[0]];
    assert!(matches!(item.attrs[0].args[0], AttrArg::Positional(_)));
}

#[test]
fn stmt_attribute_stored() {
    // Porttan sonra ',' gerekli — aksi halde '@timing' port domain
    // anotasyonu olarak bağlanır (grammar §4 DomainAnnot önceliklidir).
    let result = p("module M { in clk : clock, @timing reg r : u8 = 0 }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    assert_eq!(result.ast.stmts[module.body[0]].attrs.len(), 1);
}

// ═══ wire / comb / for ════════════════════════════════════════════

#[test]
fn wire_decl_parsed() {
    // 18_wire_and_widths.volt deseni
    let result = p("module M { wire temp : u33 }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let StmtKind::Wire(w) = &result.ast.stmts[module.body[0]].kind else {
        panic!("wire bekleniyor")
    };
    assert_eq!(w.name.text, "temp");
}

#[test]
fn wire_missing_colon_is_error() {
    assert!(codes("module M { wire w u8 }").contains(&"E0001"));
}

#[test]
fn comb_block_is_real() {
    let result = p("module M { in a : u8 out r : u8 comb { r = a + 1 } }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let StmtKind::Comb(block) = &result.ast.stmts[module.body[0]].kind else {
        panic!("comb bekleniyor")
    };
    assert_eq!(
        result.ast.blocks[*block].context,
        BlockContext::Combinational
    );
}

#[test]
fn for_stmt_module_level() {
    // 20_const_and_generate.volt deseni
    let result = p("module M { for i in 0..8 { temp[i] = data[i] & mask[i] } }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let StmtKind::For(f) = &result.ast.stmts[module.body[0]].kind else {
        panic!("for bekleniyor")
    };
    assert_eq!(f.var.text, "i");
    assert_eq!(result.ast.blocks[f.body].stmts.len(), 1);
}

#[test]
fn for_in_on_block_inherits_sequential_context() {
    let result = p("module M { in clk : clock on clk { for i in 0..4 { r <= i } } }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let StmtKind::On(on) = &result.ast.stmts[module.body[0]].kind else {
        panic!()
    };
    let BlockStmt::For(f) = &result.ast.blocks[on.body].stmts[0] else {
        panic!("for bekleniyor")
    };
    assert_eq!(result.ast.blocks[f.body].context, BlockContext::Sequential);
}

#[test]
fn for_range_with_const_names() {
    let result = p("module M { for i in BASLA..BITIS { t[i] = 0 } }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
}

// ═══ Modül örnekleme — [N3] yeniden sınıflandırma ═════════════════

#[test]
fn instance_reclassified_from_struct_lit() {
    // 17_module_instantiation.volt deseni — geri izleme YOK
    let result = p("module M { let add1 = Adder { a: w, b: x } }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let StmtKind::Instance(inst) = &result.ast.stmts[module.body[0]].kind else {
        panic!("InstanceDecl bekleniyor")
    };
    assert_eq!(inst.name.text, "add1");
    assert_eq!(inst.module_path.segments[0].text, "Adder");
    assert_eq!(inst.bindings.len(), 2);
    assert_eq!(inst.bindings[0].port_name.text, "a");
    assert!(inst.bindings[0].value.is_some());
}

#[test]
fn instance_shorthand_binding() {
    let result = p("module M { let u = Uart { clk } }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let StmtKind::Instance(inst) = &result.ast.stmts[module.body[0]].kind else {
        panic!()
    };
    assert_eq!(inst.bindings[0].port_name.text, "clk");
    assert!(inst.bindings[0].value.is_none(), "kısayol: değer None");
}

#[test]
fn instance_with_path_module() {
    let result = p("module M { let u = lib::Adder { a: x } }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let StmtKind::Instance(inst) = &result.ast.stmts[module.body[0]].kind else {
        panic!()
    };
    assert_eq!(inst.module_path.segments.len(), 2);
}

// ═══ Generic argümanlı örnekleme (grammar §10, ADR-0027) ══════════

#[test]
fn instance_with_generic_args_parses_directly() {
    let result = p(
        // ADR-0028: 'fifo' artık ayrılmış kelime değil, örnekleme adı olabilir.
        "module M { let fifo = AsyncFifo<u8, 16> { wr_clk: fclk, wr_data: din, \
         wr_en: push, rd_clk: sclk, rd_en: pop } }",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let StmtKind::Instance(inst) = &result.ast.stmts[module.body[0]].kind else {
        panic!("InstanceDecl bekleniyor")
    };
    assert_eq!(inst.name.text, "fifo");
    assert_eq!(inst.module_path.segments[0].text, "AsyncFifo");
    assert_eq!(inst.generic_args.len(), 2, "iki generic argüman");
    assert!(matches!(inst.generic_args[0], GenericArg::Type(_)));
    assert!(matches!(inst.generic_args[1], GenericArg::Const(_)));
    assert_eq!(inst.bindings.len(), 5);
    assert_eq!(inst.bindings[0].port_name.text, "wr_clk");
}

#[test]
fn instance_single_type_arg() {
    let result = p("module M { let hs = HandshakeSync<u8> { src_clk: a, data_in: d } }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let StmtKind::Instance(inst) = &result.ast.stmts[module.body[0]].kind else {
        panic!("InstanceDecl bekleniyor")
    };
    assert_eq!(inst.generic_args.len(), 1);
}

#[test]
fn let_comparison_still_parses_as_expression() {
    // `let a = b < c` örnekleme DEĞİL — ileri bakış `> {` görmez.
    let result = p("module M { let a = b < c }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let StmtKind::Let(decl) = &result.ast.stmts[module.body[0]].kind else {
        panic!("LetDecl bekleniyor")
    };
    assert!(matches!(
        result.ast.exprs[decl.value].kind,
        ExprKind::Binary { .. }
    ));
}

#[test]
fn let_double_comparison_stays_expression() {
    // `b < c` ve `d > e` — `>` sonrası `{` yok, ifade yolu korunur.
    let result = p("module M { let a = b < c let z = d > e }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    assert!(matches!(
        result.ast.stmts[module.body[0]].kind,
        StmtKind::Let(_)
    ));
    assert!(matches!(
        result.ast.stmts[module.body[1]].kind,
        StmtKind::Let(_)
    ));
}

#[test]
fn nested_generic_close_does_not_panic() {
    // `Fifo<Entry<8>>` — `>>` iki kapanış sayılır; panik yok, AST üretilir.
    let result = p("module M { let f = Fifo<Entry<8>> { clk: c } }");
    let module = result.ast.module(0).unwrap();
    assert!(!module.body.is_empty(), "deyim üretilmeli");
    let StmtKind::Instance(inst) = &result.ast.stmts[module.body[0]].kind else {
        panic!("InstanceDecl bekleniyor: {:?}", result.error_codes())
    };
    assert_eq!(inst.generic_args.len(), 1);
}

#[test]
fn let_with_type_annotation_stays_let() {
    // Tip anotasyonu varsa yapı literali değeri olan LetDecl kalır
    let result = p("module M { let n : Nokta = Nokta { x: 1 } }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let StmtKind::Let(decl) = &result.ast.stmts[module.body[0]].kind else {
        panic!("LetDecl bekleniyor")
    };
    assert!(matches!(
        result.ast.exprs[decl.value].kind,
        ExprKind::StructLit { .. }
    ));
}

#[test]
fn plain_let_not_reclassified() {
    let result = p("module M { let x = a + b }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    assert!(matches!(
        result.ast.stmts[module.body[0]].kind,
        StmtKind::Let(_)
    ));
}

// ═══ match + desenler ═════════════════════════════════════════════

#[test]
fn match_stmt_in_on_block() {
    let result = p(
        "module M { in clk : clock on clk { match durum { Bekle => { r <= 0 } _ => { r <= 1 } } } }",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let StmtKind::On(on) = &result.ast.stmts[module.body[0]].kind else {
        panic!()
    };
    let BlockStmt::Match(m) = &result.ast.blocks[on.body].stmts[0] else {
        panic!("match bekleniyor")
    };
    assert_eq!(m.arms.len(), 2);
    assert!(matches!(m.arms[0].body, MatchArmBody::Block(_)));
    assert!(matches!(
        result.ast.patterns[m.arms[1].pattern].kind,
        PatternKind::Wildcard
    ));
}

#[test]
fn match_expr_in_assignment() {
    let result = p("module M { y = match x { 0 => 1, _ => 2 } }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let StmtKind::Assign(assign) = &result.ast.stmts[module.body[0]].kind else {
        panic!()
    };
    let ExprKind::Match { arms, .. } = &result.ast.exprs[assign.rhs].kind else {
        panic!("match ifadesi bekleniyor")
    };
    assert_eq!(arms.len(), 2);
    assert!(matches!(arms[0].body, MatchArmBody::Expr(_)));
}

#[test]
fn pattern_path_variant_and_binding() {
    let result = p("module M { y = match x { Durum::Bekle => 0, diger => diger } }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let StmtKind::Assign(assign) = &result.ast.stmts[module.body[0]].kind else {
        panic!()
    };
    let ExprKind::Match { arms, .. } = &result.ast.exprs[assign.rhs].kind else {
        panic!()
    };
    let PatternKind::Path { path, args } = &result.ast.patterns[arms[0].pattern].kind else {
        panic!("yol deseni bekleniyor")
    };
    assert_eq!(path.segments.len(), 2);
    assert!(args.is_none());
    assert!(matches!(
        result.ast.patterns[arms[1].pattern].kind,
        PatternKind::Binding(_)
    ));
}

#[test]
fn pattern_tuple_args() {
    let result = p("module M { y = match x { Veri(a, _) => a, _ => 0 } }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let StmtKind::Assign(assign) = &result.ast.stmts[module.body[0]].kind else {
        panic!()
    };
    let ExprKind::Match { arms, .. } = &result.ast.exprs[assign.rhs].kind else {
        panic!()
    };
    let PatternKind::Path {
        args: Some(PatternArgs::Tuple(elems)),
        ..
    } = &result.ast.patterns[arms[0].pattern].kind
    else {
        panic!("tuple argümanlı desen bekleniyor")
    };
    assert_eq!(elems.len(), 2);
}

#[test]
fn pattern_or_alternatives() {
    let result = p("module M { y = match x { 1 | 2 | 3 => 1, _ => 0 } }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let StmtKind::Assign(assign) = &result.ast.stmts[module.body[0]].kind else {
        panic!()
    };
    let ExprKind::Match { arms, .. } = &result.ast.exprs[assign.rhs].kind else {
        panic!()
    };
    let PatternKind::Or(alts) = &result.ast.patterns[arms[0].pattern].kind else {
        panic!("or deseni bekleniyor")
    };
    assert_eq!(alts.len(), 3);
}

#[test]
fn pattern_tuple_top_level() {
    let result = p("module M { y = match x { (a, b) => a, _ => 0 } }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let StmtKind::Assign(assign) = &result.ast.stmts[module.body[0]].kind else {
        panic!()
    };
    let ExprKind::Match { arms, .. } = &result.ast.exprs[assign.rhs].kind else {
        panic!()
    };
    assert!(matches!(
        result.ast.patterns[arms[0].pattern].kind,
        PatternKind::Tuple(_)
    ));
}

#[test]
fn match_arm_guard() {
    let result = p("module M { y = match x { n if n > 4 => 1, _ => 0 } }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let StmtKind::Assign(assign) = &result.ast.stmts[module.body[0]].kind else {
        panic!()
    };
    let ExprKind::Match { arms, .. } = &result.ast.exprs[assign.rhs].kind else {
        panic!()
    };
    assert!(arms[0].guard.is_some());
    assert!(arms[1].guard.is_none());
}

#[test]
fn match_exhaustiveness_not_checked_in_parser() {
    // İFADE konumundaki match serbesttir (ADR-0032 yalnız deyimi bağlar);
    // eksik kollar F2/F3 tip analizinin işi — parser tanı ÜRETMEZ.
    let result = p("module M { y = match x { 1 => 2 } }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
}

// ═══ E0014: deyim konumunda '_' kolu zorunlu (ADR-0032) ═══════════

#[test]
fn match_stmt_missing_wildcard_is_e0014() {
    let result =
        p("module M { in clk : clock on clk { match r { 0 => { r <= 1 } 1 => { r <= 0 } } } }");
    assert!(
        result.error_codes().contains(&"E0014"),
        "{:?}",
        result.error_codes()
    );
}

#[test]
fn match_stmt_or_wildcard_alternative_counts() {
    // `1 | _` alternatifi kapsayıcıdır — E0014 üretilmez.
    let result =
        p("module M { in clk : clock on clk { match r { 0 => { r <= 1 } 1 | _ => { } } } }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
}

#[test]
fn match_stmt_guarded_wildcard_is_still_e0014() {
    // Muhafızlı '_' kapsayıcı değildir.
    let result = p("module M { in clk : clock on clk { match r { _ if c => { r <= 1 } } } }");
    assert!(
        result.error_codes().contains(&"E0014"),
        "{:?}",
        result.error_codes()
    );
}

#[test]
fn match_stmt_in_comb_block_requires_wildcard() {
    let result = p("module M { comb { match x { 0 => { y = 1 } } } }");
    assert!(
        result.error_codes().contains(&"E0014"),
        "{:?}",
        result.error_codes()
    );
}

#[test]
fn cast_to_arbitrary_width_parses() {
    // ADR-0031: `as u10` gibi keyfi genişlik dönüşümleri.
    let result = p("module M { in a : u8 out y : u10 y = a as u10 }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
}

// ═══ todo! / tuple / dizi / yapı literalleri / string ═════════════

#[test]
fn todo_expr_with_message() {
    let result = p("module M { y = todo!(\"sonra\") }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let StmtKind::Assign(assign) = &result.ast.stmts[module.body[0]].kind else {
        panic!()
    };
    let ExprKind::Todo { message } = &result.ast.exprs[assign.rhs].kind else {
        panic!("todo bekleniyor")
    };
    assert_eq!(message.as_deref(), Some("sonra"));
}

#[test]
fn todo_expr_bare() {
    let result = p("module M { y = todo! }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
}

#[test]
fn tuple_literal_expr() {
    let (result, root) = volt_syntax::parser::parse_expr(FileId(0), "(a, b, 3)");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    assert_eq!(dump(&result.ast, root), "(tuple a b 3)");
}

#[test]
fn parenthesized_expr_is_not_tuple() {
    let (result, root) = volt_syntax::parser::parse_expr(FileId(0), "(a)");
    assert!(result.diagnostics.is_empty());
    assert_eq!(dump(&result.ast, root), "a");
}

#[test]
fn array_literal_list_and_repeat() {
    let (result, root) = volt_syntax::parser::parse_expr(FileId(0), "[1, 2, 3]");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    assert_eq!(dump(&result.ast, root), "[1 2 3]");

    let (result, root) = volt_syntax::parser::parse_expr(FileId(0), "[0; 16]");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    assert_eq!(dump(&result.ast, root), "[0; 16]");
}

#[test]
fn struct_lit_as_assign_rhs() {
    let result = p("module M { y = Nokta { x: 1, y: 2 } }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).unwrap();
    let StmtKind::Assign(assign) = &result.ast.stmts[module.body[0]].kind else {
        panic!()
    };
    let ExprKind::StructLit { fields, .. } = &result.ast.exprs[assign.rhs].kind else {
        panic!("yapı literali bekleniyor")
    };
    assert_eq!(fields.len(), 2);
}

#[test]
fn struct_lit_not_parsed_in_if_condition() {
    // 'if x { }' — buradaki '{' blok başlangıcıdır, yapı literali değil
    let result = p("module M { in clk : clock on clk { if hazir { r <= 1 } else { r <= 0 } } }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
}

#[test]
fn string_literal_expr_with_escapes() {
    let (result, root) = volt_syntax::parser::parse_expr(FileId(0), "\"a\\n\\\"b\\\"\"");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let ExprKind::StringLit(s) = &result.ast.exprs[root].kind else {
        panic!("string bekleniyor")
    };
    assert_eq!(s, "a\n\"b\"");
}

#[test]
fn invalid_escape_is_e0012() {
    let (result, _) = volt_syntax::parser::parse_expr(FileId(0), "\"kotu\\q\"");
    assert!(result.error_codes().contains(&"E0012"));
}

#[test]
fn range_in_for_only_not_general_expr() {
    // '..' yalnız for başlığında aralıktır; ifadede '[hi:lo]' kullanılır
    let result = p("module M { for i in 0..N { t[i] = 0 } }");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
}

// ═══ W0010 — parantez önerisi (operator-precedence.md §5) ═════════

#[test]
fn w0010_bitand_bitor_mix() {
    let (result, _) = volt_syntax::parser::parse_expr(FileId(0), "a & b | c");
    assert!(
        result.error_codes().contains(&"W0010"),
        "{:?}",
        result.error_codes()
    );
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "W0010")
        .unwrap();
    assert!(diag.message.contains("'&'") && diag.message.contains("'|'"));
}

#[test]
fn w0010_shl_add_mix_shows_interpretation() {
    let (result, _) = volt_syntax::parser::parse_expr(FileId(0), "a << 1 + 2");
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "W0010")
        .expect("W0010 bekleniyor");
    assert!(diag.message.contains("'<<'") && diag.message.contains("'+'"));
    // Mevcut yorum gösterilmeli: a << (1 + 2)
    let help = diag.help.as_deref().unwrap_or("");
    assert!(
        help.contains("a << (1 + 2)"),
        "yardım mevcut yorumu göstermeli: {help}"
    );
}

#[test]
fn w0010_help_for_and_or_mix() {
    let (result, _) = volt_syntax::parser::parse_expr(FileId(0), "a & b | c");
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "W0010")
        .unwrap();
    let help = diag.help.as_deref().unwrap_or("");
    assert!(
        help.contains("(a & b) | c"),
        "yardım mevcut yorumu göstermeli: {help}"
    );
}

#[test]
fn w0010_suppressed_by_parentheses() {
    let (result, _) = volt_syntax::parser::parse_expr(FileId(0), "(a & b) | c");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let (result, _) = volt_syntax::parser::parse_expr(FileId(0), "a << (1 + 2)");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
}

#[test]
fn w0010_not_fired_for_same_op_chain() {
    let (result, _) = volt_syntax::parser::parse_expr(FileId(0), "a & b & c");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
}

#[test]
fn w0010_not_fired_for_unrelated_mixes() {
    // Yalnız '&'/'|' ve '<<'/'+' çiftleri uyarılır (spec §5)
    let (result, _) = volt_syntax::parser::parse_expr(FileId(0), "a ^ b | c");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let (result, _) = volt_syntax::parser::parse_expr(FileId(0), "a >> 1 + 2");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
}

// ═══ tests/ui taraması (F1 tamamlanma ölçütleri) ═══════════════════

#[test]
fn ui_pass_all_42_of_42_parse_clean() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/ui/pass");
    let mut total = 0;
    let mut clean = 0;
    let mut dirty = Vec::new();
    for entry in std::fs::read_dir(dir).expect("ui/pass okunmalı") {
        let path = entry.expect("girdi").path();
        if path.extension().and_then(|e| e.to_str()) != Some("volt") {
            continue;
        }
        total += 1;
        let src = std::fs::read_to_string(&path).expect("dosya okunmalı");
        let result = p(&src);
        if result.diagnostics.is_empty() {
            clean += 1;
        } else {
            dirty.push(format!(
                "{}: {:?}",
                path.file_name().unwrap().to_string_lossy(),
                result.error_codes()
            ));
        }
    }
    assert_eq!(total, 42, "ui/pass 42 dosya içermeli");
    // F1b öncesi 02 ve 19 'out out : u8' yazıyordu (port adı olarak
    // 'out' anahtar kelimesi); fixture'lar 'result' olarak düzeltildi,
    // artık tamamı temiz ayrışmalı. F4b 23_provable_invariant'ı ekledi;
    // F5 (ADR-0027) 27-29 yerleşik CDC primitif fixture'larını,
    // ADR-0029 ise 30-35 tek saatli stdlib fixture'larını,
    // ADR-0031/0032 ise 37-38 keyfi genişlik + match fixture'larını,
    // ADR-0033 ise 39_test_block'u (test blokları),
    // ADR-0034 ise 40_implication_operator'ı (implikasyon),
    // ADR-0035 ise 41-42'yi (dizi indeksi + part-select),
    // ADR-0036 ise 43'ü (işaretli işlemler),
    // ADR-0037 ise 44'ü (L1 zamanlama, Delayed),
    // ADR-0038 ise 45-46'yı (pipeline sözdizimi) ekledi.
    assert_eq!(
        clean, 42,
        "42/42 ayrışmalı; temiz: {clean}, sorunlu: {dirty:#?}"
    );
}

#[test]
fn ui_pass_13_cdc_bridge_parses() {
    // ADR-0023 tamamlanma ölçütü
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/ui/pass/13_cdc_correct_bridge.volt"
    );
    let src = std::fs::read_to_string(path).expect("dosya okunmalı");
    let result = p(&src);
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
}

#[test]
fn ui_fail_files_produce_expected_codes() {
    let cases = [
        ("05_comparison_chain.volt", "E0010"),
        ("06_wrong_assign_operator.volt", "E0006"),
        ("16_missing_else.volt", "E0008"),
        ("18_comb_wrong_operator.volt", "E0007"),
        ("27_match_missing_wildcard.volt", "E0014"),
        // ADR-0038: pipeline tanıları desugar'da (parse içinde) üretilir.
        ("33_stage_out_of_range.volt", "E5012"),
        ("34_pipeline_bad_stall.volt", "E5013"),
    ];
    for (file, expected) in cases {
        let path = format!(
            "{}/../../tests/ui/fail/{}",
            env!("CARGO_MANIFEST_DIR"),
            file
        );
        let src = std::fs::read_to_string(&path).expect("dosya okunmalı");
        let result = p(&src);
        assert!(
            result.error_codes().contains(&expected),
            "{file}: {expected} bekleniyor, bulunan: {:?}",
            result.error_codes()
        );
    }
}

// ═══ Hata kurtarma ════════════════════════════════════════════════

#[test]
fn recovery_continues_after_broken_module() {
    // error-recovery.md §8.1: bozuk modülden sonraki modül ayrıştırılmalı
    let src = "module Broken {\n    in a :\n}\n\nmodule Valid {\n    in b : u8\n    out c : u8\n    c = b\n}";
    let result = p(src);
    assert_eq!(result.diagnostics.len(), 1, "{:?}", result.error_codes());
    assert_eq!(result.ast.items.len(), 2);
    let valid = result.ast.module(1).expect("ikinci modül ayrıştırılmalı");
    assert_eq!(valid.name.text, "Valid");
    assert_eq!(valid.ports.len(), 2);
    assert_eq!(valid.body.len(), 1);
}

#[test]
fn recovery_garbage_between_modules() {
    let result = p("module A {} 123 456 module B {}");
    assert_eq!(result.ast.items.len(), 3); // A, Error, B
    assert_eq!(result.ast.module(0).unwrap().name.text, "A");
    assert_eq!(result.ast.module(2).unwrap().name.text, "B");
    assert!(result.error_codes().contains(&"E0001"));
}

#[test]
fn recovery_missing_rbrace_is_e0002_with_open_location() {
    let result = p("module M { in a : u8");
    assert!(
        result.error_codes().contains(&"E0002"),
        "{:?}",
        result.error_codes()
    );
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E0002")
        .unwrap();
    assert!(
        diag.spans
            .iter()
            .any(|s| !s.primary && s.label.contains("opening")),
        "açılış konumu ikincil span olmalı"
    );
}

#[test]
fn recovery_error_stmt_does_not_kill_block() {
    // error-recovery.md §4.4: bozuk deyimden sonrakiler ayrıştırılmalı
    let result = p("module M { in clk : clock on clk { a <= + ; c <= d; e <= f; } }");
    let module = result.ast.module(0).unwrap();
    let StmtKind::On(on) = &result.ast.stmts[module.body[0]].kind else {
        panic!()
    };
    let assigns = result.ast.blocks[on.body]
        .stmts
        .iter()
        .filter(|s| matches!(s, BlockStmt::NonBlockAssign { .. }))
        .count();
    assert!(assigns >= 2, "sonraki atamalar kurtarılmalı: {assigns}");
    assert!(!result.diagnostics.is_empty());
}

#[test]
fn parser_never_hangs_on_pathological_input() {
    let big_braces = "{".repeat(5000);
    let big_parens = "(".repeat(5000);
    let cases: Vec<&str> = vec![
        "",
        "{",
        "}",
        "((((",
        "module",
        "module {",
        "@@@@",
        "module module module",
        "on on on",
        "= = =",
        "<= <=",
        &big_braces,
        &big_parens,
    ];
    for src in cases {
        let start = std::time::Instant::now();
        let _ = p(src);
        assert!(start.elapsed().as_secs() < 5, "takıldı: {:.20}...", src);
    }
}

#[test]
fn deep_expression_nesting_does_not_overflow() {
    let src = format!(
        "module M {{ let x = {}1{} }}",
        "(".repeat(400),
        ")".repeat(400)
    );
    let _ = p(&src); // panik/stack overflow olmamalı
}

#[test]
fn empty_source_is_valid() {
    let result = p("");
    assert!(result.diagnostics.is_empty());
    assert!(result.ast.items.is_empty());
}

#[test]
fn cascade_suppression_limits_error_flood() {
    // Bozuk tek bölge → tanı sayısı token sayısından ÇOK az olmalı
    let result = p("module M { in in in in in in in in : : : : }");
    assert!(
        result.diagnostics.len() <= 5,
        "kaskad bastırma çalışmıyor: {} tanı",
        result.diagnostics.len()
    );
}

#[test]
fn port_doc_comment_attached() {
    let result = p("module M { /// saat girişi\n in clk : clock }");
    let module = result.ast.module(0).unwrap();
    let doc = module.ports[0].doc.as_ref().expect("port doc bekleniyor");
    assert!(doc.contains("saat girişi"));
}

#[test]
fn every_stmt_carries_span() {
    let result = p(&counter_src());
    let module = result.ast.module(0).unwrap();
    for &stmt in &module.body {
        let span = result.ast.stmts[stmt].span;
        assert!(span.end > span.start, "boş span: {span:?}");
        assert_eq!(span.file, FileId(0));
    }
}

// ═══ Test blokları (ADR-0033) ══════════════════════════════════════

#[test]
fn test_block_parses_with_all_builtins() {
    let src = "test \"counter increments\" {\n    let dut = Counter { };\n    dut.enable = true;\n    step(1);\n    assert_eq(dut.count, 1);\n    reset();\n    assert_ne(dut.count, 1);\n    assert_true(dut.count);\n    assert_false(dut.count);\n}\n";
    let result = p(src);
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let ast = result.ast;
    assert_eq!(ast.items.len(), 1);
    let ItemKind::Test(test) = &ast.items_arena[ast.items[0]].kind else {
        panic!("test öğesi bekleniyor");
    };
    assert_eq!(test.name, "counter increments");
    assert_eq!(test.display_name(), "counter_increments");
    assert_eq!(test.stmts.len(), 8);
}

#[test]
fn test_is_contextual_not_reserved() {
    // 'test' bağlamsal (ADR-0023 deseni): sıradan Ident olarak serbest.
    let src = "module M {\n    in  clk  : clock\n    in  test : bool\n    out q    : bool\n    reg r : bool = false\n    on clk { r <= test }\n    q = r\n}\n";
    let result = p(src);
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
}

#[test]
fn test_block_hex_and_bool_literals() {
    let src = "test \"t\" {\n    let dut = Tx { };\n    dut.data = 0xA5;\n    dut.start = false;\n    step(2);\n}\n";
    let result = p(src);
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let ItemKind::Test(test) = &result.ast.items_arena[result.ast.items[0]].kind else {
        panic!("test öğesi bekleniyor");
    };
    assert_eq!(test.stmts.len(), 4);
}

#[test]
fn test_block_missing_semi_is_e0001() {
    let src = "test \"t\" {\n    let dut = Counter { };\n    step(1)\n}\n";
    assert_eq!(codes(src), vec!["E0001"]);
}

#[test]
fn test_block_bad_stmt_recovers_to_next_semi() {
    // Bozuk deyim atlanır, sonrakiler ayrışmaya devam eder.
    let src = "test \"t\" {\n    let dut = Counter { };\n    += 3;\n    step(1);\n}\n";
    let result = p(src);
    assert!(!result.diagnostics.is_empty());
    let ItemKind::Test(test) = &result.ast.items_arena[result.ast.items[0]].kind else {
        panic!("test öğesi bekleniyor");
    };
    // let + step ayrışır; bozuk satır düşer.
    assert_eq!(test.stmts.len(), 2);
}

#[test]
fn test_block_unclosed_brace_is_e0002() {
    let src = "test \"t\" {\n    let dut = Counter { };\n";
    assert!(codes(src).contains(&"E0002"));
}

// ═══ Indexed part-select (ADR-0035) ═══════════════════════════════

#[test]
fn part_select_ascending_sexp() {
    assert_eq!(expr_clean_sexp("a[i +: 8]"), "(+: a i 8)");
}

#[test]
fn part_select_descending_sexp() {
    assert_eq!(expr_clean_sexp("a[i -: 8]"), "(-: a i 8)");
}

#[test]
fn part_select_start_is_full_expr() {
    assert_eq!(expr_clean_sexp("a[i + 1 +: 8]"), "(+: a (+ i 1) 8)");
}

#[test]
fn part_select_chains_with_index() {
    // Sonek zinciri: önce eleman, sonra parça.
    assert_eq!(expr_clean_sexp("a[0][i +: 4]"), "(+: (index a 0) i 4)");
}

#[test]
fn range_select_still_parses_as_range() {
    assert_eq!(expr_clean_sexp("a[7:4]"), "(range a 7 4)");
}

#[test]
fn part_select_lvalue_target_parses() {
    let result = p("module M {\n    in clk : clock\n    in i : bits<3>\n    reg acc : u8 = 0\n    on clk {\n        acc[i +: 4] <= 0\n    }\n}\n");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let module = result.ast.module(0).expect("modül");
    let stmt = &result.ast.stmts[module.body[1]];
    let StmtKind::On(on) = &stmt.kind else {
        panic!("on bloğu bekleniyor");
    };
    let block = &result.ast.blocks[on.body];
    let BlockStmt::NonBlockAssign { lhs, .. } = &block.stmts[0] else {
        panic!("nonblocking atama bekleniyor");
    };
    assert!(matches!(
        lhs.suffixes[0],
        LValueSuffix::PartSelect {
            ascending: true,
            ..
        }
    ));
}

#[test]
fn dynamic_array_index_lvalue_parses() {
    let result = p("module M {\n    in clk : clock\n    in idx : bits<2>\n    reg regs : [u8; 4] = [0; 4]\n    on clk {\n        regs[idx] <= 1\n    }\n}\n");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
}

#[test]
fn part_select_missing_width_recovers() {
    let (result, _) = parse_expr(FileId(0), "a[i +: ]");
    assert!(
        !result.diagnostics.is_empty(),
        "genişlik eksik — tanı bekleniyor"
    );
}

// ═══ L1 zamanlama sözdizimi (ADR-0037) ═════════════════════════════

#[test]
fn delayed_type_desugars_to_inner_with_side_table() {
    let result =
        p("module M {\n    in clk : clock\n    reg r : Delayed<u8, 2> = 0\n    on clk { r <= r }\n}\n");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    assert_eq!(result.ast.timing.delayed_types.len(), 1);
    let m = result.ast.module(0).expect("modül bekleniyor");
    let StmtKind::Reg(r) = &result.ast.stmts[m.body[0]].kind else {
        panic!("reg bekleniyor");
    };
    let ty = r.ty.expect("tip anotasyonu bekleniyor");
    // Tip düğümü iç tiptir — çözümleme ve SV üretimi yalnız u8 görür.
    assert!(matches!(result.ast.types[ty].kind, TypeRefKind::UInt(8)));
    let (cycles, _) = result.ast.timing.delayed_types[&ty];
    assert!(matches!(
        result.ast.exprs[cycles].kind,
        ExprKind::IntLit { value: 2, .. }
    ));
}

#[test]
fn delayed_type_const_name_cycles_becomes_path_expr() {
    let result = p(
        "const K : u32 = 2\nmodule M {\n    in clk : clock\n    reg r : Delayed<u8, K> = 0\n    on clk { r <= r }\n}\n",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let (_ty, &(cycles, _)) = result
        .ast
        .timing
        .delayed_types
        .iter()
        .next()
        .expect("tablo girdisi bekleniyor");
    assert!(matches!(result.ast.exprs[cycles].kind, ExprKind::Path(_)));
}

#[test]
fn delayed_with_wrong_arity_is_e0001() {
    assert!(
        codes("module M { in clk : clock reg r : Delayed<8> = 0 on clk { r <= r } }")
            .contains(&"E0001")
    );
}

#[test]
fn delay_expr_desugars_to_inner_with_side_table() {
    let result =
        p("module M {\n    in x : u32\n    out y : u32\n    let s = delay<2>(x)\n    y = s\n}\n");
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    assert_eq!(result.ast.timing.delay_exprs.len(), 1);
    let m = result.ast.module(0).expect("modül bekleniyor");
    let StmtKind::Let(l) = &result.ast.stmts[m.body[0]].kind else {
        panic!("let bekleniyor");
    };
    // AST'de yalnız iç ifade yaşar; sarmalayıcı yan tablodadır.
    assert!(matches!(result.ast.exprs[l.value].kind, ExprKind::Path(_)));
    let wraps = &result.ast.timing.delay_exprs[&l.value];
    assert_eq!(wraps.len(), 1);
    assert!(matches!(
        result.ast.exprs[wraps[0].0].kind,
        ExprKind::IntLit { value: 2, .. }
    ));
}

#[test]
fn nested_delay_exprs_accumulate_on_same_key() {
    let result = p(
        "module M {\n    in x : u32\n    out y : u32\n    let s = delay<1>(delay<2>(x))\n    y = s\n}\n",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let m = result.ast.module(0).expect("modül bekleniyor");
    let StmtKind::Let(l) = &result.ast.stmts[m.body[0]].kind else {
        panic!("let bekleniyor");
    };
    assert_eq!(result.ast.timing.delay_exprs[&l.value].len(), 2);
}

#[test]
fn delay_as_plain_identifier_still_parses() {
    // 'delay' rezerve değildir: tam kalıp (`delay<K>(`) dışında sıradan isim.
    let result = p(
        "module M {\n    in delay : u32\n    out y : u32\n    let s = delay + 1\n    y = s\n}\n",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    assert!(result.ast.timing.delay_exprs.is_empty());
}

#[test]
fn strict_timing_attribute_is_known() {
    let result = p("@strict_timing\nmodule M {\n    in clk : clock\n}\n");
    assert!(
        !result.error_codes().contains(&"W0020"),
        "@strict_timing tanınan nitelik olmalı: {:?}",
        result.error_codes()
    );
}

#[test]
fn delayed_in_let_annotation_desugars() {
    let result = p(
        "module M {\n    in x : u32\n    out y : u32\n    let f : Delayed<u32, 1> = x\n    y = f\n}\n",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    assert_eq!(result.ast.timing.delayed_types.len(), 1);
    let m = result.ast.module(0).expect("modül bekleniyor");
    let StmtKind::Let(l) = &result.ast.stmts[m.body[0]].kind else {
        panic!("let bekleniyor");
    };
    let ty = l.ty.expect("tip anotasyonu bekleniyor");
    assert!(matches!(result.ast.types[ty].kind, TypeRefKind::UInt(32)));
}
