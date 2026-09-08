//! Tek saatli stdlib yapı taşlarının SV üretimi (ADR-0029).
//!
//! Kapsam: gövde üretimi, alan erişimi çevirisi, Immediate modda
//! formal kontratlar ve üretilen SV'nin ASCII temizliği.

use volt_span::FileId;
use volt_sv_emit::{emit, EmitResult};

fn compile(src: &str, name: &str) -> EmitResult {
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse hatasız olmalı: {:?}",
        parsed.error_codes()
    );
    emit(&parsed.ast, name)
}

fn sv(src: &str) -> String {
    let result = compile(src, "test.volt");
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

fn emit_codes(src: &str) -> Vec<&'static str> {
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    let result = emit(&parsed.ast, "test.volt");
    result.diagnostics.iter().map(|d| d.code.as_str()).collect()
}

fn sv_immediate(src: &str) -> String {
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse hatasız olmalı: {:?}",
        parsed.error_codes()
    );
    let result = volt_sv_emit::emit_full(
        &parsed.ast,
        "test.volt",
        src,
        volt_sv_emit::SvaMode::Immediate,
    );
    let errors: Vec<&str> = result
        .diagnostics
        .iter()
        .filter(|d| !d.code.is_warning())
        .map(|d| d.code.as_str())
        .collect();
    assert!(errors.is_empty(), "emit hatasız olmalı: {errors:?}");
    result.sv
}

fn sync_fifo_src(depth: &str) -> String {
    format!(
        "module M {{ in clk : clock in din : u8 in push : bool in pop : bool \
         out dout : u8 out full : bool out empty : bool \
         let f = SyncFifo<u8, {depth}> {{ clk: clk, wr_data: din, wr_en: push, rd_en: pop }} \
         dout = f.rd_data full = f.full empty = f.empty }}"
    )
}

const RAM_SRC: &str = "module M { in clk : clock in addr : bits<4> in wd : u8 in we : bool \
     out rd : u8 \
     let m = Ram<u8, 16> { clk: clk, addr: addr, wr_data: wd, wr_en: we } \
     rd = m.rd_data }";

const DPRAM_SRC: &str = "module M { in clk : clock \
     in aa : bits<4> in ad : u8 in aw : bool out aq : u8 \
     in ba : bits<4> in bd : u8 in bw : bool out bq : u8 \
     let m = DualPortRam<u8, 16> { clk: clk, a_addr: aa, a_wr_data: ad, a_wr_en: aw, \
     b_addr: ba, b_wr_data: bd, b_wr_en: bw } \
     aq = m.a_rd_data bq = m.b_rd_data }";

const COUNTER_SRC: &str = "module M { in clk : clock in en : bool in clr : bool \
     out c : bits<8> out o : bool \
     let cnt = Counter<8> { clk: clk, enable: en, clear: clr } \
     c = cnt.count o = cnt.overflow }";

const SHIFT_SRC: &str = "module M { in clk : clock in d : bool in s : bool \
     out q : bool out t : bits<8> \
     let sr = ShiftRegister<bool, 8> { clk: clk, data_in: d, shift_en: s } \
     q = sr.data_out t = sr.taps }";

const RR_SRC: &str = "module M { in clk : clock in req : bits<4> out g : bits<4> \
     let a = RoundRobinArbiter<4> { clk: clk, req: req } g = a.grant }";

const PRIO_SRC: &str = "module M { in clk : clock in req : bits<4> out g : bits<4> \
     let a = PriorityArbiter<4> { clk: clk, req: req } g = a.grant }";

const EDGE_SRC: &str = "module M { in clk : clock in sig : bool \
     out r : bool out f : bool out b : bool \
     let e = EdgeDetect { clk: clk, signal: sig } \
     r = e.rising f = e.falling b = e.both }";

// ═══ SyncFifo ═════════════════════════════════════════════════════

#[test]
fn sync_fifo_emits_mem_count_and_flags() {
    let out = sv(&sync_fifo_src("16"));
    assert!(out.contains("localparam int f_DEPTH = 16;"), "{out}");
    assert!(out.contains("f_mem ["), "bellek dizisi: {out}");
    assert!(out.contains("f_count"), "doluluk sayacı: {out}");
    assert!(out.contains("assign f_full = (f_count == 5'd16);"), "{out}");
    assert!(out.contains("assign f_empty = (f_count == 5'd0);"), "{out}");
}

#[test]
fn sync_fifo_field_access_becomes_prefixed_signal() {
    let out = sv(&sync_fifo_src("16"));
    assert!(out.contains("assign dout = f_rd_data;"), "{out}");
    assert!(out.contains("assign full = f_full;"), "{out}");
    assert!(!out.contains("f.rd_data"), "alan erişimi çevrilmeli: {out}");
}

#[test]
fn sync_fifo_depth_10_is_e2025() {
    let codes = emit_codes(&sync_fifo_src("10"));
    assert!(codes.contains(&"E2025"), "E2025 bekleniyor: {codes:?}");
}

#[test]
fn sync_fifo_has_no_gray_code_pointers() {
    // Tek saatli FIFO gray kod maliyeti taşımaz (ADR-0029 gerekçesi).
    let out = sv(&sync_fifo_src("16"));
    assert!(!out.contains("gray"), "gray kod olmamalı: {out}");
}

#[test]
fn sync_fifo_immediate_proves_full_empty_disjoint() {
    let out = sv_immediate(&sync_fifo_src("16"));
    assert!(out.contains("f_count <= 5'd16"), "doluluk sınırı: {out}");
    assert!(
        out.contains("!(f_full && f_empty)"),
        "bayrak ayrıklığı: {out}"
    );
    assert!(out.contains("// volt:f_inv_0"), "{out}");
    assert!(out.contains("// volt:f_inv_1"), "{out}");
    assert!(out.contains("// volt:f_cov_0"), "{out}");
    assert!(out.contains("// volt:f_cov_1"), "{out}");
}

// ═══ Ram / DualPortRam ════════════════════════════════════════════

#[test]
fn ram_emits_read_first_memory() {
    let out = sv(RAM_SRC);
    assert!(out.contains("localparam int m_DEPTH = 16;"), "{out}");
    assert!(out.contains("m_mem ["), "{out}");
    assert!(
        out.contains("m_rd_data <= m_mem[(addr)];"),
        "okuma-önce: {out}"
    );
    assert!(out.contains("m_mem[(addr)] <= (wd);"), "{out}");
}

#[test]
fn ram_immediate_contract_addr_in_range() {
    let out = sv_immediate(RAM_SRC);
    assert!(out.contains("(addr) < 5'd16"), "adres sınırı: {out}");
    assert!(out.contains("// volt:m_inv_0"), "{out}");
}

#[test]
fn dual_port_ram_uses_single_always_block() {
    // İki ayrı always_ff bloğu Verilator MULTIDRIVEN üretirdi.
    let out = sv(DPRAM_SRC);
    let count = out.matches("always_ff").count();
    assert_eq!(count, 1, "tek always_ff bekleniyor ({count}): {out}");
}

#[test]
fn dual_port_ram_port_b_writes_last() {
    // Aynı adrese eş zamanlı yazmada B kazanır (W3006 sözleşmesi):
    // B'nin yazması metinde A'nınkinden SONRA gelmeli.
    let out = sv(DPRAM_SRC);
    let a = out.find("m_mem[(aa)] <= (ad);").expect("A yazması");
    let b = out.find("m_mem[(ba)] <= (bd);").expect("B yazması");
    assert!(b > a, "B portu son yazmalı: {out}");
}

#[test]
fn dual_port_ram_immediate_has_two_addr_contracts() {
    let out = sv_immediate(DPRAM_SRC);
    assert!(out.contains("// volt:m_inv_0"), "{out}");
    assert!(out.contains("// volt:m_inv_1"), "{out}");
}

// ═══ Counter ══════════════════════════════════════════════════════

#[test]
fn counter_emits_wrap_and_overflow_pulse() {
    let out = sv(COUNTER_SRC);
    assert!(out.contains("cnt_count <= cnt_count + 8'd1;"), "{out}");
    assert!(
        out.contains("if (cnt_count == {8{1'b1}}) begin"),
        "sarma sezimi: {out}"
    );
    assert!(out.contains("cnt_overflow <= 1'b1;"), "{out}");
}

#[test]
fn counter_clear_wins_over_enable() {
    let out = sv(COUNTER_SRC);
    let clear = out.find("if ((clr)) begin").expect("clear dalı");
    let enable = out.find("else if ((en)) begin").expect("enable dalı");
    assert!(clear < enable, "clear önce gelmeli: {out}");
}

#[test]
fn counter_immediate_contracts() {
    let out = sv_immediate(COUNTER_SRC);
    assert!(out.contains("cnt_count < 9'd256"), "üst sınır: {out}");
    assert!(out.contains("// volt:cnt_inv_0"), "{out}");
    assert!(out.contains("// volt:cnt_cov_0"), "{out}");
}

// ═══ ShiftRegister ════════════════════════════════════════════════

#[test]
fn shift_register_emits_flat_vector_and_slices() {
    let out = sv(SHIFT_SRC);
    assert!(out.contains("logic [7:0] sr_shift;"), "{out}");
    assert!(
        out.contains("sr_shift <= {sr_shift[6:0], (d)};"),
        "kaydırma: {out}"
    );
    assert!(out.contains("assign sr_taps = sr_shift;"), "{out}");
    assert!(
        out.contains("assign sr_data_out = sr_shift[7:7];"),
        "en eski aşama: {out}"
    );
}

#[test]
fn shift_register_wide_data_scales_slices() {
    // T=u8, LEN=4: 32 bitlik vektör, veri dilimleri 8'er bit.
    let src = "module M { in clk : clock in d : u8 in s : bool out q : u8 out t : bits<32> \
         let sr = ShiftRegister<u8, 4> { clk: clk, data_in: d, shift_en: s } \
         q = sr.data_out t = sr.taps }";
    let out = sv(src);
    assert!(out.contains("logic [31:0] sr_shift;"), "{out}");
    assert!(out.contains("sr_shift <= {sr_shift[23:0], (d)};"), "{out}");
    assert!(
        out.contains("assign sr_data_out = sr_shift[31:24];"),
        "{out}"
    );
}

// ═══ Arbiterler ═══════════════════════════════════════════════════

#[test]
fn round_robin_emits_mask_and_lowest_bit_isolation() {
    let out = sv(RR_SRC);
    assert!(out.contains("a_mask"), "dönen maske: {out}");
    assert!(out.contains("a_masked & (~a_masked + 4'd1)"), "{out}");
    assert!(out.contains("a_mask <= ~((a_grant << 1) - 4'd1);"), "{out}");
}

#[test]
fn round_robin_immediate_contracts_and_per_bit_covers() {
    let out = sv_immediate(RR_SRC);
    assert!(
        out.contains("(a_grant & (a_grant - 4'd1)) == 4'd0"),
        "bir-sıcak: {out}"
    );
    assert!(
        out.contains("(a_grant & a_req_v) == a_grant"),
        "alt küme: {out}"
    );
    for k in 0..4 {
        assert!(
            out.contains(&format!("// volt:a_cov_{k}")),
            "cover {k}: {out}"
        );
    }
}

#[test]
fn priority_arbiter_is_stateless_combinational() {
    let out = sv(PRIO_SRC);
    assert!(out.contains("a_req_v & (~a_req_v + 4'd1)"), "{out}");
    assert!(
        !out.contains("always_ff"),
        "PriorityArbiter durumsuz olmalı: {out}"
    );
}

// ═══ EdgeDetect ═══════════════════════════════════════════════════

#[test]
fn edge_detect_emits_prev_register_and_edges() {
    let out = sv(EDGE_SRC);
    assert!(out.contains("e_prev <= (sig);"), "{out}");
    assert!(out.contains("assign e_rising = (sig) & ~e_prev;"), "{out}");
    assert!(out.contains("assign e_falling = ~(sig) & e_prev;"), "{out}");
    assert!(out.contains("assign e_both = (sig) ^ e_prev;"), "{out}");
}

#[test]
fn edge_detect_immediate_proves_both_identity() {
    let out = sv_immediate(EDGE_SRC);
    assert!(
        out.contains("e_both == (e_rising | e_falling)"),
        "kimlik: {out}"
    );
    assert!(out.contains("// volt:e_cov_0"), "{out}");
    assert!(out.contains("// volt:e_cov_1"), "{out}");
}

// ═══ Ortak güvenceler ═════════════════════════════════════════════

#[test]
fn normal_build_has_no_formal_artifacts() {
    for src in [
        sync_fifo_src("16").as_str(),
        RAM_SRC,
        DPRAM_SRC,
        COUNTER_SRC,
        SHIFT_SRC,
        RR_SRC,
        PRIO_SRC,
        EDGE_SRC,
    ] {
        let out = sv(src);
        assert!(
            !out.contains("formal"),
            "normal build formal içermez: {out}"
        );
        assert!(!out.contains("assume"), "{out}");
        assert!(!out.contains("initial"), "{out}");
    }
}

#[test]
fn generated_sv_has_no_turkish_characters() {
    // Üretilen kod dili İngilizcedir (ADR-0026); Türkçe karakter sızması
    // araç zincirini (Verilator/Yosys) bozar. (Başlıktaki tipografik
    // tire ASCII dışıdır ama Türkçe değildir — kapsam dışı.)
    const TURKISH: &[char] = &['ç', 'Ç', 'ğ', 'Ğ', 'ı', 'İ', 'ö', 'Ö', 'ş', 'Ş', 'ü', 'Ü'];
    for src in [
        sync_fifo_src("16").as_str(),
        RAM_SRC,
        DPRAM_SRC,
        COUNTER_SRC,
        SHIFT_SRC,
        RR_SRC,
        PRIO_SRC,
        EDGE_SRC,
    ] {
        for gen in [sv(src), sv_immediate(src)] {
            assert!(
                !gen.contains(TURKISH),
                "üretilen SV Türkçe karakter içermemeli: {gen}"
            );
        }
    }
}
