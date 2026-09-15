//! Çift yönlü port desugar testleri — ADR-0051.
//!
//! `inout` / `opendrain` portlarının yöntem çağrıları (`drive`,
//! `drive_low`, `release`) `on` bloğunda `<=` atamalarına, `read()` porta,
//! `released`/`driving` sanal alanları sürücü register'ına yeniden
//! yazılır; sürücü register'ları parser sonunda gövdeye eklenir.

use volt_ast::{BlockStmt, ExprKind, ItemKind, ModuleDecl, PortDir, StmtKind, UnOp};
use volt_span::FileId;
use volt_syntax::parser::{parse, ParseResult};

fn p(src: &str) -> ParseResult {
    parse(FileId(0), src)
}

fn module(res: &ParseResult, idx: usize) -> &ModuleDecl {
    let ItemKind::Module(m) = &res.ast.items_arena[res.ast.items[idx]].kind else {
        panic!("öğe {idx} modül olmalı");
    };
    m
}

/// Modül gövdesindeki `reg` / `let` bildirim adları (sırayla).
fn decl_names(res: &ParseResult, idx: usize) -> Vec<(&str, String)> {
    module(res, idx)
        .body
        .iter()
        .filter_map(|&s| match &res.ast.stmts[s].kind {
            StmtKind::Reg(r) => Some(("reg", r.name.text.clone())),
            StmtKind::Let(l) => Some(("let", l.name.text.clone())),
            _ => None,
        })
        .collect()
}

/// İlk `on` bloğunun gövdesindeki `<=` atamaları: (hedef, sağ taraf).
fn on_assigns(res: &ParseResult, idx: usize) -> Vec<(String, String)> {
    let m = module(res, idx);
    let on = m
        .body
        .iter()
        .find_map(|&s| match &res.ast.stmts[s].kind {
            StmtKind::On(on) => Some(on.body),
            _ => None,
        })
        .expect("on bloğu");
    res.ast.blocks[on]
        .stmts
        .iter()
        .filter_map(|bs| match bs {
            BlockStmt::NonBlockAssign { lhs, rhs, .. } => Some((
                lhs.base.text.clone(),
                match &res.ast.exprs[*rhs].kind {
                    ExprKind::BoolLit(b) => b.to_string(),
                    ExprKind::Path(p) => p.segments[0].text.clone(),
                    other => format!("{other:?}"),
                },
            )),
            _ => None,
        })
        .collect()
}

const OD: &str = "module M {\n    in clk : clock\n    in en : bool\n    opendrain sda : bool\n    on clk {\n        sda.drive_low()\n        sda.release()\n    }\n}\n";

#[test]
fn opendrain_port_parses_with_its_own_direction() {
    let res = p(OD);
    assert!(res.diagnostics.is_empty(), "{:?}", res.error_codes());
    let ports: Vec<_> = module(&res, 0)
        .ports
        .iter()
        .map(|p| (p.direction, p.name.text.as_str()))
        .collect();
    assert_eq!(ports[2], (PortDir::OpenDrain, "sda"));
    assert!(PortDir::OpenDrain.is_bidirectional());
    assert_eq!(PortDir::OpenDrain.keyword(), "opendrain");
}

#[test]
fn inout_port_keeps_inout_direction() {
    let res =
        p("module M {\n    in clk : clock\n    inout dq : u8\n    on clk { dq.release() }\n}\n");
    assert!(res.diagnostics.is_empty(), "{:?}", res.error_codes());
    assert_eq!(module(&res, 0).ports[1].direction, PortDir::InOut);
}

#[test]
fn opendrain_is_contextual_and_usable_as_an_identifier() {
    // Port konumu dışında `opendrain` sıradan bir isimdir (ADR-0023 tarzı).
    let res = p("module M {\n    in clk : clock\n    out q : bool\n    let opendrain = true\n    q = opendrain\n}\n");
    assert!(res.diagnostics.is_empty(), "{:?}", res.error_codes());
    assert_eq!(decl_names(&res, 0), vec![("let", "opendrain".to_string())]);
}

#[test]
fn drive_low_and_release_become_enable_register_assignments() {
    let res = p(OD);
    assert_eq!(
        on_assigns(&res, 0),
        vec![
            ("sda_drive_low".to_string(), "true".to_string()),
            ("sda_drive_low".to_string(), "false".to_string()),
        ]
    );
}

#[test]
fn inout_drive_produces_enable_and_data_assignments() {
    let res = p("module M {\n    in clk : clock\n    in v : u8\n    inout dq : u8\n    on clk {\n        dq.drive(v)\n        dq.release()\n    }\n}\n");
    assert!(res.diagnostics.is_empty(), "{:?}", res.error_codes());
    assert_eq!(
        on_assigns(&res, 0),
        vec![
            ("dq_oe".to_string(), "true".to_string()),
            ("dq_out".to_string(), "v".to_string()),
            ("dq_oe".to_string(), "false".to_string()),
        ]
    );
}

#[test]
fn driven_ports_get_synthesised_registers_at_the_top_of_the_body() {
    let res = p(OD);
    assert_eq!(
        decl_names(&res, 0),
        vec![("reg", "sda_drive_low".to_string())]
    );
    let res = p("module M {\n    in clk : clock\n    inout dq : bits<8>\n    reg x : bool = false\n    on clk { dq.drive(0 as bits<8>) }\n}\n");
    assert!(res.diagnostics.is_empty(), "{:?}", res.error_codes());
    assert_eq!(
        decl_names(&res, 0),
        vec![
            ("reg", "dq_oe".to_string()),
            ("reg", "dq_out".to_string()),
            ("reg", "x".to_string()),
        ]
    );
}

#[test]
fn read_call_rewrites_to_the_port_itself() {
    let res = p("module M {\n    in clk : clock\n    opendrain sda : bool\n    out q : bool\n    q = sda.read()\n}\n");
    assert!(res.diagnostics.is_empty(), "{:?}", res.error_codes());
    let m = module(&res, 0);
    let assign = m
        .body
        .iter()
        .find_map(|&s| match &res.ast.stmts[s].kind {
            StmtKind::Assign(a) => Some(a.rhs),
            _ => None,
        })
        .expect("atama");
    match &res.ast.exprs[assign].kind {
        ExprKind::Path(p) => assert_eq!(p.segments[0].text, "sda"),
        other => panic!("Path bekleniyor: {other:?}"),
    }
}

#[test]
fn released_and_driving_rewrite_to_the_enable_register() {
    let src = "module M {\n    in clk : clock\n    opendrain sda : bool\n    invariant: sda.released\n    cover: sda.driving\n    on clk { sda.drive_low() }\n}\n";
    let res = p(src);
    assert!(res.diagnostics.is_empty(), "{:?}", res.error_codes());
    let m = module(&res, 0);
    match &res.ast.exprs[m.contracts[0].expr].kind {
        ExprKind::Unary {
            op: UnOp::Not,
            operand,
        } => match &res.ast.exprs[*operand].kind {
            ExprKind::Path(p) => assert_eq!(p.segments[0].text, "sda_drive_low"),
            other => panic!("Path bekleniyor: {other:?}"),
        },
        other => panic!("!sda_drive_low bekleniyor: {other:?}"),
    }
    match &res.ast.exprs[m.contracts[1].expr].kind {
        ExprKind::Path(p) => assert_eq!(p.segments[0].text, "sda_drive_low"),
        other => panic!("Path bekleniyor: {other:?}"),
    }
}

#[test]
fn observed_but_undriven_port_gets_a_constant_let() {
    let res = p("module M {\n    in clk : clock\n    opendrain sda : bool\n    invariant: sda.released\n    out q : bool\n    q = sda.read()\n}\n");
    assert!(res.diagnostics.is_empty(), "{:?}", res.error_codes());
    assert_eq!(
        decl_names(&res, 0),
        vec![("let", "sda_drive_low".to_string())]
    );
}

#[test]
fn read_only_pad_synthesises_nothing() {
    let res = p("module M {\n    in clk : clock\n    inout pin : bool\n    out q : bool\n    q = pin.read()\n}\n");
    assert!(res.diagnostics.is_empty(), "{:?}", res.error_codes());
    assert!(decl_names(&res, 0).is_empty());
}

#[test]
fn drive_call_outside_an_on_block_is_e4008() {
    let res = p("module M {\n    in clk : clock\n    in en : bool\n    opendrain sda : bool\n    comb {\n        if en { sda.drive_low() }\n    }\n}\n");
    assert_eq!(res.error_codes(), vec!["E4008"]);
}

#[test]
fn unknown_member_is_e4008() {
    let res = p("module M {\n    in clk : clock\n    opendrain sda : bool\n    on clk { sda.toggle() }\n}\n");
    assert_eq!(res.error_codes(), vec!["E4008"]);
    let res = p("module M {\n    in clk : clock\n    opendrain sda : bool\n    out q : bool\n    q = sda.level\n}\n");
    assert_eq!(res.error_codes(), vec!["E4008"]);
}

#[test]
fn wrong_drive_method_for_the_port_kind_is_e4008() {
    // opendrain: drive(value) yok; inout: drive_low() yok.
    let res = p("module M {\n    in clk : clock\n    opendrain sda : bool\n    on clk { sda.drive(true) }\n}\n");
    assert_eq!(res.error_codes(), vec!["E4008"]);
    let res =
        p("module M {\n    in clk : clock\n    inout dq : u8\n    on clk { dq.drive_low() }\n}\n");
    assert_eq!(res.error_codes(), vec!["E4008"]);
}

#[test]
fn drive_arity_mismatch_is_e4008() {
    let res =
        p("module M {\n    in clk : clock\n    inout dq : u8\n    on clk { dq.drive() }\n}\n");
    assert_eq!(res.error_codes(), vec!["E4008"]);
    let res = p("module M {\n    in clk : clock\n    opendrain sda : bool\n    on clk { sda.release(true) }\n}\n");
    assert_eq!(res.error_codes(), vec!["E4008"]);
}

#[test]
fn read_as_a_statement_and_drive_as_a_value_are_e4008() {
    let res = p(
        "module M {\n    in clk : clock\n    opendrain sda : bool\n    on clk { sda.read() }\n}\n",
    );
    assert_eq!(res.error_codes(), vec!["E4008"]);
    let res = p("module M {\n    in clk : clock\n    opendrain sda : bool\n    out q : bool\n    q = sda.release()\n}\n");
    assert_eq!(res.error_codes(), vec!["E4008"]);
}

#[test]
fn opendrain_must_be_bool_and_inout_must_be_scalar() {
    let res = p("module M {\n    in clk : clock\n    opendrain bus : u8\n    on clk { bus.drive_low() }\n}\n");
    assert_eq!(res.error_codes(), vec!["E4008"]);
    let res = p("module M {\n    in clk : clock\n    inout arr : [u8; 4]\n    on clk { arr.release() }\n}\n");
    assert_eq!(res.error_codes(), vec!["E4008"]);
}

#[test]
fn method_call_on_an_ordinary_port_takes_the_normal_assignment_path() {
    // `x` çift yönlü değil: E4008 değil, sıradan "atama hedefi" hatası.
    let res = p("module M {\n    in clk : clock\n    in x : bool\n    on clk { x.release() }\n}\n");
    assert!(
        !res.error_codes().contains(&"E4008"),
        "{:?}",
        res.error_codes()
    );
    assert!(!res.diagnostics.is_empty());
}

#[test]
fn extern_module_accepts_bidirectional_ports() {
    let res = p(
        "extern module Pad {\n    in clk : clock\n    opendrain sda : bool\n    inout dq : u8\n}\n",
    );
    assert!(res.diagnostics.is_empty(), "{:?}", res.error_codes());
    let ItemKind::Extern(x) = &res.ast.items_arena[res.ast.items[0]].kind else {
        panic!("extern bekleniyor");
    };
    assert_eq!(x.ports[1].direction, PortDir::OpenDrain);
    assert_eq!(x.ports[2].direction, PortDir::InOut);
}

#[test]
fn drive_calls_inside_nested_if_and_match_are_rewritten() {
    let src = "module M {\n    in clk : clock\n    in s : u2\n    opendrain sda : bool\n    on clk {\n        match s {\n            0 => { sda.drive_low() }\n            _ => { if s == 1 { sda.release() } }\n        }\n    }\n}\n";
    let res = p(src);
    assert!(res.diagnostics.is_empty(), "{:?}", res.error_codes());
    assert_eq!(
        decl_names(&res, 0),
        vec![("reg", "sda_drive_low".to_string())]
    );
}
