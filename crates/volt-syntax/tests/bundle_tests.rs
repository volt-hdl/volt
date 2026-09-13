//! Bundle (port grubu) düzleştirme testleri — ADR-0039.
//!
//! `struct port` tipli modül portları parser sonunda `<port>_<alan>`
//! düz portlarına açılır; `in` port yönleri tersler; `aw.addr`
//! erişimleri düz isme yeniden yazılır.

use std::collections::HashSet;

use volt_ast::{BlockStmt, ExprKind, ItemKind, ModuleDecl, PortDir, StmtKind};
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

fn path_text(res: &ParseResult, e: volt_ast::Idx<volt_ast::Expr>) -> Option<String> {
    match &res.ast.exprs[e].kind {
        ExprKind::Path(p) if p.segments.len() == 1 => Some(p.segments[0].text.clone()),
        _ => None,
    }
}

const HANDSHAKE: &str =
    "struct port Handshake {\n    out data  : u8\n    out valid : bool\n    in  ready : bool\n}\n";

#[test]
fn out_bundle_keeps_declared_directions() {
    let src =
        format!("{HANDSHAKE}module M {{ out hs : Handshake\n hs.data = 0\n hs.valid = true }}");
    let res = p(&src);
    assert!(res.diagnostics.is_empty(), "{:?}", res.error_codes());
    assert_eq!(
        ports(&res, 1),
        vec![
            (PortDir::Out, "hs_data".into()),
            (PortDir::Out, "hs_valid".into()),
            (PortDir::In, "hs_ready".into()),
        ]
    );
}

#[test]
fn in_bundle_flips_every_field_direction() {
    let src = format!("{HANDSHAKE}module M {{ in hs : Handshake\n hs.ready = true }}");
    let res = p(&src);
    assert!(res.diagnostics.is_empty(), "{:?}", res.error_codes());
    assert_eq!(
        ports(&res, 1),
        vec![
            (PortDir::In, "hs_data".into()),
            (PortDir::In, "hs_valid".into()),
            (PortDir::Out, "hs_ready".into()),
        ]
    );
}

#[test]
fn inout_bundle_keeps_declared_directions() {
    let src = format!("{HANDSHAKE}module M {{ inout hs : Handshake }}");
    let res = p(&src);
    assert_eq!(ports(&res, 1)[0], (PortDir::Out, "hs_data".into()));
    assert_eq!(ports(&res, 1)[2], (PortDir::In, "hs_ready".into()));
}

#[test]
fn flattened_ports_keep_their_place_among_plain_ports() {
    let src = format!("{HANDSHAKE}module M {{ in clk : clock\n out hs : Handshake\n out done : bool\n done = true }}");
    let res = p(&src);
    let names: Vec<String> = ports(&res, 1).into_iter().map(|(_, n)| n).collect();
    assert_eq!(
        names,
        vec!["clk", "hs_data", "hs_valid", "hs_ready", "done"]
    );
}

#[test]
fn bundle_declared_after_module_still_flattens() {
    let src = format!("module M {{ out hs : Handshake\n hs.data = 0 }}\n{HANDSHAKE}");
    let res = p(&src);
    assert!(res.diagnostics.is_empty(), "{:?}", res.error_codes());
    assert_eq!(ports(&res, 0).len(), 3);
}

#[test]
fn two_bundle_ports_of_same_type_get_distinct_prefixes() {
    let src =
        format!("{HANDSHAKE}module M {{ in a : Handshake\n out b : Handshake\n b.data = a.data }}");
    let res = p(&src);
    let names: Vec<String> = ports(&res, 1).into_iter().map(|(_, n)| n).collect();
    assert_eq!(
        names,
        vec!["a_data", "a_valid", "a_ready", "b_data", "b_valid", "b_ready"]
    );
}

// ═══ İç içe bundle ════════════════════════════════════════════════

const LINK: &str = "struct port Chan {\n    out data  : u8\n    in  ready : bool\n}\nstruct port Link {\n    out tx : Chan\n    in  rx : Chan\n}\n";

#[test]
fn nested_bundle_flattens_with_double_prefix() {
    let src = format!("{LINK}module M {{ out link : Link }}");
    let res = p(&src);
    let names: Vec<String> = ports(&res, 2).into_iter().map(|(_, n)| n).collect();
    assert_eq!(
        names,
        vec![
            "link_tx_data",
            "link_tx_ready",
            "link_rx_data",
            "link_rx_ready"
        ]
    );
}

#[test]
fn nested_in_field_flips_inner_directions() {
    let src = format!("{LINK}module M {{ out link : Link }}");
    let res = p(&src);
    let ps = ports(&res, 2);
    // tx: Chan bildirildiği gibi; rx: 'in rx' Chan'ı tersler.
    assert_eq!(ps[0], (PortDir::Out, "link_tx_data".into()));
    assert_eq!(ps[1], (PortDir::In, "link_tx_ready".into()));
    assert_eq!(ps[2], (PortDir::In, "link_rx_data".into()));
    assert_eq!(ps[3], (PortDir::Out, "link_rx_ready".into()));
}

#[test]
fn in_port_of_nested_bundle_flips_twice_on_in_fields() {
    let src = format!("{LINK}module M {{ in link : Link }}");
    let res = p(&src);
    let ps = ports(&res, 2);
    assert_eq!(ps[0], (PortDir::In, "link_tx_data".into()));
    assert_eq!(ps[1], (PortDir::Out, "link_tx_ready".into()));
    assert_eq!(ps[2], (PortDir::Out, "link_rx_data".into()));
    assert_eq!(ps[3], (PortDir::In, "link_rx_ready".into()));
}

#[test]
fn self_referential_bundle_terminates() {
    let src = "struct port Loop { out inner : Loop }\nmodule M { in l : Loop }";
    let res = p(src);
    // Sonsuz açılım yok; derinlik sınırı ile boş port listesi.
    assert!(ports(&res, 1).len() < 64);
}

// ═══ İfade yeniden yazımı ═════════════════════════════════════════

#[test]
fn field_access_in_module_body_rewritten_to_flat_path() {
    let src = format!("{HANDSHAKE}module M {{ in hs : Handshake\n out d : u8\n d = hs.data }}");
    let res = p(&src);
    let m = module(&res, 1);
    let StmtKind::Assign(a) = &res.ast.stmts[m.body[0]].kind else {
        panic!("atama bekleniyor");
    };
    assert_eq!(a.lhs.base.text, "d");
    assert_eq!(path_text(&res, a.rhs).as_deref(), Some("hs_data"));
}

#[test]
fn lvalue_field_suffix_rewritten_to_flat_base() {
    let src = format!("{HANDSHAKE}module M {{ out hs : Handshake\n hs.data = 0 }}");
    let res = p(&src);
    let m = module(&res, 1);
    let StmtKind::Assign(a) = &res.ast.stmts[m.body[0]].kind else {
        panic!("atama bekleniyor");
    };
    assert_eq!(a.lhs.base.text, "hs_data");
    assert!(a.lhs.suffixes.is_empty());
}

#[test]
fn lvalue_index_suffix_survives_rewrite() {
    let src = format!("{HANDSHAKE}module M {{ out hs : Handshake\n hs.data[3] = true }}");
    let res = p(&src);
    let m = module(&res, 1);
    let StmtKind::Assign(a) = &res.ast.stmts[m.body[0]].kind else {
        panic!("atama bekleniyor");
    };
    assert_eq!(a.lhs.base.text, "hs_data");
    assert_eq!(a.lhs.suffixes.len(), 1);
}

#[test]
fn field_access_in_contract_rewritten() {
    let src = format!("{HANDSHAKE}module M {{ in hs : Handshake\n cover: hs.valid\n }}");
    let res = p(&src);
    let m = module(&res, 1);
    assert_eq!(
        path_text(&res, m.contracts[0].expr).as_deref(),
        Some("hs_valid")
    );
}

#[test]
fn field_access_inside_on_block_rewritten() {
    let src = format!(
        "{HANDSHAKE}module M {{ in clk : clock\n in hs : Handshake\n reg r : u8 = 0\n on clk {{ if hs.valid {{ r <= hs.data }} }} }}"
    );
    let res = p(&src);
    let m = module(&res, 1);
    let StmtKind::On(on) = &res.ast.stmts[m.body[1]].kind else {
        panic!("on bloğu bekleniyor");
    };
    let BlockStmt::If(ifstmt) = &res.ast.blocks[on.body].stmts[0] else {
        panic!("if bekleniyor");
    };
    assert_eq!(path_text(&res, ifstmt.cond).as_deref(), Some("hs_valid"));
    let BlockStmt::NonBlockAssign { rhs, .. } = &res.ast.blocks[ifstmt.then_block].stmts[0] else {
        panic!("<= bekleniyor");
    };
    assert_eq!(path_text(&res, *rhs).as_deref(), Some("hs_data"));
}

#[test]
fn nested_field_chain_rewritten() {
    let src = format!("{LINK}module M {{ in link : Link\n link.rx.data = link.tx.data }}");
    let res = p(&src);
    let m = module(&res, 2);
    let StmtKind::Assign(a) = &res.ast.stmts[m.body[0]].kind else {
        panic!("atama bekleniyor");
    };
    assert_eq!(a.lhs.base.text, "link_rx_data");
    assert_eq!(path_text(&res, a.rhs).as_deref(), Some("link_tx_data"));
}

#[test]
fn unknown_bundle_field_is_left_untouched() {
    let src = format!("{HANDSHAKE}module M {{ in hs : Handshake\n out d : u8\n d = hs.bogus }}");
    let res = p(&src);
    let m = module(&res, 1);
    let StmtKind::Assign(a) = &res.ast.stmts[m.body[0]].kind else {
        panic!("atama bekleniyor");
    };
    assert!(matches!(res.ast.exprs[a.rhs].kind, ExprKind::Field { .. }));
}

#[test]
fn field_access_on_non_bundle_name_is_left_untouched() {
    let src = format!("{HANDSHAKE}module M {{ in hs : Handshake\n out d : u8\n d = other.data }}");
    let res = p(&src);
    let m = module(&res, 1);
    let StmtKind::Assign(a) = &res.ast.stmts[m.body[0]].kind else {
        panic!("atama bekleniyor");
    };
    assert!(matches!(res.ast.exprs[a.rhs].kind, ExprKind::Field { .. }));
}

// ═══ Kaynak bilgisi, domain, span ═════════════════════════════════

#[test]
fn bundle_origin_records_port_bundle_path_and_flip() {
    let src = format!("{HANDSHAKE}module M {{ in hs : Handshake }}");
    let res = p(&src);
    let m = module(&res, 1);
    let origin = m.ports[2].bundle.as_ref().expect("bundle kaynağı");
    assert_eq!(origin.port.text, "hs");
    assert_eq!(origin.bundle, "Handshake");
    assert_eq!(origin.path, "ready");
    assert_eq!(origin.declared, PortDir::In);
    assert!(origin.flipped);
}

#[test]
fn nested_origin_path_is_dotted() {
    let src = format!("{LINK}module M {{ out link : Link }}");
    let res = p(&src);
    let m = module(&res, 2);
    let origin = m.ports[3].bundle.as_ref().expect("bundle kaynağı");
    assert_eq!(origin.path, "rx.ready");
    assert!(origin.flipped, "in rx alanı Chan'ı tersler");
}

#[test]
fn plain_ports_have_no_bundle_origin() {
    let res = p("module M { in a : u8\n out b : u8\n b = a }");
    assert!(module(&res, 0).ports.iter().all(|p| p.bundle.is_none()));
}

#[test]
fn port_level_domain_annotation_propagates_to_fields() {
    let src = format!("{HANDSHAKE}module M {{ in clk : clock @Fast\n in hs : Handshake @Fast }}");
    let res = p(&src);
    let m = module(&res, 1);
    assert!(m.ports[1..]
        .iter()
        .all(|p| p.domain.as_ref().map(|d| d.text.as_str()) == Some("Fast")));
}

#[test]
fn field_level_domain_annotation_overrides_port_annotation() {
    let src = "struct port Bus { out data : u8 @Slow\n in ready : bool }\nmodule M { in bus : Bus @Fast }";
    let res = p(src);
    let m = module(&res, 1);
    assert_eq!(m.ports[0].domain.as_ref().unwrap().text, "Slow");
    assert_eq!(m.ports[1].domain.as_ref().unwrap().text, "Fast");
}

#[test]
fn struct_port_field_domain_is_parsed() {
    let res = p("struct port Bus { out data : u8 @Slow }");
    let ItemKind::Struct(s) = &res.ast.items_arena[res.ast.items[0]].kind else {
        panic!("struct bekleniyor");
    };
    assert_eq!(s.fields[0].direction, Some(PortDir::Out));
    assert_eq!(s.fields[0].domain.as_ref().unwrap().text, "Slow");
}

#[test]
fn flattened_name_spans_are_unique() {
    let src = format!("{LINK}module M {{ in a : Link\n out b : Link }}");
    let res = p(&src);
    let spans: HashSet<_> = module(&res, 2).ports.iter().map(|p| p.name.span).collect();
    assert_eq!(spans.len(), 8, "decl_spans anahtarı çakışmamalı");
}

#[test]
fn flattened_name_spans_stay_on_the_port_line() {
    let src = format!("{HANDSHAKE}module M {{\n    in hs : Handshake\n}}");
    let res = p(&src);
    let m = module(&res, 1);
    let decl = m.ports[0].span;
    for port in &m.ports {
        assert!(port.name.span.start >= decl.start && port.name.span.end <= decl.end);
    }
}

// ═══ Sözdizimi hataları ═══════════════════════════════════════════

#[test]
fn struct_port_field_without_direction_is_parse_error() {
    let res = p("struct port Bad { data : u8 }");
    assert!(
        !res.diagnostics.is_empty(),
        "yönsüz struct port alanı hata vermeli"
    );
}

#[test]
fn plain_struct_field_with_direction_is_parse_error() {
    let res = p("struct Plain { out data : u8 }");
    assert!(
        !res.diagnostics.is_empty(),
        "sıradan struct alanı yön taşıyamaz"
    );
}

#[test]
fn generic_struct_port_is_not_flattened() {
    let res = p("struct port G<T> { out d : T }\nmodule M { in g : G<u8> }");
    assert_eq!(ports(&res, 1), vec![(PortDir::In, "g".into())]);
}

#[test]
fn plain_struct_type_port_is_not_flattened() {
    let res = p("struct Plain { d : u8 }\nmodule M { in g : Plain }");
    assert_eq!(ports(&res, 1), vec![(PortDir::In, "g".into())]);
}
