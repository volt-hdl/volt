//! Önceden kapsanmayan editör yolları (ADR-0091 Adım 4): anahtar kelime
//! belgeleri, tüm öğe türlerinin sembolleri, tanım türü etiketleri,
//! saat alanı satırının kenar/kaynak biçimleri, const genişlikli struct
//! düzeni.

use tower_lsp::lsp_types::SymbolKind;
use volt_lsp::{analysis, docs, hover, symbols};
use volt_syntax::TokenKind;

fn hover_at(src: &str, needle: &str) -> String {
    let a = analysis::analyze_editor("test.volt", src);
    let offset = src.find(needle).expect("metin kaynakta olmalı") as u32;
    hover::hover(&a, offset)
        .map(|(md, _)| md)
        .unwrap_or_default()
}

#[test]
fn every_taught_keyword_has_a_doc() {
    use TokenKind::*;
    for kind in [
        KwClock, KwReg, KwWire, KwLet, KwOn, KwComb, KwDomain, KwModule, KwIn, KwOut, KwInout,
        KwReset, KwIf, KwMatch, KwFor, KwConst, KwBits, Le, Eq, At,
    ] {
        let doc = docs::keyword_doc(kind).unwrap_or_else(|| panic!("{kind:?} belgesiz"));
        assert!(doc.starts_with("**"), "{kind:?}: {doc}");
    }
    assert!(docs::keyword_doc(Ident).is_none());
}

const ITEMS: &str = "\
domain Sys {
    clock = negedge
    reset = sync active_high
}

const W : u32 = 4

struct Pair {
    hi : uint<W>
    lo : u4
}

enum Mode { Idle, Run }

type Byte = u8

fn twice(x: u8) -> u8 {
    x + x
}

module Leaf {
    in  a : u8
    out b : u8
    b = a
}

module Top {
    in    clk  : clock @Sys
    in    x    : u8
    inout pad  : bool
    out   y    : u8
    out   z    : u8

    wire w : u8
    reg  p : Pair = Pair { hi: 0, lo: 0 }
    let  leaf = Leaf { a: x }
    let  k = twice(x)

    on clk {
        p <= Pair { hi: 1, lo: 2 }
        if x == 0 {
            pad.drive(true)
        } else {
            pad.release()
        }
    }

    w = leaf.b
    y = w
    z = k
}
";

#[test]
fn symbols_cover_every_item_kind() {
    let a = analysis::analyze_editor("items.volt", ITEMS);
    let syms = symbols::document_symbols(&a);
    let kind_of = |name: &str| {
        syms.iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| panic!("{name} sembolü yok"))
            .kind
    };
    assert_eq!(kind_of("Sys"), SymbolKind::NAMESPACE);
    assert_eq!(kind_of("W"), SymbolKind::CONSTANT);
    assert_eq!(kind_of("Pair"), SymbolKind::STRUCT);
    assert_eq!(kind_of("Mode"), SymbolKind::ENUM);
    assert_eq!(kind_of("twice"), SymbolKind::FUNCTION);

    let top = syms.iter().find(|s| s.name == "Top").expect("Top");
    let children = top.children.as_ref().expect("çocuklar");
    let child = |name: &str| {
        children
            .iter()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("{name} yok"))
    };
    assert_eq!(child("pad").kind, SymbolKind::INTERFACE);
    assert_eq!(child("pad").detail.as_deref(), Some("inout"));
    assert_eq!(child("w").detail.as_deref(), Some("wire"));
    assert_eq!(child("p").detail.as_deref(), Some("reg"));
}

#[test]
fn hover_labels_each_kind_of_definition() {
    for (needle, label) in [
        ("W : u32", "const"),
        ("Byte = u8", "type alias"),
        ("leaf = Leaf", "module instance"),
        ("w : u8", "wire"),
        ("pad  : bool", "inout port"),
        ("y    : u8", "output port"),
        ("Sys {", "clock domain"),
        ("Leaf {\n", "module"),
    ] {
        let md = hover_at(ITEMS, needle);
        assert!(md.contains(label), "{needle}: {md}");
    }
}

#[test]
fn hover_shows_a_declared_negedge_domain() {
    let md = hover_at(ITEMS, "x    : u8");
    assert!(md.contains("@Sys (negedge)"), "{md}");
}

#[test]
fn hover_shows_timeless_for_a_constant_binding() {
    let src = "module M {\n    in  a : u8\n    out y : u8\n    let c = 3u8\n    y = a + c\n}\n";
    let md = hover_at(src, "c = 3u8");
    assert!(md.contains("timeless"), "{md}");
}

#[test]
fn hover_lays_out_a_struct_with_a_const_width_field() {
    let md = hover_at(ITEMS, "p : Pair");
    assert!(md.contains("Pair (8 bits"), "{md}");
    assert!(md.contains("hi: uint<W> [7:4]"), "{md}");
}
