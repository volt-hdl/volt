//! Bundle (port grubu) SV üretimi — ADR-0039: bundle DÜZLEŞİR, SV
//! `interface` üretilmez; port adı `<bundle>_<alan>`.

use volt_span::FileId;
use volt_sv_emit::emit;

fn sv(src: &str) -> String {
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse hatasız olmalı: {:?}",
        parsed.error_codes()
    );
    let result = emit(&parsed.ast, "test.volt");
    assert!(
        !result.has_errors(),
        "emit hatasız olmalı: {:?}",
        result
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>()
    );
    result.sv
}

/// Verilen port adını içeren port satırı (yön + tip + ad).
fn port_line<'a>(sv: &'a str, name: &str) -> &'a str {
    sv.lines()
        .find(|l| {
            let t = l.trim_start();
            (t.starts_with("input") || t.starts_with("output") || t.starts_with("inout"))
                && t.split_whitespace()
                    .any(|w| w.trim_end_matches(',') == name)
        })
        .unwrap_or_else(|| panic!("{name} port satırı yok:\n{sv}"))
}

const HANDSHAKE: &str =
    "struct port Handshake {\n    out data  : u8\n    out valid : bool\n    in  ready : bool\n}\n";

#[test]
fn out_bundle_emits_flat_ports_with_declared_directions() {
    let src =
        format!("{HANDSHAKE}module M {{ out hs : Handshake\n hs.data = 0\n hs.valid = true }}");
    let out = sv(&src);
    assert!(port_line(&out, "hs_data")
        .trim_start()
        .starts_with("output"));
    assert!(port_line(&out, "hs_data").contains("[7:0]"));
    assert!(port_line(&out, "hs_valid")
        .trim_start()
        .starts_with("output"));
    assert!(port_line(&out, "hs_ready")
        .trim_start()
        .starts_with("input"));
}

#[test]
fn in_bundle_emits_flipped_directions() {
    let src = format!("{HANDSHAKE}module M {{ in hs : Handshake\n hs.ready = true }}");
    let out = sv(&src);
    assert!(port_line(&out, "hs_data").trim_start().starts_with("input"));
    assert!(port_line(&out, "hs_valid")
        .trim_start()
        .starts_with("input"));
    assert!(port_line(&out, "hs_ready")
        .trim_start()
        .starts_with("output"));
}

#[test]
fn nested_bundle_emits_double_prefixed_names() {
    let src = "struct port Chan { out data : u8\n in ready : bool }\nstruct port Link { out tx : Chan\n in rx : Chan }\nmodule M { in link : Link\n link.rx.data = link.tx.data\n link.tx.ready = link.rx.ready }";
    let out = sv(src);
    assert!(port_line(&out, "link_tx_data")
        .trim_start()
        .starts_with("input"));
    assert!(port_line(&out, "link_tx_ready")
        .trim_start()
        .starts_with("output"));
    assert!(port_line(&out, "link_rx_data")
        .trim_start()
        .starts_with("output"));
    assert!(port_line(&out, "link_rx_ready")
        .trim_start()
        .starts_with("input"));
}

#[test]
fn no_interface_construct_is_emitted() {
    let src = format!("{HANDSHAKE}module M {{ in hs : Handshake\n hs.ready = true }}");
    let out = sv(&src);
    assert!(!out.contains("interface"), "{out}");
    assert!(!out.contains("modport"), "{out}");
    assert!(
        !out.contains("Handshake"),
        "bundle adı SV'ye sızmamalı:\n{out}"
    );
}

#[test]
fn body_assignments_use_flat_names() {
    let src = "struct port Chan { out data : u8\n in ready : bool }\nstruct port Link { out tx : Chan\n in rx : Chan }\nmodule M { in link : Link\n link.rx.data = link.tx.data\n link.tx.ready = link.rx.ready }";
    let out = sv(src);
    assert!(out.contains("assign link_rx_data = link_tx_data;"), "{out}");
    assert!(
        out.contains("assign link_tx_ready = link_rx_ready;"),
        "{out}"
    );
}

#[test]
fn ui_pass_47_emits_both_modules() {
    let src = include_str!("../../../tests/ui/pass/47_bundle_basic.volt");
    let out = sv(src);
    assert!(out.contains("module Producer"), "{out}");
    assert!(out.contains("module Consumer"), "{out}");
    assert!(out.contains("hs_ready"));
}

#[test]
fn ui_pass_48_emits_six_flat_ports() {
    let src = include_str!("../../../tests/ui/pass/48_bundle_nested.volt");
    let out = sv(src);
    let ports = out
        .lines()
        .filter(|l| l.trim_start().starts_with("input") || l.trim_start().starts_with("output"))
        .count();
    assert_eq!(ports, 6, "{out}");
}
