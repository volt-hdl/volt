//! Tek saatli stdlib yapı taşları (ADR-0029): SyncFifo, Ram,
//! DualPortRam, Counter, ShiftRegister, RoundRobinArbiter,
//! PriorityArbiter, EdgeDetect — isim çözümleme, tip kontrolü ve
//! domain denetimi.

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

fn assert_clean(src: &str) {
    let result = check(src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

fn sync_fifo_module(inst: &str) -> String {
    format!(
        "module M {{\n    in  clk  : clock\n    in  din  : u8\n    in  push : bool\n    \
         in  pop  : bool\n    out dout : u8\n    out full : bool\n    out empty : bool\n\n\
         {inst}\n\n    dout = f.rd_data\n    full = f.full\n    empty = f.empty\n}}\n"
    )
}

const SYNC_FIFO_OK: &str =
    "    let f = SyncFifo<u8, 16> { clk: clk, wr_data: din, wr_en: push, rd_en: pop }";

// ═══ SyncFifo ═════════════════════════════════════════════════════

#[test]
fn sync_fifo_resolves_and_typechecks_clean() {
    assert_clean(&sync_fifo_module(SYNC_FIFO_OK));
}

#[test]
fn sync_fifo_depth_not_power_of_two_is_e2025() {
    let inst = "    let f = SyncFifo<u8, 10> { clk: clk, wr_data: din, wr_en: push, rd_en: pop }";
    let codes = codes(&sync_fifo_module(inst));
    assert!(codes.contains(&"E2025"), "E2025 bekleniyor: {codes:?}");
}

#[test]
fn sync_fifo_depth_above_range_is_e2025() {
    let inst =
        "    let f = SyncFifo<u8, 131072> { clk: clk, wr_data: din, wr_en: push, rd_en: pop }";
    let codes = codes(&sync_fifo_module(inst));
    assert!(codes.contains(&"E2025"), "E2025 bekleniyor: {codes:?}");
}

#[test]
fn sync_fifo_depth_not_literal_is_e2008() {
    let inst =
        "    let f = SyncFifo<u8, DEPTH_N> { clk: clk, wr_data: din, wr_en: push, rd_en: pop }";
    let src = format!("const DEPTH_N : u32 = 16\n{}", sync_fifo_module(inst));
    let codes = codes(&src);
    assert!(codes.contains(&"E2008"), "E2008 bekleniyor: {codes:?}");
}

#[test]
fn sync_fifo_wrong_generic_arity_is_e2003() {
    let inst = "    let f = SyncFifo<u8> { clk: clk, wr_data: din, wr_en: push, rd_en: pop }";
    let codes = codes(&sync_fifo_module(inst));
    assert!(codes.contains(&"E2003"), "E2003 bekleniyor: {codes:?}");
}

#[test]
fn sync_fifo_unknown_port_is_e1009() {
    let inst =
        "    let f = SyncFifo<u8, 16> { clk: clk, wr_data: din, wr_enable: push, rd_en: pop }";
    let codes = codes(&sync_fifo_module(inst));
    assert!(codes.contains(&"E1009"), "E1009 bekleniyor: {codes:?}");
}

#[test]
fn sync_fifo_binding_output_port_is_e1009() {
    let inst = "    let f = SyncFifo<u8, 16> { clk: clk, wr_data: din, wr_en: push, \
                rd_en: pop, full: push }";
    let codes = codes(&sync_fifo_module(inst));
    assert!(codes.contains(&"E1009"), "E1009 bekleniyor: {codes:?}");
}

#[test]
fn sync_fifo_cross_domain_binding_is_e3001() {
    // Tek saatli primitifin TÜM portları clk alanındadır; başka alandan
    // veri bağlamak CDC ihlali.
    let src = "domain Fast { clock = posedge, reset = sync active_high }\n\
               domain Slow { clock = posedge, reset = sync active_high }\n\
               module M {\n    in  fclk : clock @Fast\n    in  sclk : clock @Slow\n    \
               in  din  : u8 @Slow\n    in  push : bool @Fast\n    in  pop : bool @Fast\n    \
               out dout : u8 @Fast\n    out sq : bool @Slow\n\n    \
               let f = SyncFifo<u8, 16> { clk: fclk, wr_data: din, wr_en: push, rd_en: pop }\n\n    \
               dout = f.rd_data\n    sq = sclk == sclk\n}\n";
    let parsed = parse(FileId(0), src);
    let result = analyze(&parsed.ast);
    assert!(
        result.error_codes().contains(&"E3001"),
        "E3001 bekleniyor: {:?}",
        result.error_codes()
    );
}

// ═══ Ram / DualPortRam ════════════════════════════════════════════

#[test]
fn ram_resolves_and_typechecks_clean() {
    assert_clean(
        "module M {\n    in  clk : clock\n    in  addr : bits<8>\n    in  wd : u16\n    \
         in  we : bool\n    out rd : u16\n\n    \
         let m = Ram<u16, 256> { clk: clk, addr: addr, wr_data: wd, wr_en: we }\n\n    \
         rd = m.rd_data\n}\n",
    );
}

#[test]
fn ram_addr_width_mismatch_is_type_error() {
    // 256 derinlik 8 bit adres ister; bits<7> bağlamak tip hatası.
    let codes = codes(
        "module M {\n    in  clk : clock\n    in  addr : bits<7>\n    in  wd : u16\n    \
         in  we : bool\n    out rd : u16\n\n    \
         let m = Ram<u16, 256> { clk: clk, addr: addr, wr_data: wd, wr_en: we }\n\n    \
         rd = m.rd_data\n}\n",
    );
    assert!(
        codes.iter().any(|c| c.starts_with("E2")),
        "tip hatası bekleniyor: {codes:?}"
    );
}

#[test]
fn ram_depth_not_power_of_two_is_e2025() {
    let codes = codes(
        "module M {\n    in  clk : clock\n    in  addr : bits<8>\n    in  wd : u16\n    \
         in  we : bool\n    out rd : u16\n\n    \
         let m = Ram<u16, 200> { clk: clk, addr: addr, wr_data: wd, wr_en: we }\n\n    \
         rd = m.rd_data\n}\n",
    );
    assert!(codes.contains(&"E2025"), "E2025 bekleniyor: {codes:?}");
}

#[test]
fn dual_port_ram_is_clean_and_warns_w3006() {
    let result = check(
        "module M {\n    in  clk : clock\n    in  aa : bits<6>\n    in  ad : u8\n    \
         in  aw : bool\n    out aq : u8\n    in  ba : bits<6>\n    in  bd : u8\n    \
         in  bw : bool\n    out bq : u8\n\n    \
         let m = DualPortRam<u8, 64> { clk: clk, a_addr: aa, a_wr_data: ad, a_wr_en: aw, \
         b_addr: ba, b_wr_data: bd, b_wr_en: bw }\n\n    \
         aq = m.a_rd_data\n    bq = m.b_rd_data\n}\n",
    );
    assert!(!result.has_errors(), "{:?}", result.error_codes());
    assert!(
        result.error_codes().contains(&"W3006"),
        "W3006 bekleniyor: {:?}",
        result.error_codes()
    );
}

#[test]
fn single_port_ram_does_not_warn_w3006() {
    let result = check(
        "module M {\n    in  clk : clock\n    in  addr : bits<4>\n    in  wd : u8\n    \
         in  we : bool\n    out rd : u8\n\n    \
         let m = Ram<u8, 16> { clk: clk, addr: addr, wr_data: wd, wr_en: we }\n\n    \
         rd = m.rd_data\n}\n",
    );
    assert!(
        !result.error_codes().contains(&"W3006"),
        "W3006 yalnız DualPortRam'de: {:?}",
        result.error_codes()
    );
}

// ═══ Counter ══════════════════════════════════════════════════════

#[test]
fn counter_resolves_and_typechecks_clean() {
    assert_clean(
        "module M {\n    in  clk : clock\n    in  en : bool\n    in  clr : bool\n    \
         out c : bits<8>\n    out o : bool\n\n    \
         let cnt = Counter<8> { clk: clk, enable: en, clear: clr }\n\n    \
         c = cnt.count\n    o = cnt.overflow\n}\n",
    );
}

#[test]
fn counter_count_width_mismatch_is_type_error() {
    // count bits<8>'dir; bits<9> çıkışa atamak genişlik hatası.
    let codes = codes(
        "module M {\n    in  clk : clock\n    in  en : bool\n    in  clr : bool\n    \
         out c : bits<9>\n\n    \
         let cnt = Counter<8> { clk: clk, enable: en, clear: clr }\n\n    \
         c = cnt.count\n}\n",
    );
    assert!(
        codes.iter().any(|c| c.starts_with("E2")),
        "tip hatası bekleniyor: {codes:?}"
    );
}

#[test]
fn counter_width_zero_is_e2025() {
    let codes = codes(
        "module M {\n    in  clk : clock\n    in  en : bool\n    in  clr : bool\n    \
         out c : bits<8>\n\n    \
         let cnt = Counter<0> { clk: clk, enable: en, clear: clr }\n\n    \
         c = cnt.count\n}\n",
    );
    assert!(codes.contains(&"E2025"), "E2025 bekleniyor: {codes:?}");
}

#[test]
fn counter_width_65_is_e2025() {
    let codes = codes(
        "module M {\n    in  clk : clock\n    in  en : bool\n    in  clr : bool\n    \
         out c : bits<8>\n\n    \
         let cnt = Counter<65> { clk: clk, enable: en, clear: clr }\n\n    \
         c = cnt.count\n}\n",
    );
    assert!(codes.contains(&"E2025"), "E2025 bekleniyor: {codes:?}");
}

// ═══ ShiftRegister ════════════════════════════════════════════════

#[test]
fn shift_register_bool_taps_are_len_bits() {
    assert_clean(
        "module M {\n    in  clk : clock\n    in  d : bool\n    in  s : bool\n    \
         out q : bool\n    out t : bits<8>\n\n    \
         let sr = ShiftRegister<bool, 8> { clk: clk, data_in: d, shift_en: s }\n\n    \
         q = sr.data_out\n    t = sr.taps\n}\n",
    );
}

#[test]
fn shift_register_taps_width_scales_with_data_width() {
    // T=u8, LEN=4 → taps bits<32>.
    assert_clean(
        "module M {\n    in  clk : clock\n    in  d : u8\n    in  s : bool\n    \
         out q : u8\n    out t : bits<32>\n\n    \
         let sr = ShiftRegister<u8, 4> { clk: clk, data_in: d, shift_en: s }\n\n    \
         q = sr.data_out\n    t = sr.taps\n}\n",
    );
}

#[test]
fn shift_register_len_1_is_e2025() {
    let codes = codes(
        "module M {\n    in  clk : clock\n    in  d : bool\n    in  s : bool\n    \
         out q : bool\n\n    \
         let sr = ShiftRegister<bool, 1> { clk: clk, data_in: d, shift_en: s }\n\n    \
         q = sr.data_out\n}\n",
    );
    assert!(codes.contains(&"E2025"), "E2025 bekleniyor: {codes:?}");
}

// ═══ Arbiterler ═══════════════════════════════════════════════════

fn arbiter_module(inst: &str) -> String {
    format!(
        "module M {{\n    in  clk : clock\n    in  req : bits<4>\n    out g : bits<4>\n\n\
         {inst}\n\n    g = a.grant\n}}\n"
    )
}

#[test]
fn round_robin_arbiter_is_clean() {
    assert_clean(&arbiter_module(
        "    let a = RoundRobinArbiter<4> { clk: clk, req: req }",
    ));
}

#[test]
fn priority_arbiter_is_clean() {
    assert_clean(&arbiter_module(
        "    let a = PriorityArbiter<4> { clk: clk, req: req }",
    ));
}

#[test]
fn arbiter_req_width_mismatch_is_type_error() {
    // N=8 iken bits<4> istek vektörü bağlamak genişlik hatası.
    let codes = codes(&arbiter_module(
        "    let a = RoundRobinArbiter<8> { clk: clk, req: req }",
    ));
    assert!(
        codes.iter().any(|c| c.starts_with("E2")),
        "tip hatası bekleniyor: {codes:?}"
    );
}

#[test]
fn arbiter_n_above_64_is_e2025() {
    let codes = codes(&arbiter_module(
        "    let a = RoundRobinArbiter<65> { clk: clk, req: req }",
    ));
    assert!(codes.contains(&"E2025"), "E2025 bekleniyor: {codes:?}");
}

#[test]
fn arbiter_n_1_is_e2025() {
    let codes = codes(&arbiter_module(
        "    let a = PriorityArbiter<1> { clk: clk, req: req }",
    ));
    assert!(codes.contains(&"E2025"), "E2025 bekleniyor: {codes:?}");
}

// ═══ EdgeDetect ═══════════════════════════════════════════════════

#[test]
fn edge_detect_is_clean() {
    assert_clean(
        "module M {\n    in  clk : clock\n    in  sig : bool\n    out r : bool\n    \
         out f : bool\n    out b : bool\n\n    \
         let e = EdgeDetect { clk: clk, signal: sig }\n\n    \
         r = e.rising\n    f = e.falling\n    b = e.both\n}\n",
    );
}

#[test]
fn edge_detect_with_generic_args_is_e2003() {
    let codes = codes(
        "module M {\n    in  clk : clock\n    in  sig : bool\n    out r : bool\n\n    \
         let e = EdgeDetect<u8> { clk: clk, signal: sig }\n\n    r = e.rising\n}\n",
    );
    assert!(codes.contains(&"E2003"), "E2003 bekleniyor: {codes:?}");
}

#[test]
fn edge_detect_does_not_warn_w3005() {
    // EdgeDetect senkronizatör DEĞİLDİR ama tek alanda çalıştığından
    // PulseSync'in aralık uyarısı ona uygulanmaz.
    let result = check(
        "module M {\n    in  clk : clock\n    in  sig : bool\n    out r : bool\n\n    \
         let e = EdgeDetect { clk: clk, signal: sig }\n\n    r = e.rising\n}\n",
    );
    assert!(
        !result.error_codes().contains(&"W3005"),
        "W3005 yalnız PulseSync'te: {:?}",
        result.error_codes()
    );
}

// ═══ Gölgeleme ════════════════════════════════════════════════════

#[test]
fn user_module_named_sync_fifo_wins_over_builtin() {
    let src = "module SyncFifo { in clk : clock in a : bool out b : bool b = a }\n\
               module M { in clk : clock in x : bool out y : bool \
               let u = SyncFifo { clk: clk, a: x } y = u.b }";
    let parsed = parse(FileId(0), src);
    let result = analyze(&parsed.ast);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}
