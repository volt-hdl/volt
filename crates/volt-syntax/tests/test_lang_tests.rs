//! Test dili genişletmesi (ADR-0058): yerel değişken, dizi, `for`,
//! ifadeler ve değer yerleşikleri — ayrıştırma düzeyi.

use volt_ast::{ItemKind, TestBinOp, TestDecl, TestExpr, TestExprKind, TestStmt, TestUnOp};
use volt_syntax::{parse, FileId, ParseResult};

fn p(src: &str) -> ParseResult {
    parse(FileId(0), src)
}

/// Gövdeyi `test "t" { ... }` içine sarar ve temiz ayrıştığını doğrular.
fn stmts_of(body: &str) -> Vec<TestStmt> {
    let src = format!("test \"t\" {{\n{body}\n}}\n");
    let result = p(&src);
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let mut ast = result.ast;
    let idx = ast.items[0];
    let ItemKind::Test(TestDecl { stmts, .. }) =
        std::mem::replace(&mut ast.items_arena[idx].kind, ItemKind::Error)
    else {
        panic!("test öğesi bekleniyor");
    };
    stmts
}

/// `let x = <ifade>;` deyiminin sağ tarafı.
fn let_value(body: &str) -> TestExpr {
    let mut stmts = stmts_of(body);
    let TestStmt::LetVar { value, .. } = stmts.remove(0) else {
        panic!("LetVar bekleniyor");
    };
    value
}

#[test]
fn let_with_module_literal_is_still_a_dut() {
    let stmts = stmts_of("    let dut = Sbox { };\n    let n = 4;");
    assert!(matches!(&stmts[0], TestStmt::LetDut { module, .. } if module.text == "Sbox"));
    assert!(matches!(&stmts[1], TestStmt::LetVar { name, .. } if name.text == "n"));
}

#[test]
fn array_literal_with_trailing_comma() {
    let value = let_value("    let expected = [0x63, 0x7c, true,];");
    let TestExprKind::Array(items) = value.kind else {
        panic!("dizi bekleniyor");
    };
    assert_eq!(items.len(), 3);
    assert!(matches!(items[0].kind, TestExprKind::Int(0x63)));
    assert!(matches!(items[2].kind, TestExprKind::Bool(true)));
}

#[test]
fn index_var_and_port_forms() {
    let value = let_value("    let v = expected[i + 1];");
    let TestExprKind::Index { base, index } = value.kind else {
        panic!("indeks bekleniyor");
    };
    assert_eq!(base.text, "expected");
    assert!(matches!(
        index.kind,
        TestExprKind::Binary {
            op: TestBinOp::Add,
            ..
        }
    ));
    assert!(matches!(
        let_value("    let v = dut.count;").kind,
        TestExprKind::PortRead { .. }
    ));
    assert!(matches!(
        let_value("    let v = i;").kind,
        TestExprKind::Var(_)
    ));
}

#[test]
fn member_path_has_every_segment() {
    let stmts = stmts_of("    load(dut.cpu.imem, rom);");
    let TestStmt::Call { func, args, .. } = &stmts[0] else {
        panic!("çağrı bekleniyor");
    };
    assert_eq!(func.text, "load");
    let TestExprKind::MemberPath { dut, path } = &args[0].kind else {
        panic!("üye yolu bekleniyor");
    };
    assert_eq!(dut.text, "dut");
    let names: Vec<&str> = path.iter().map(|n| n.text.as_str()).collect();
    assert_eq!(names, ["cpu", "imem"]);
}

#[test]
fn read_hex_takes_a_string_and_unescapes_backslashes() {
    let value = let_value("    let rom = read_hex(\"sw\\\\hello.hex\");");
    let TestExprKind::Call { func, args } = value.kind else {
        panic!("çağrı bekleniyor");
    };
    assert_eq!(func.text, "read_hex");
    assert!(matches!(&args[0].kind, TestExprKind::Str(s) if s == "sw\\hello.hex"));
}

/// İkili ifadeyi `(op lhs rhs)` biçiminde yazar — öncelik denetimi için.
fn show(expr: &TestExpr) -> String {
    match &expr.kind {
        TestExprKind::Int(n) => n.to_string(),
        TestExprKind::Var(name) => name.text.clone(),
        TestExprKind::Unary {
            op: TestUnOp::Not,
            operand,
        } => format!("(! {})", show(operand)),
        TestExprKind::Binary { op, lhs, rhs } => format!("({op:?} {} {})", show(lhs), show(rhs)),
        other => format!("{other:?}"),
    }
}

#[test]
fn multiplication_binds_tighter_than_addition() {
    assert_eq!(
        show(&let_value("    let v = 1 + i * 2;")),
        "(Add 1 (Mul i 2))"
    );
}

#[test]
fn binary_operators_are_left_associative() {
    assert_eq!(
        show(&let_value("    let v = 8 - 4 - 1;")),
        "(Sub (Sub 8 4) 1)"
    );
    assert_eq!(
        show(&let_value("    let v = 64 / 4 % 3;")),
        "(Rem (Div 64 4) 3)"
    );
}

#[test]
fn bitwise_binds_tighter_than_comparison_and_logic_is_loosest() {
    assert_eq!(
        show(&let_value("    let v = a & 1 == 0 || b << 2 > 4 && !c;")),
        "(LogOr (Eq (And a 1) 0) (LogAnd (Gt (Shl b 2) 4) (! c)))"
    );
    assert_eq!(
        show(&let_value("    let v = a | b ^ c & d;")),
        "(Or a (Xor b (And c d)))"
    );
}

#[test]
fn comparison_binds_tighter_than_equality() {
    // operator-precedence.md: `< > <= >=` (4) eşitlikten (3) sıkıdır.
    assert_eq!(
        show(&let_value("    let v = a < b == c > d;")),
        "(Eq (Lt a b) (Gt c d))"
    );
}

#[test]
fn chained_comparisons_are_rejected() {
    for body in ["let v = a < b < c;", "let v = a == b != c;"] {
        let result = p(&format!(
            "test \"t\" {{
    {body}
}}
"
        ));
        assert!(!result.diagnostics.is_empty(), "{body} reddedilmeli");
    }
    // Parantezle yazılınca geçerlidir.
    assert_eq!(
        show(&let_value("    let v = (a < b) == (b < c);")),
        "(Eq (Lt a b) (Lt b c))"
    );
}

#[test]
fn parentheses_override_precedence() {
    assert_eq!(
        show(&let_value("    let v = (1 + i) * 2;")),
        "(Mul (Add 1 i) 2)"
    );
}

#[test]
fn for_loop_holds_bounds_and_body() {
    let stmts = stmts_of("    for i in 0..len(rom) {\n        dut.a = i;\n        step(1);\n    }");
    let TestStmt::For {
        var,
        start,
        end,
        body,
        ..
    } = &stmts[0]
    else {
        panic!("for bekleniyor");
    };
    assert_eq!(var.text, "i");
    assert!(matches!(start.kind, TestExprKind::Int(0)));
    assert!(matches!(&end.kind, TestExprKind::Call { func, .. } if func.text == "len"));
    assert_eq!(body.len(), 2);
}

#[test]
fn for_loops_nest() {
    let stmts = stmts_of(
        "    for i in 0..2 {\n        for j in i..4 {\n            step(1);\n        }\n    }",
    );
    let TestStmt::For { body, .. } = &stmts[0] else {
        panic!("for bekleniyor");
    };
    let TestStmt::For { var, start, .. } = &body[0] else {
        panic!("iç for bekleniyor");
    };
    assert_eq!(var.text, "j");
    assert!(matches!(&start.kind, TestExprKind::Var(n) if n.text == "i"));
}

#[test]
fn for_without_range_reports_and_recovers() {
    let result = p("test \"t\" {\n    let dut = C { };\n    for i in 4 {\n        step(1);\n    }\n    step(2);\n}\n");
    assert!(
        !result.diagnostics.is_empty(),
        "'..' eksikliği raporlanmalı"
    );
    // Ayrıştırıcı takılmaz: test öğesi yine de kurulur.
    assert_eq!(result.ast.items.len(), 1);
}

#[test]
fn unclosed_array_literal_is_an_error_not_a_hang() {
    let result = p("test \"t\" {\n    let a = [1, 2;\n    step(1);\n}\n");
    assert!(!result.diagnostics.is_empty());
}

#[test]
fn step_accepts_an_expression_argument() {
    let stmts = stmts_of("    step(n * 2);");
    let TestStmt::Call { func, args, .. } = &stmts[0] else {
        panic!("çağrı bekleniyor");
    };
    assert_eq!(func.text, "step");
    assert_eq!(show(&args[0]), "(Mul n 2)");
}

// ─── Patolojik girdi: yığın taşması yerine tanı ───────────────────

fn assert_rejected_without_crash(body: &str) {
    let result = p(&format!("test \"t\" {{\n    {body}\n}}\n"));
    assert!(
        !result.diagnostics.is_empty(),
        "derinlik sınırı tanı üretmeli"
    );
}

#[test]
fn deeply_nested_parentheses_hit_the_depth_limit() {
    let body = format!("let x = {}1{};", "(".repeat(5000), ")".repeat(5000));
    assert_rejected_without_crash(&body);
}

#[test]
fn long_operator_chain_hits_the_depth_limit() {
    // Sol-derin ağaç: sonraki özyineli yürüyüşler de bu derinliği görür.
    let body = format!("let x = 1{};", " + 1".repeat(5000));
    assert_rejected_without_crash(&body);
}

#[test]
fn long_not_chain_hits_the_depth_limit() {
    let body = format!("let x = {}1;", "!".repeat(200_000));
    assert_rejected_without_crash(&body);
}

#[test]
fn deeply_nested_loops_hit_the_depth_limit() {
    let body = format!(
        "{}step(1);{}",
        "for i in 0..2 { ".repeat(5000),
        " }".repeat(5000)
    );
    assert_rejected_without_crash(&body);
}

#[test]
fn ordinary_nesting_stays_below_the_limit() {
    let body = format!("let x = {}1{} + 2 * 3;", "(".repeat(50), ")".repeat(50));
    let result = p(&format!("test \"t\" {{\n    {body}\n}}\n"));
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
}

#[test]
fn bad_expression_in_for_header_does_not_close_the_test_early() {
    let result = p("test \"t\" {\n    let dut = C { };\n    for i in 0.. {\n        step(1);\n    }\n    step(2);\n}\n");
    assert_eq!(result.diagnostics.len(), 1, "{:?}", result.error_codes());
    assert_eq!(result.ast.items.len(), 1);
}

#[test]
fn enum_variant_is_a_test_value() {
    // ADR-0074: `State::Idle` test değeri.
    let value = let_value("    let s = State::Idle;");
    let TestExprKind::Variant { enum_name, variant } = &value.kind else {
        panic!("Variant bekleniyor: {value:?}");
    };
    assert_eq!(
        (enum_name.text.as_str(), variant.text.as_str()),
        ("State", "Idle")
    );
}

#[test]
fn enum_path_without_variant_is_e0001() {
    let res = parse(FileId(0), "test \"t\" {\n    let s = State::;\n}\n");
    assert!(
        res.error_codes().contains(&"E0001"),
        "{:?}",
        res.error_codes()
    );
}
