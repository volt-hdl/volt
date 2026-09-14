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

// ═══ Yerleşik Handshake<T> (ADR-0050) ═════════════════════════════

const PRODUCER: &str = "module P {\n    in  clk : clock\n    out tx : Handshake<u8>\n    reg v : bool = false\n    on clk { if tx.fired { v <= false } else { v <= true } }\n    tx.data = 7\n    tx.valid = v\n}\n";

#[test]
fn builtin_handshake_ports_are_flat() {
    let out = sv(PRODUCER);
    assert!(port_line(&out, "tx_data")
        .trim_start()
        .starts_with("output"));
    assert!(port_line(&out, "tx_valid")
        .trim_start()
        .starts_with("output"));
    assert!(port_line(&out, "tx_ready")
        .trim_start()
        .starts_with("input"));
    assert!(!out.contains("interface"), "{out}");
}

#[test]
fn builtin_handshake_struct_payload_ports_are_flat_and_typed() {
    let src = "struct Req { addr : u32, prot : u3 }\nmodule C {\n    in  clk : clock\n    in  req : Handshake<Req>\n    out a : u32\n    req.ready = req.data.prot == 0\n    a = req.data.addr\n}\n";
    let out = sv(src);
    let addr = port_line(&out, "req_data_addr");
    assert!(addr.contains("input") && addr.contains("[31:0]"), "{addr}");
    let prot = port_line(&out, "req_data_prot");
    assert!(prot.contains("[2:0]"), "{prot}");
    assert!(port_line(&out, "req_ready")
        .trim_start()
        .starts_with("output"));
}

#[test]
fn builtin_handshake_fired_emits_valid_and_ready() {
    let out = sv(PRODUCER);
    assert!(out.contains("tx_valid && tx_ready"), "{out}");
}

#[test]
fn builtin_handshake_auto_contracts_reach_immediate_sva() {
    let parsed = volt_syntax::parser::parse(FileId(0), PRODUCER);
    assert!(parsed.diagnostics.is_empty());
    let out = volt_sv_emit::emit_full(
        &parsed.ast,
        "test.volt",
        PRODUCER,
        volt_sv_emit::SvaMode::Immediate,
    );
    assert!(
        !out.diagnostics
            .iter()
            .any(|d| d.code.as_str().starts_with('E')),
        "{:?}",
        out.diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>()
    );
    let sv = out.sv;
    // valid, ready gelene dek düşmez; veri sabit — geçmiş register zinciri.
    assert!(sv.contains("past_tx_valid_1"), "{sv}");
    assert!(sv.contains("past_tx_ready_1"), "{sv}");
    assert!(
        sv.contains("assert (!(past_tx_valid_1 && !past_tx_ready_1) || tx_valid)"),
        "{sv}"
    );
    assert!(sv.contains("tx_data == past_tx_data_1"), "{sv}");
}

#[test]
fn builtin_handshake_consumer_contracts_are_assumptions() {
    let src = "module C {\n    in  clk : clock\n    in  rx : Handshake<u8>\n    out d : u8\n    rx.ready = true\n    d = rx.data\n}\n";
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    let out = volt_sv_emit::emit_full(
        &parsed.ast,
        "test.volt",
        src,
        volt_sv_emit::SvaMode::Immediate,
    );
    assert!(
        out.sv
            .contains("assume (!(past_rx_valid_1 && !past_rx_ready_1) || rx_valid)"),
        "{}",
        out.sv
    );
    assert!(!out.sv.contains("assert (!(past_rx_valid_1"), "{}", out.sv);
}

#[test]
fn builtin_handshake_rtl_output_has_no_past_registers() {
    let out = sv(PRODUCER);
    assert!(!out.contains("past_"), "{out}");
}
