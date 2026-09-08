//! Yerleşik CDC primitifleri (ADR-0027): AsyncFifo, HandshakeSync,
//! PulseSync — isim çözümleme, tip kontrolü ve domain denetimi.

use volt_hir::{analyze, AnalysisResult};
use volt_syntax::{parse, FileId};

fn check(src: &str) -> AnalysisResult {
    let parsed = parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "kaynak ayrışmalı: {:?}",
        parsed.error_codes()
    );
    analyze(&parsed.ast)
}

fn codes(src: &str) -> Vec<&'static str> {
    check(src).error_codes()
}

/// İki saatli test modülleri için ortak önsöz.
const TWO_DOMAINS: &str = "domain Fast { clock = posedge, reset = sync active_high }\n\
                           domain Slow { clock = posedge, reset = sync active_high }\n";

fn fifo_module(inst: &str, extra: &str) -> String {
    format!(
        "{TWO_DOMAINS}module M {{\n    in  fast_clk : clock @Fast\n    \
         in  slow_clk : clock @Slow\n    in  din  : u8   @Fast\n    \
         in  push : bool @Fast\n    in  pop  : bool @Slow\n    \
         out dout : u8   @Slow\n    out full : bool @Fast\n    \
         out empty : bool @Slow\n\n{inst}\n\n    full = f.wr_full\n    \
         dout = f.rd_data\n    empty = f.rd_empty\n{extra}}}\n"
    )
}

const FIFO_OK: &str = "    let f = AsyncFifo<u8, 16> { wr_clk: fast_clk, wr_data: din, \
                       wr_en: push, rd_clk: slow_clk, rd_en: pop }";

// ═══ Temiz geçmesi gerekenler ═════════════════════════════════════

#[test]
fn async_fifo_resolves_and_typechecks_clean() {
    let result = check(&fifo_module(FIFO_OK, ""));
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn handshake_sync_data_flow_is_clean() {
    let src = format!(
        "{TWO_DOMAINS}module M {{\n    in  fast_clk : clock @Fast\n    \
         in  slow_clk : clock @Slow\n    in  din : u8 @Fast\n    in  s : bool @Fast\n    \
         out busy : bool @Fast\n    out dout : u8 @Slow\n    out valid : bool @Slow\n\n    \
         let hs = HandshakeSync<u8> {{ src_clk: fast_clk, data_in: din, send: s, \
         dst_clk: slow_clk }}\n\n    busy = hs.busy\n    dout = hs.data_out\n    \
         valid = hs.valid\n}}\n"
    );
    let result = check(&src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn pulse_sync_emits_w3005_and_no_errors() {
    let src = format!(
        "{TWO_DOMAINS}module M {{\n    in  fast_clk : clock @Fast\n    \
         in  slow_clk : clock @Slow\n    in  p : bool @Fast\n    out q : bool @Slow\n\n    \
         let ps = PulseSync {{ src_clk: fast_clk, pulse_in: p, dst_clk: slow_clk }}\n\n    \
         q = ps.pulse_out\n}}\n"
    );
    let result = check(&src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
    assert!(
        result.error_codes().contains(&"W3005"),
        "W3005 bekleniyor: {:?}",
        result.error_codes()
    );
}

// ═══ Generic argüman hataları ═════════════════════════════════════

#[test]
fn fifo_depth_not_power_of_two_is_e2025() {
    let inst = "    let f = AsyncFifo<u8, 12> { wr_clk: fast_clk, wr_data: din, \
                wr_en: push, rd_clk: slow_clk, rd_en: pop }";
    let codes = codes(&fifo_module(inst, ""));
    assert!(codes.contains(&"E2025"), "E2025 bekleniyor: {codes:?}");
}

#[test]
fn fifo_depth_one_is_e2025() {
    let inst = "    let f = AsyncFifo<u8, 1> { wr_clk: fast_clk, wr_data: din, \
                wr_en: push, rd_clk: slow_clk, rd_en: pop }";
    let codes = codes(&fifo_module(inst, ""));
    assert!(codes.contains(&"E2025"), "E2025 bekleniyor: {codes:?}");
}

#[test]
fn fifo_depth_not_literal_is_e2008() {
    let inst = "    let f = AsyncFifo<u8, DEPTH_N> { wr_clk: fast_clk, wr_data: din, \
                wr_en: push, rd_clk: slow_clk, rd_en: pop }";
    let src = format!("const DEPTH_N : u32 = 16\n{}", fifo_module(inst, ""));
    let codes = codes(&src);
    assert!(codes.contains(&"E2008"), "E2008 bekleniyor: {codes:?}");
}

#[test]
fn fifo_wrong_generic_arity_is_e2003() {
    let inst = "    let f = AsyncFifo<u8> { wr_clk: fast_clk, wr_data: din, \
                wr_en: push, rd_clk: slow_clk, rd_en: pop }";
    let codes = codes(&fifo_module(inst, ""));
    assert!(codes.contains(&"E2003"), "E2003 bekleniyor: {codes:?}");
}

#[test]
fn handshake_sync_without_type_arg_is_e2003() {
    let src = format!(
        "{TWO_DOMAINS}module M {{\n    in  fast_clk : clock @Fast\n    \
         in  slow_clk : clock @Slow\n    in  p : bool @Fast\n    out q : bool @Slow\n\n    \
         let hs = HandshakeSync {{ src_clk: fast_clk, data_in: p, send: p, \
         dst_clk: slow_clk }}\n\n    q = hs.valid\n}}\n"
    );
    let codes = codes(&src);
    assert!(codes.contains(&"E2003"), "E2003 bekleniyor: {codes:?}");
}

// ═══ Port hataları ════════════════════════════════════════════════

#[test]
fn fifo_unknown_port_is_e1009() {
    let inst = "    let f = AsyncFifo<u8, 16> { wr_clk: fast_clk, wr_data: din, \
                wr_enable: push, rd_clk: slow_clk, rd_en: pop }";
    let codes = codes(&fifo_module(inst, ""));
    assert!(codes.contains(&"E1009"), "E1009 bekleniyor: {codes:?}");
}

#[test]
fn binding_an_output_port_is_e1009() {
    let inst = "    let f = AsyncFifo<u8, 16> { wr_clk: fast_clk, wr_data: din, \
                wr_en: push, rd_clk: slow_clk, rd_en: pop, wr_full: push }";
    let result = check(&fifo_module(inst, ""));
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E1009")
        .expect("E1009 bekleniyor");
    assert!(
        diag.help.as_deref().unwrap_or("").contains("field access")
            || diag.help.as_deref().unwrap_or("").contains("alan erişimi"),
        "yardım metni alan erişimini önermeli: {:?}",
        diag.help
    );
}

#[test]
fn reading_an_input_port_via_field_is_e1009() {
    let extra = "    wire w : bool\n    w = f.wr_en\n";
    let codes = codes(&fifo_module(FIFO_OK, extra));
    assert!(codes.contains(&"E1009"), "E1009 bekleniyor: {codes:?}");
}

// ═══ Tip ve domain denetimi ═══════════════════════════════════════

#[test]
fn fifo_wr_data_type_mismatch_is_error() {
    // din u8 iken T=u16 — E2001/E2003 genişlik uyumsuzluğu.
    let inst = "    let f = AsyncFifo<u16, 16> { wr_clk: fast_clk, wr_data: din, \
                wr_en: push, rd_clk: slow_clk, rd_en: pop }";
    let src = format!(
        "{TWO_DOMAINS}module M {{\n    in  fast_clk : clock @Fast\n    \
         in  slow_clk : clock @Slow\n    in  din : u8 @Fast\n    in  push : bool @Fast\n    \
         in  pop : bool @Slow\n    out dout : u16 @Slow\n\n{inst}\n\n    \
         dout = f.rd_data\n}}\n"
    );
    let codes = codes(&src);
    assert!(
        codes.iter().any(|c| c.starts_with("E2")),
        "tip hatası bekleniyor: {codes:?}"
    );
}

#[test]
fn fifo_src_side_binding_from_wrong_domain_is_e3001() {
    // wr_data hedefi kaynak (Fast) alanında; Slow alanından veri bağlamak CDC ihlali.
    let inst = "    let f = AsyncFifo<u8, 16> { wr_clk: fast_clk, wr_data: sdin, \
                wr_en: push, rd_clk: slow_clk, rd_en: pop }";
    let src = format!(
        "{TWO_DOMAINS}module M {{\n    in  fast_clk : clock @Fast\n    \
         in  slow_clk : clock @Slow\n    in  sdin : u8 @Slow\n    in  push : bool @Fast\n    \
         in  pop : bool @Slow\n    out dout : u8 @Slow\n\n{inst}\n\n    \
         dout = f.rd_data\n}}\n"
    );
    let codes = codes(&src);
    assert!(codes.contains(&"E3001"), "E3001 bekleniyor: {codes:?}");
}

#[test]
fn fifo_output_field_carries_read_domain() {
    // rd_data hedef (Slow) alanındadır; Fast çıkışa atamak E3001.
    let src = format!(
        "{TWO_DOMAINS}module M {{\n    in  fast_clk : clock @Fast\n    \
         in  slow_clk : clock @Slow\n    in  din : u8 @Fast\n    in  push : bool @Fast\n    \
         in  pop : bool @Slow\n    out dout : u8 @Fast\n\n    \
         let f = AsyncFifo<u8, 16> {{ wr_clk: fast_clk, wr_data: din, wr_en: push, \
         rd_clk: slow_clk, rd_en: pop }}\n\n    dout = f.rd_data\n}}\n"
    );
    let codes = codes(&src);
    assert!(codes.contains(&"E3001"), "E3001 bekleniyor: {codes:?}");
}

#[test]
fn user_module_named_like_builtin_wins() {
    // Kullanıcı 'PulseSync' adında modül tanımlarsa yerleşik devreye girmez:
    // bilinmeyen port E1009 kullanıcı modülüne göre raporlanır, W3005 üretilmez.
    let src = "module PulseSync { in clk : clock in a : bool out b : bool b = a }\n\
               module M { in clk : clock in x : bool out y : bool \
               let u = PulseSync { clk: clk, a: x } y = u.b }";
    let result = check(src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
    assert!(
        !result.error_codes().contains(&"W3005"),
        "kullanıcı modülü yerleşiği gölgelemeli: {:?}",
        result.error_codes()
    );
}
