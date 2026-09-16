//! Bundle dizisi düzleştirme testleri — ADR-0056.
//!
//! `[Handshake<T>; N]` ve `[struct port; N]` portları eleman eleman
//! `<port>_<k>_<alan>` düz portlarına açılır; `p[k].alan` erişimleri
//! (sabit k) düz isme yeniden yazılır; sabit olmayan / aralık dışı
//! indeks E2008.

use volt_ast::{ExprKind, ItemKind, ModuleDecl, PortDir, StmtKind};
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

fn ports(res: &ParseResult, idx: usize) -> Vec<(PortDir, String)> {
    module(res, idx)
        .ports
        .iter()
        .map(|p| (p.direction, p.name.text.clone()))
        .collect()
}

const REQ: &str =
    "struct port Req {\n    out addr  : u8\n    out valid : bool\n    in  ack   : bool\n}\n";

#[test]
fn handshake_array_port_flattens_per_element() {
    let r = p("module M { in clk : clock\n in rx : [Handshake<u8>; 2]\n out y : u8\n rx[0].ready = true\n rx[1].ready = true\n y = rx[0].data | rx[1].data }");
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    let names: Vec<String> = ports(&r, 0).into_iter().map(|(_, n)| n).collect();
    assert_eq!(
        names,
        [
            "clk",
            "rx_0_data",
            "rx_0_valid",
            "rx_0_ready",
            "rx_1_data",
            "rx_1_valid",
            "rx_1_ready",
            "y"
        ]
    );
    // `in` elemanın yönleri terslenir: data/valid giriş, ready çıkış.
    let dirs = ports(&r, 0);
    assert_eq!(dirs[1].0, PortDir::In);
    assert_eq!(dirs[3].0, PortDir::Out);
}

#[test]
fn struct_port_array_flattens_and_flips_per_element() {
    let r = p(&format!(
        "{REQ}module M {{ in req : [Req; 2]\n out busy : bool\n req[0].ack = req[0].valid\n req[1].ack = req[1].valid\n busy = req[0].addr != 0 }}"
    ));
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    let ps = ports(&r, 1);
    assert_eq!(ps[0], (PortDir::In, "req_0_addr".to_string()));
    assert_eq!(ps[2], (PortDir::Out, "req_0_ack".to_string()));
    assert_eq!(ps[5], (PortDir::Out, "req_1_ack".to_string()));
    let m = module(&r, 1);
    let StmtKind::Assign(a) = &r.ast.stmts[m.body[1]].kind else {
        panic!()
    };
    assert_eq!(a.lhs.base.text, "req_1_ack");
    assert!(a.lhs.suffixes.is_empty());
}

#[test]
fn constant_index_expressions_and_for_variables_are_rewritten() {
    let r = p("const N : u32 = 3\nmodule M { in rx : [Handshake<u8>; N]\n out v : [bool; N]\n out last : u8\n for i in 0..N { rx[i].ready = true\n v[i] = rx[i].valid }\n last = rx[N - 1].data }");
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    let m = module(&r, 1);
    // for açılımı: 3 × (ready ataması + valid ataması) + last.
    assert_eq!(m.body.len(), 7);
    let StmtKind::Assign(a) = &r.ast.stmts[m.body[4]].kind else {
        panic!()
    };
    assert_eq!(a.lhs.base.text, "rx_2_ready");
    let StmtKind::Assign(last) = &r.ast.stmts[m.body[6]].kind else {
        panic!()
    };
    let ExprKind::Path(path) = &r.ast.exprs[last.rhs].kind else {
        panic!("düz isim bekleniyor: {:?}", r.ast.exprs[last.rhs].kind)
    };
    assert_eq!(path.segments[0].text, "rx_2_data");
}

#[test]
fn virtual_fields_work_on_array_elements() {
    let r = p("module M { in rx : [Handshake<u8>; 2]\n out f : bool\n rx[0].ready = true\n rx[1].ready = true\n f = rx[1].fired || rx[0].stalled }");
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    let m = module(&r, 0);
    let StmtKind::Assign(a) = &r.ast.stmts[m.body[2]].kind else {
        panic!()
    };
    let ExprKind::Binary { lhs, .. } = &r.ast.exprs[a.rhs].kind else {
        panic!()
    };
    // rx[1].fired → rx_1_valid && rx_1_ready
    let ExprKind::Binary { lhs: v, .. } = &r.ast.exprs[*lhs].kind else {
        panic!()
    };
    let ExprKind::Path(path) = &r.ast.exprs[*v].kind else {
        panic!()
    };
    assert_eq!(path.segments[0].text, "rx_1_valid");
}

#[test]
fn automatic_protocol_contracts_are_generated_per_element() {
    let r = p("module M { in rx : [Handshake<u8>; 2]\n out tx : [Handshake<u8>; 2]\n rx[0].ready = tx[0].ready\n rx[1].ready = tx[1].ready\n tx[0].valid = rx[0].valid\n tx[1].valid = rx[1].valid\n tx[0].data = rx[0].data\n tx[1].data = rx[1].data }");
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    // Port başına 2 kontrat (tutma + veri kararlılığı) × 4 eleman.
    assert_eq!(module(&r, 0).contracts.len(), 8);
}

#[test]
fn dynamic_index_is_e2008() {
    let r = p("module M { in sel : u1\n in rx : [Handshake<u8>; 2]\n out y : u8\n rx[0].ready = true\n rx[1].ready = true\n y = rx[sel].data }");
    assert_eq!(r.error_codes(), vec!["E2008"]);
    let d = &r.diagnostics[0];
    assert!(d.message.contains("'rx'"), "{}", d.message);
    assert!(d.help.as_deref().unwrap_or("").contains("for"));
}

#[test]
fn out_of_range_index_is_e2008() {
    let r = p("module M { in rx : [Handshake<u8>; 2]\n out y : u8\n rx[0].ready = true\n rx[1].ready = true\n y = rx[2].data }");
    assert_eq!(r.error_codes(), vec!["E2008"]);
    assert!(r.diagnostics[0].message.contains("out of range"));
}

#[test]
fn non_constant_length_is_e2008_and_port_dropped() {
    let r = p("module M { in n : u8\n in rx : [Handshake<u8>; n]\n out y : u8\n y = 0 }");
    assert_eq!(r.error_codes(), vec!["E2008"]);
    assert_eq!(ports(&r, 0).len(), 2, "hatalı port düşürülmeli");
}

#[test]
fn plain_scalar_array_ports_are_untouched() {
    // `[u8; 4]` bundle değildir: düzleşmez, SV üretimi paketler (ADR-0056).
    let r = p("module M { in a : [u8; 4]\n out y : u8\n y = a[0] }");
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    assert_eq!(ports(&r, 0).len(), 2);
}

#[test]
fn array_length_from_generic_parameter_after_mono() {
    // Düzleştirme mono'dan SONRA koşar: `[Handshake<u8>; K]` monomorfta literaldir.
    let r = p("module Mux<const K: u32> { in rx : [Handshake<u8>; K]\n out y : u8\n rx[0].ready = true\n y = rx[0].data }\nmodule Top { in rx : [Handshake<u8>; 1]\n out y : u8\n let m = Mux<1> { rx_0_data: rx[0].data, rx_0_valid: rx[0].valid }\n rx[0].ready = m.rx_0_ready\n y = m.y }");
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    let names: Vec<String> = ports(&r, 0).into_iter().map(|(_, n)| n).collect();
    assert_eq!(names, ["rx_0_data", "rx_0_valid", "rx_0_ready", "y"]);
}
