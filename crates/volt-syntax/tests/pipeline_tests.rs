//! Pipeline sözdizimi (ADR-0038) birim testleri: ayrıştırma, desugar,
//! aşama register üretimi, stall/flush ve tanı kodları E5011–E5016.

use volt_ast::{BlockStmt, ExprKind, ItemKind, ModuleDecl, SourceFile, StmtKind};
use volt_span::FileId;
use volt_syntax::parser::{parse, ParseResult};

fn p(src: &str) -> ParseResult {
    parse(FileId(0), src)
}

fn codes(src: &str) -> Vec<&'static str> {
    p(src).error_codes()
}

fn module(result: &ParseResult) -> &ModuleDecl {
    match &result.ast.items_arena[result.ast.items[0]].kind {
        ItemKind::Module(m) => m,
        other => panic!("modül bekleniyor, bulunan: {other:?}"),
    }
}

/// Modül gövdesindeki reg adları (bildirim sırasıyla).
fn reg_names(ast: &SourceFile, m: &ModuleDecl) -> Vec<String> {
    m.body
        .iter()
        .filter_map(|&si| match &ast.stmts[si].kind {
            StmtKind::Reg(r) => Some(r.name.text.clone()),
            _ => None,
        })
        .collect()
}

/// Modül gövdesindeki let adları (bildirim sırasıyla).
fn let_names(ast: &SourceFile, m: &ModuleDecl) -> Vec<String> {
    m.body
        .iter()
        .filter_map(|&si| match &ast.stmts[si].kind {
            StmtKind::Let(l) => Some(l.name.text.clone()),
            _ => None,
        })
        .collect()
}

const BASIC: &str = "pipeline(2) P {
    in clk : clock
    in x : u32
    out y : u32

    stage F {
        let a : u32 = x
    }
    stage D {
        let b : u32 = a + 1
    }

    y = stage(D).b
}
";

// ═══ Yapı ═════════════════════════════════════════════════════════

#[test]
fn pipeline_desugars_to_module() {
    let r = p(BASIC);
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    let m = module(&r);
    assert_eq!(m.name.text, "P");
    assert_eq!(m.ports.len(), 3);
}

#[test]
fn pipeline_gets_implicit_strict_timing() {
    let r = p(BASIC);
    let item = &r.ast.items_arena[r.ast.items[0]];
    assert!(item.attrs.iter().any(|a| a.name.text == "strict_timing"));
}

#[test]
fn explicit_strict_timing_not_duplicated() {
    let src = format!("@strict_timing\n{BASIC}");
    let r = p(&src);
    let item = &r.ast.items_arena[r.ast.items[0]];
    let n = item
        .attrs
        .iter()
        .filter(|a| a.name.text == "strict_timing")
        .count();
    assert_eq!(n, 1);
}

#[test]
fn boundary_register_generated_with_stage_prefix() {
    let r = p(BASIC);
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    let m = module(&r);
    assert_eq!(reg_names(&r.ast, m), vec!["f_a_r"]);
}

#[test]
fn hoisted_lets_reach_module_level() {
    let r = p(BASIC);
    let m = module(&r);
    let lets = let_names(&r.ast, m);
    assert!(lets.contains(&"a".to_string()) && lets.contains(&"b".to_string()));
}

#[test]
fn on_block_generated_for_single_clock() {
    let r = p(BASIC);
    let m = module(&r);
    let on_count = m
        .body
        .iter()
        .filter(|&&si| matches!(r.ast.stmts[si].kind, StmtKind::On(_)))
        .count();
    assert_eq!(on_count, 1);
}

#[test]
fn multi_boundary_value_gets_register_chain() {
    let src = "pipeline(3) P {
    in clk : clock
    in x : u32
    out y : u32
    stage F { let a : u32 = x }
    stage D { let b : u32 = a }
    stage E { let c : u32 = a + b }
    y = stage(E).c
}
";
    let r = p(src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    let regs = reg_names(&r.ast, module(&r));
    // a: F→E zinciri (f_a_r, d_a_r); b: D→E (d_b_r).
    assert!(regs.contains(&"f_a_r".to_string()), "{regs:?}");
    assert!(regs.contains(&"d_a_r".to_string()), "{regs:?}");
    assert!(regs.contains(&"d_b_r".to_string()), "{regs:?}");
}

#[test]
fn bool_value_bubbles_to_false() {
    let src = "pipeline(2) P {
    in clk : clock
    in x : u32
    out y : bool
    stage F { let a : bool = x == 0 }
    stage D { let b : bool = a }
    y = stage(D).b
}
";
    let r = p(src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    let m = module(&r);
    let init = m
        .body
        .iter()
        .find_map(|&si| match &r.ast.stmts[si].kind {
            StmtKind::Reg(reg) if reg.name.text == "f_a_r" => Some(reg.init),
            _ => None,
        })
        .expect("f_a_r üretilmeli");
    assert!(matches!(r.ast.exprs[init].kind, ExprKind::BoolLit(false)));
}

// ═══ E5011: yapı hataları ═════════════════════════════════════════

#[test]
fn stage_count_mismatch_is_e5011() {
    let src = BASIC.replace("pipeline(2)", "pipeline(5)");
    assert!(codes(&src).contains(&"E5011"));
}

#[test]
fn pipeline_without_stages_is_e5011() {
    let src =
        "pipeline(2) P {\n    in clk : clock\n    in x : u32\n    out y : u32\n    y = x\n}\n";
    assert!(codes(src).contains(&"E5011"));
}

#[test]
fn duplicate_stage_name_is_e5011() {
    let src = BASIC.replace("stage D", "stage F");
    assert!(codes(&src).contains(&"E5011"));
}

#[test]
fn missing_clock_port_is_e5011() {
    let src = BASIC.replace("in clk : clock\n", "");
    assert!(codes(&src).contains(&"E5011"));
}

#[test]
fn two_clock_ports_is_e5011() {
    let src = BASIC.replace("in clk : clock", "in clk : clock\n    in clk2 : clock");
    assert!(codes(&src).contains(&"E5011"));
}

// ═══ E5012: aşama referansları ════════════════════════════════════

#[test]
fn stage_ref_rewrites_to_boundary_register() {
    let r = p(BASIC);
    // y = stage(D).b → canlı 'b' (D kendi aşaması); b, f_a_r okur.
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    let m = module(&r);
    let b_value = m
        .body
        .iter()
        .find_map(|&si| match &r.ast.stmts[si].kind {
            StmtKind::Let(l) if l.name.text == "b" => Some(l.value),
            _ => None,
        })
        .expect("b taşınmalı");
    let ExprKind::Binary { lhs, .. } = &r.ast.exprs[b_value].kind else {
        panic!("b = a + 1 bekleniyor");
    };
    let ExprKind::Path(path) = &r.ast.exprs[*lhs].kind else {
        panic!("yol bekleniyor");
    };
    assert_eq!(path.segments[0].text, "f_a_r");
}

#[test]
fn relative_back_reference_parses_clean() {
    let src = BASIC.replace("a + 1", "stage(-1).a + 1");
    assert!(codes(&src).is_empty(), "{:?}", codes(&src));
}

#[test]
fn forward_live_reference_parses_clean() {
    // Erken aşama, geç aşamanın canlı değerini okuyabilir (geri tel).
    let src = "pipeline(2) P {
    in clk : clock
    in x : u32
    out y : u32
    stage F { let a : u32 = if stage(D).b == 0 { x } else { x + 1 } }
    stage D { let b : u32 = a }
    y = stage(D).b
}
";
    assert!(codes(src).is_empty(), "{:?}", codes(src));
}

#[test]
fn stage_ref_out_of_range_is_e5012() {
    let src = BASIC.replace("a + 1", "stage(+9).a");
    assert!(codes(&src).contains(&"E5012"));
}

#[test]
fn stage_ref_unknown_stage_is_e5012() {
    let src = BASIC.replace("stage(D).b", "stage(Yok).b");
    assert!(codes(&src).contains(&"E5012"));
}

#[test]
fn stage_ref_unknown_value_is_e5012() {
    let src = BASIC.replace("stage(D).b", "stage(D).yok");
    assert!(codes(&src).contains(&"E5012"));
}

#[test]
fn stage_ref_before_definition_is_e5012() {
    // b, D'de tanımlı; F görünümü henüz yok.
    let src = BASIC.replace("stage(D).b", "stage(F).b");
    assert!(codes(&src).contains(&"E5012"));
}

#[test]
fn relative_ref_at_module_level_is_e5012() {
    let src = BASIC.replace("stage(D).b", "stage(-1).b");
    assert!(codes(&src).contains(&"E5012"));
}

#[test]
fn bare_name_of_future_stage_is_e5012() {
    let src = BASIC.replace("let a : u32 = x", "let a : u32 = b");
    assert!(codes(&src).contains(&"E5012"));
}

#[test]
fn bare_stage_value_at_module_level_is_e5012() {
    let src = BASIC.replace("stage(D).b", "b");
    assert!(codes(&src).contains(&"E5012"));
}

#[test]
fn stage_ref_outside_pipeline_is_e5012() {
    let src = "module M {\n    in x : u32\n    out y : u32\n    y = stage(F).a\n}\n";
    assert!(codes(src).contains(&"E5012"));
}

// ═══ E5013: stall/flush ═══════════════════════════════════════════

#[test]
fn bare_stall_at_module_level_is_e5013() {
    let src = BASIC.replace("y = stage(D).b", "stall when x == 0\n    y = stage(D).b");
    assert!(codes(&src).contains(&"E5013"));
}

#[test]
fn non_prefix_stall_is_e5013() {
    let src = BASIC.replace("y = stage(D).b", "stall D when x == 0\n    y = stage(D).b");
    assert!(codes(&src).contains(&"E5013"));
}

#[test]
fn unknown_stage_in_stall_is_e5013() {
    let src = BASIC.replace(
        "y = stage(D).b",
        "stall Yok when x == 0\n    y = stage(D).b",
    );
    assert!(codes(&src).contains(&"E5013"));
}

#[test]
fn flush_without_list_is_e5013() {
    let src = BASIC.replace("y = stage(D).b", "flush when x == 0\n    y = stage(D).b");
    assert!(codes(&src).contains(&"E5013"));
}

#[test]
fn flush_unknown_stage_is_e5013() {
    let src = BASIC.replace(
        "y = stage(D).b",
        "flush Yok when x == 0\n    y = stage(D).b",
    );
    assert!(codes(&src).contains(&"E5013"));
}

#[test]
fn prefix_stall_list_parses_clean() {
    let src = BASIC.replace(
        "y = stage(D).b",
        "stall F, D when x == 0\n    y = stage(D).b",
    );
    assert!(codes(&src).is_empty(), "{:?}", codes(&src));
}

#[test]
fn stall_in_stage_generates_guard_when_referenced() {
    let src = "pipeline(2) P {
    in clk : clock
    in x : u32
    out y : u32
    out s : bool
    stage F { let a : u32 = x }
    stage D {
        let b : u32 = a + 1
        stall when b == 0
    }
    y = stage(D).b
    s = stall_fetch
}
"
    .replace("stall_fetch", "stall_f");
    let r = p(&src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    let lets = let_names(&r.ast, module(&r));
    assert!(lets.contains(&"stall_f".to_string()), "{lets:?}");
}

#[test]
fn unused_guard_lets_are_not_generated() {
    let src = "pipeline(2) P {
    in clk : clock
    in x : u32
    out y : u32
    stage F { let a : u32 = x }
    stage D {
        let b : u32 = a + 1
        stall when b == 0
    }
    y = stage(D).b
}
";
    let r = p(src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    let lets = let_names(&r.ast, module(&r));
    // stall_d sınır tutması için gerekli; stall_f hiç kullanılmıyor.
    assert!(lets.contains(&"stall_d".to_string()), "{lets:?}");
    assert!(!lets.contains(&"stall_f".to_string()), "{lets:?}");
}

// ═══ E5014 / E5015 / E5016 ════════════════════════════════════════

#[test]
fn crossing_value_without_type_is_e5014() {
    let src = BASIC.replace("let a : u32 = x", "let a = x");
    assert!(codes(&src).contains(&"E5014"));
}

#[test]
fn non_scalar_crossing_value_is_e5014() {
    let src = BASIC
        .replace("let a : u32 = x", "let a : [u32; 2] = [x, x]")
        .replace("a + 1", "a[0] + 1");
    assert!(codes(&src).contains(&"E5014"));
}

#[test]
fn same_stage_only_value_needs_no_type() {
    let src = BASIC.replace(
        "let b : u32 = a + 1",
        "let t = a\n        let b : u32 = t + 1",
    );
    assert!(codes(&src).is_empty(), "{:?}", codes(&src));
}

#[test]
fn combinational_cycle_is_e5015() {
    let src = "pipeline(2) P {
    in clk : clock
    in x : u32
    out y : u32
    stage F { let a : u32 = stage(D).b }
    stage D { let b : u32 = stage(F).a }
    y = stage(D).b
}
";
    assert!(codes(src).contains(&"E5015"));
}

#[test]
fn duplicate_value_name_is_e5016() {
    let src = BASIC.replace("let b : u32 = a + 1", "let a : u32 = 1");
    assert!(codes(&src).contains(&"E5016"));
}

// ═══ Ayrıştırma ayrıntıları ═══════════════════════════════════════

#[test]
fn missing_when_is_parse_error() {
    let src = BASIC.replace("stage(D).b", "stage(D).b\n    stall F");
    assert!(!codes(&src).is_empty());
}

#[test]
fn pinned_table_records_stage_ref_lets() {
    let src = "pipeline(2) P {
    in clk : clock
    in x : u32
    out y : u32
    stage F { let a : u32 = x }
    stage D { let b : u32 = stage(-1).a + stage(D).c
              let c : u32 = 7 }
    y = stage(D).b
}
";
    let r = p(src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    // b, stage(...) içerir → kendi aşamasına (1) sabitlenir.
    assert!(!r.ast.timing.pinned.is_empty());
    assert!(r.ast.timing.pinned.values().any(|&(n, _)| n == 1));
}

#[test]
fn user_stage_assign_guarded_by_stall() {
    let src = "pipeline(2) P {
    in clk : clock
    in x : u32
    out y : u32
    reg cnt : u32 = 0
    stage F {
        let a : u32 = x
        cnt <= cnt + 1
    }
    stage D {
        let b : u32 = a + 1
        stall when b == 0
    }
    y = stage(D).b
}
";
    let r = p(src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    let m = module(&r);
    let on_body = m
        .body
        .iter()
        .find_map(|&si| match &r.ast.stmts[si].kind {
            StmtKind::On(on) => Some(on.body),
            _ => None,
        })
        .expect("on bloğu üretilmeli");
    // İlk deyim: if (!stall_f) { cnt <= cnt + 1 }
    let first = &r.ast.blocks[on_body].stmts[0];
    let BlockStmt::If(ifstmt) = first else {
        panic!("stall muhafızı bekleniyor");
    };
    let ExprKind::Unary { operand, .. } = &r.ast.exprs[ifstmt.cond].kind else {
        panic!("!stall_f bekleniyor");
    };
    let ExprKind::Path(path) = &r.ast.exprs[*operand].kind else {
        panic!("stall_f yolu bekleniyor");
    };
    assert_eq!(path.segments[0].text, "stall_f");
}

#[test]
fn forwarding_let_ordered_after_its_dependency() {
    // D aşamasındaki let, E'nin canlı değerini okur → E'nin let'i önce
    // gelmeli (çözümleme sıralı).
    let src = "pipeline(3) P {
    in clk : clock
    in x : u32
    out y : u32
    stage F { let a : u32 = x }
    stage D { let b : u32 = if stage(E).c == 0 { a } else { a + 1 } }
    stage E { let c : u32 = b }
    y = stage(E).c
}
";
    let r = p(src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    let lets = let_names(&r.ast, module(&r));
    let pos_b = lets.iter().position(|n| n == "b").unwrap();
    let pos_c = lets.iter().position(|n| n == "c").unwrap();
    assert!(pos_c < pos_b, "c, b'den önce bildirilmeli: {lets:?}");
}

#[test]
fn stall_and_flush_keywords_only_in_pipeline() {
    let src = "module M {\n    in x : u32\n    out y : u32\n    stall when x == 0\n    y = x\n}\n";
    assert!(!codes(src).is_empty());
}

#[test]
fn closing_module_keyword_not_required() {
    let r = p(BASIC);
    assert!(module(&r).closing_name.is_none());
}
