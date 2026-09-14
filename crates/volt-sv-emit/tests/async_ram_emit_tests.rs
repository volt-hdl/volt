//! AsyncDualPortRam SV üretimi (ADR-0049): iki saatte iki always_ff
//! bloğu, resetlenmeyen bellek dizisi (BRAM çıkarımı), senkronizatör
//! YOK (adresler kendi alanlarında kalır), Immediate modda saat başına
//! adres kontratı ve yazma cover'ı.

use volt_span::FileId;
use volt_sv_emit::{emit, EmitResult};

const CDC_DOMAINS: &str = "domain Fast { clock = posedge, reset = sync active_high }
                           domain Slow { clock = posedge, reset = sync active_high }
";

fn ram_src(depth: &str) -> String {
    format!(
        "{CDC_DOMAINS}module M {{ in fast_clk : clock @Fast in slow_clk : clock @Slow \
         in wa : bits<4> @Fast in wd : u8 @Fast in we : bool @Fast \
         in ra : bits<4> @Slow out rd : u8 @Slow \
         let m = AsyncDualPortRam<u8, {depth}> {{ wr_clk: fast_clk, wr_addr: wa, wr_data: wd, \
         wr_en: we, rd_clk: slow_clk, rd_addr: ra }} \
         rd = m.rd_data }}"
    )
}

fn compile(src: &str) -> EmitResult {
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse hatasız olmalı: {:?}",
        parsed.error_codes()
    );
    emit(&parsed.ast, "test.volt")
}

fn sv(src: &str) -> String {
    let result = compile(src);
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

fn emit_immediate(src: &str) -> volt_sv_emit::EmitOutput {
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse hatasız olmalı: {:?}",
        parsed.error_codes()
    );
    volt_sv_emit::emit_full(
        &parsed.ast,
        "test.volt",
        src,
        volt_sv_emit::SvaMode::Immediate,
    )
}

fn sv_immediate(src: &str) -> String {
    let result = emit_immediate(src);
    let errors: Vec<&str> = result
        .diagnostics
        .iter()
        .filter(|d| !d.code.is_warning())
        .map(|d| d.code.as_str())
        .collect();
    assert!(errors.is_empty(), "emit hatasız olmalı: {errors:?}");
    result.sv
}

// ═══ Gövde ════════════════════════════════════════════════════════

#[test]
fn emits_one_always_ff_per_clock() {
    let out = sv(&ram_src("16"));
    let count = out.matches("always_ff").count();
    assert_eq!(count, 2, "iki always_ff bekleniyor ({count}): {out}");
    assert!(
        out.contains("always_ff @(posedge fast_clk)"),
        "yazma bloğu wr_clk'ta: {out}"
    );
    assert!(
        out.contains("always_ff @(posedge slow_clk"),
        "okuma bloğu rd_clk'ta: {out}"
    );
}

#[test]
fn write_port_has_no_reset_branch() {
    // Bellek dizisi resetlenmez ve yazma bloğu reset dalı taşımaz:
    // BRAM çıkarımı için yazma portu saf `if (we) mem[wa] <= wd`.
    let out = sv(&ram_src("16"));
    let wr_start = out
        .find("always_ff @(posedge fast_clk)")
        .expect("yazma bloğu");
    let wr_block = &out[wr_start..out[wr_start..].find("\n    end").unwrap() + wr_start];
    assert!(
        !wr_block.contains("rst"),
        "yazma bloğunda reset yok: {wr_block}"
    );
    assert!(
        wr_block.contains("m_mem[(wa)] <= (wd);"),
        "yazma satırı: {wr_block}"
    );
}

#[test]
fn read_port_is_registered_on_rd_clk() {
    let out = sv(&ram_src("16"));
    assert!(
        out.contains("m_rd_data <= m_mem[(ra)];"),
        "senkron okuma: {out}"
    );
    assert!(out.contains("localparam int m_DEPTH = 16;"), "{out}");
    assert!(out.contains("m_mem [m_DEPTH];"), "{out}");
}

#[test]
fn memory_array_is_touched_only_by_write_and_read() {
    // Reset dalında bellek dizisine atama yok (BRAM'e eşlenebilirlik).
    let out = sv(&ram_src("16"));
    let refs = out.matches("m_mem[").count();
    assert_eq!(
        refs, 2,
        "bellek yalnız yazma + okuma satırında ({refs}): {out}"
    );
}

#[test]
fn no_pointer_synchronizers_are_generated() {
    // AsyncFifo'nun aksine adres senkronizasyonu GEREKMEZ: her adres
    // kendi alanında kalır, bellek dizisi kendisi CDC sınırıdır.
    let out = sv(&ram_src("16"));
    assert!(!out.contains("gray"), "gray pointer olmamalı: {out}");
    assert!(!out.contains("_s0"), "iki-flop senkron olmamalı: {out}");
    assert!(!out.contains("_s1"), "iki-flop senkron olmamalı: {out}");
}

#[test]
fn field_access_becomes_prefixed_signal() {
    let out = sv(&ram_src("16"));
    assert!(out.contains("assign rd = m_rd_data;"), "{out}");
    assert!(!out.contains("m.rd_data"), "alan erişimi çevrilmeli: {out}");
}

#[test]
fn normal_build_has_no_formal_artifacts() {
    let out = sv(&ram_src("16"));
    assert!(
        !out.contains("initial"),
        "formal init yalnız Immediate'te: {out}"
    );
    assert!(
        !out.contains("assert"),
        "kontrat yalnız Immediate'te: {out}"
    );
}

#[test]
fn module_with_two_clocks_is_marked_multiclock() {
    let out = emit_immediate(&ram_src("16"));
    assert!(
        out.multiclock_modules.iter().any(|m| m == "M"),
        "multiclock on bekleniyor: {:?}",
        out.multiclock_modules
    );
}

// ═══ Doğrulama ════════════════════════════════════════════════════

#[test]
fn depth_not_power_of_two_is_e2025() {
    let codes = emit_codes(&ram_src("12"));
    assert!(codes.contains(&"E2025"), "E2025 bekleniyor: {codes:?}");
}

#[test]
fn missing_rd_clk_is_rejected() {
    let src = format!(
        "{CDC_DOMAINS}module M {{ in fast_clk : clock @Fast in slow_clk : clock @Slow \
         in wa : bits<4> @Fast in wd : u8 @Fast in we : bool @Fast \
         in ra : bits<4> @Slow out rd : u8 @Slow \
         let m = AsyncDualPortRam<u8, 16> {{ wr_clk: fast_clk, wr_addr: wa, wr_data: wd, \
         wr_en: we, rd_addr: ra }} \
         rd = m.rd_data }}"
    );
    let result = compile(&src);
    assert!(result.has_errors(), "rd_clk eksik: hata bekleniyor");
}

// ═══ Kontratlar (Immediate) ═══════════════════════════════════════

#[test]
fn immediate_contracts_bound_each_address_on_its_own_clock() {
    let out = sv_immediate(&ram_src("16"));
    assert!(
        out.contains("always @(posedge fast_clk)\n        if (!(rst)) assert ((wa) < 5'd16); // volt:m_inv_0"),
        "wr_addr kontratı wr_clk'ta: {out}"
    );
    assert!(
        out.contains("always @(posedge slow_clk)\n        if (!(rst)) assert ((ra) < 5'd16); // volt:m_inv_1"),
        "rd_addr kontratı rd_clk'ta: {out}"
    );
}

#[test]
fn immediate_covers_a_write_while_reading() {
    // Okuma portu her çevrim okur; "eş zamanlı okuma ve yazma" cover'ı
    // yazma etkinliğinin erişilebilirliğidir.
    let out = sv_immediate(&ram_src("16"));
    assert!(
        out.contains("cover ((we)); // volt:m_cov_0"),
        "yazma cover'ı: {out}"
    );
}

#[test]
fn immediate_formal_init_resets_only_the_read_register() {
    let out = sv_immediate(&ram_src("16"));
    assert!(out.contains("m_rd_data = 8'd0;"), "formal init: {out}");
    let init_start = out.find("initial begin").expect("formal init");
    let init_block = &out[init_start..];
    assert!(
        !init_block.contains("m_mem"),
        "bellek init edilmez: {init_block}"
    );
}

#[test]
fn immediate_assumes_no_mid_trace_reset_on_both_clocks() {
    // Çift saatli primitif: clk2fflogic altında kısmi reset'i dışlayan
    // kenar bazlı varsayım iki saatte de üretilir (ADR-0027 kalıbı).
    let out = sv_immediate(&ram_src("16"));
    assert!(
        out.contains("always @(posedge fast_clk) assume (!(rst)); // formal: no mid-trace reset"),
        "{out}"
    );
    assert!(
        out.contains("always @(posedge slow_clk) assume (!(rst)); // formal: no mid-trace reset"),
        "{out}"
    );
}
