//! volt-sw-emit birim testleri (ADR-0053): dört üreticinin saf çıktısı.
//! Harita parser'dan alınır (`ParseResult.regmaps`) — üreticiler AST
//! görmez, yalnız `RegMap` okur.

use std::path::Path;

use volt_ast::mmio::RegMap;
use volt_span::FileId;
use volt_sw_emit::{emit_c, emit_json, emit_markdown, emit_rust, file_stem, EmitOpts, SwKind};
use volt_syntax::parser::parse;

const TIMER: &str = r#"
/// Timer block.
/// Counts ticks | compares.
@mmio(base = 0x0000_0100, bus = AXI4Lite)
module TimerRegs {
    in  clk    : clock
    in  tick   : bool
    out irq    : bool

    /// Control word.
    @reg(offset = 0x00, access = ReadWrite)
    ctrl : {
        /// Run enable.
        enable : bool,
        /// Restart pulse.
        clear  : bool @self_clearing,
        mode   : u3,
        @reserved : bits<27>,
    }

    /// Hard-wired identifier.
    @reg(offset = 0x04, access = ReadOnly)
    id : { value : u16, @reserved : bits<16> }

    @reg(offset = 0x08, access = WriteOnly)
    compare : { value : u16, @reserved : bits<16> }

    /// Live status.
    @reg(offset = 0x0C, access = ReadOnly, volatile)
    status : {
        count   : u16,
        /// Counter reached compare.
        expired : bool @w1c,
        @reserved : bits<15>,
    }

    @reg(offset = 0x10, access = ReadWrite)
    wide : { all : bits<32> }

    on clk {
        if regs.ctrl.clear {
            regs.status.count <= 0
        } else if regs.ctrl.enable && tick {
            regs.status.count <= regs.status.count + 1
        }
        if regs.status.count == regs.compare.value { regs.status.expired <= true }
    }
    irq = regs.status.expired
}
"#;

fn timer() -> RegMap {
    let r = parse(FileId(0), TIMER);
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    r.regmaps.into_iter().next().expect("harita")
}

fn opts() -> EmitOpts {
    EmitOpts {
        source: "timer.volt".to_string(),
        version: "0.1.0".to_string(),
    }
}

// ═══ Adlandırma ve yollar ═══════════════════════════════════════════

#[test]
fn file_stem_is_snake_case_of_module_name() {
    assert_eq!(file_stem("Gpio"), "gpio");
    assert_eq!(file_stem("GpioRegs"), "gpio_regs");
    assert_eq!(file_stem("UART2Ctrl"), "uart2_ctrl");
    assert_eq!(file_stem("AXI4LiteSlave"), "axi4_lite_slave");
    assert_eq!(file_stem("already_snake"), "already_snake");
}

#[test]
fn output_paths_follow_cli_contract_layout() {
    let map = timer();
    let dir = Path::new("build");
    assert_eq!(
        SwKind::Rust.output_path(dir, &map),
        Path::new("build/sw/timer_regs.rs")
    );
    assert_eq!(
        SwKind::C.output_path(dir, &map),
        Path::new("build/sw/timer_regs.h")
    );
    assert_eq!(
        SwKind::Json.output_path(dir, &map),
        Path::new("build/sw/timer_regs.json")
    );
    assert_eq!(
        SwKind::Markdown.output_path(dir, &map),
        Path::new("build/docs/timer_regs.md")
    );
}

#[test]
fn sw_kind_flags_match_emit_values() {
    assert_eq!(SwKind::Rust.flag(), "rust");
    assert_eq!(SwKind::C.flag(), "c");
    assert_eq!(SwKind::Json.flag(), "regmap");
    assert_eq!(SwKind::Markdown.flag(), "regmap-md");
    let map = timer();
    assert_eq!(SwKind::Rust.render(&map, &opts()), emit_rust(&map, &opts()));
    assert_eq!(SwKind::C.render(&map, &opts()), emit_c(&map, &opts()));
    assert_eq!(SwKind::Json.render(&map, &opts()), emit_json(&map, &opts()));
    assert_eq!(
        SwKind::Markdown.render(&map, &opts()),
        emit_markdown(&map, &opts())
    );
}

// ═══ Rust ══════════════════════════════════════════════════════════

#[test]
fn rust_driver_is_no_std_and_uses_volatile_access() {
    let rs = emit_rust(&timer(), &opts());
    assert!(rs.contains("use core::ptr::{read_volatile, write_volatile};"));
    assert!(!rs.contains("std::"), "no_std: std kullanılmamalı\n{rs}");
    assert!(rs.contains("read_volatile(self.base.byte_add(offset))"));
    assert!(rs.contains("write_volatile(self.base.byte_add(offset), value)"));
    assert!(rs.contains("#[derive(Debug)]\npub struct TimerRegs {\n    base: *mut u32,\n}"));
    assert!(rs.contains("pub const BASE: usize = 0x0000_0100;"));
    assert!(rs.contains("/// # Safety"));
    assert!(rs.contains("pub const unsafe fn new(base: *mut u32) -> Self"));
}

#[test]
fn rust_readwrite_field_gets_getter_and_setter() {
    let rs = emit_rust(&timer(), &opts());
    assert!(rs.contains("pub fn ctrl_enable(&self) -> bool"));
    assert!(rs.contains("pub fn set_ctrl_enable(&mut self, enable: bool)"));
    assert!(rs.contains("pub fn ctrl_mode(&self) -> u8"));
    assert!(rs.contains("pub fn set_ctrl_mode(&mut self, mode: u8)"));
    // mode bits 4:2 → shift 2, mask 0x7
    assert!(rs.contains("((self.read(Self::CTRL_OFFSET) >> 2) & 0x7) as u8"));
    assert!(rs.contains("(u32::from(mode) << 2) & 0x0000_001C"));
}

#[test]
fn rust_single_field_register_collapses_to_register_name() {
    let rs = emit_rust(&timer(), &opts());
    assert!(rs.contains("pub fn id(&self) -> u16"), "id.value → id()");
    assert!(rs.contains("pub fn set_compare(&mut self, value: u16)"));
    assert!(!rs.contains("fn id_value("));
}

#[test]
fn rust_readonly_register_has_no_setter() {
    let rs = emit_rust(&timer(), &opts());
    assert!(
        !rs.contains("fn set_id"),
        "ReadOnly (sabit) register'a setter yok\n{rs}"
    );
    assert!(
        !rs.contains("fn set_status"),
        "ReadOnly volatile register'a setter yok"
    );
    assert!(rs.contains("pub fn id_raw(&self) -> u32"));
    assert!(!rs.contains("fn set_id_raw"));
    assert!(rs.contains("pub fn status_count(&self) -> u16"));
    assert!(rs.contains("pub fn status_expired(&self) -> bool"));
}

#[test]
fn rust_writeonly_register_has_no_getter() {
    let rs = emit_rust(&timer(), &opts());
    assert!(
        !rs.contains("fn compare(&self)"),
        "WriteOnly register'a getter yok"
    );
    assert!(!rs.contains("fn compare_raw"));
    assert!(rs.contains("pub fn set_compare_raw(&mut self, word: u32)"));
    assert!(rs.contains("// WriteOnly register: the other fields are written as 0."));
}

#[test]
fn rust_self_clearing_field_gets_trigger_and_w1c_gets_clear() {
    let rs = emit_rust(&timer(), &opts());
    assert!(rs.contains("pub fn trigger_ctrl_clear(&mut self)"));
    assert!(
        !rs.contains("fn set_ctrl_clear"),
        "darbe alanına düz setter yok"
    );
    assert!(rs.contains("pub fn clear_status_expired(&mut self)"));
    assert!(rs.contains("self.write(Self::STATUS_OFFSET, 0x0001_0000)"));
    // Başka bir alanın RMW'si darbe bitini korumaz (yeniden tetiklemesin):
    // keep = enable|mode (0x1D) eksi enable = 0x1C.
    assert!(rs.contains("let word = self.read(Self::CTRL_OFFSET) & 0x0000_001C;"));
}

#[test]
fn rust_reserved_bits_are_masked_in_constants_and_raw_access() {
    let rs = emit_rust(&timer(), &opts());
    assert!(rs.contains("pub const CTRL_MASK: u32 = 0x0000_001F;"));
    assert!(rs.contains("pub const STATUS_MASK: u32 = 0x0001_FFFF;"));
    assert!(rs.contains("pub const WIDE_MASK: u32 = 0xFFFF_FFFF;"));
    assert!(rs.contains("self.read(Self::CTRL_OFFSET) & Self::CTRL_MASK"));
    assert!(rs.contains("self.write(Self::CTRL_OFFSET, word & Self::CTRL_MASK)"));
    // 32 bitlik alan: maske ve kaydırma yok, tip u32.
    assert!(rs.contains("pub fn wide(&self) -> u32"));
    assert!(rs.contains("pub fn set_wide(&mut self, all: u32)"));
}

#[test]
fn rust_driver_carries_volt_doc_comments() {
    let rs = emit_rust(&timer(), &opts());
    assert!(rs.contains("/// Timer block.\n/// Counts ticks | compares.\n#[derive(Debug)]"));
    assert!(rs.contains("    /// Control word.\n    /// Raw `ctrl` word"));
    assert!(rs.contains(
        "    /// Run enable.\n    /// `ctrl.enable` (bit 0).\n    #[must_use]\n    pub fn ctrl_enable"
    ));
    assert!(rs.contains("    /// Restart pulse.\n    /// Pulse `ctrl.clear` (bit 1)"));
    assert!(rs.contains("//! Do not edit: regenerate with `volt build --emit=rust timer.volt`."));
}

#[test]
fn rust_field_types_follow_width() {
    let src = r#"
@mmio(base = 0, bus = AXI4Lite)
module W {
    in clk : clock
    out o : bool
    @reg(offset = 0x00, access = ReadWrite)
    r : { a : u8, b : u9, c : bits<15> }
    o = regs.r.a == 0
}
"#;
    let r = parse(FileId(0), src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    let rs = emit_rust(&r.regmaps[0], &opts());
    assert!(rs.contains("pub fn r_a(&self) -> u8"));
    assert!(rs.contains("pub fn r_b(&self) -> u16"), "9 bit → u16");
    assert!(rs.contains("pub fn r_c(&self) -> u16"), "15 bit → u16");
    assert!(rs.contains("((self.read(Self::R_OFFSET) >> 17) & 0x7FFF) as u16"));
}

// ═══ C ═════════════════════════════════════════════════════════════

#[test]
fn c_header_has_guard_includes_and_base() {
    let h = emit_c(&timer(), &opts());
    assert!(h.starts_with("/* `TimerRegs` register map"));
    assert!(h.contains("#ifndef TIMER_REGS_H\n#define TIMER_REGS_H\n"));
    assert!(h.trim_end().ends_with("#endif /* TIMER_REGS_H */"));
    assert!(h.contains("#include <stdint.h>\n#include <stdbool.h>"));
    assert!(h.contains("#define TIMER_REGS_BASE 0x00000100U"));
    assert!(h.contains("extern \"C\" {"));
}

#[test]
fn c_header_defines_address_shift_and_mask_per_field() {
    let h = emit_c(&timer(), &opts());
    assert!(h.contains("#define TIMER_REGS_CTRL (TIMER_REGS_BASE + 0x00U)"));
    assert!(h.contains("#define TIMER_REGS_CTRL_OFFSET 0x00U"));
    assert!(h.contains("#define TIMER_REGS_CTRL_MASK 0x0000001FU"));
    assert!(h.contains("#define TIMER_REGS_CTRL_MODE_SHIFT 2U"));
    assert!(h.contains("#define TIMER_REGS_CTRL_MODE_MASK 0x7U"));
    assert!(h.contains("#define TIMER_REGS_STATUS_EXPIRED_SHIFT 16U"));
    assert!(h.contains("#define TIMER_REGS_WIDE_ALL_MASK 0xFFFFFFFFU"));
}

#[test]
fn c_getters_use_volatile_reads_and_narrow_types() {
    let h = emit_c(&timer(), &opts());
    assert!(h.contains("static inline uint8_t timer_regs_get_ctrl_mode(void) {"));
    assert!(h.contains(
        "return (uint8_t)((*(volatile uint32_t *)TIMER_REGS_CTRL >> TIMER_REGS_CTRL_MODE_SHIFT) & TIMER_REGS_CTRL_MODE_MASK);"
    ));
    assert!(h.contains("static inline bool timer_regs_get_ctrl_enable(void) {"));
    assert!(h.contains("static inline uint16_t timer_regs_get_status_count(void) {"));
    assert!(h.contains("static inline uint32_t timer_regs_get_wide_all(void) {"));
}

#[test]
fn c_readonly_register_has_no_setter_and_writeonly_no_getter() {
    let h = emit_c(&timer(), &opts());
    assert!(
        !h.contains("timer_regs_set_id_value"),
        "ReadOnly setter yok\n{h}"
    );
    assert!(!h.contains("timer_regs_id_write"));
    assert!(!h.contains("timer_regs_set_status_count"));
    assert!(!h.contains("timer_regs_status_write"));
    assert!(h.contains("timer_regs_get_id_value"));
    assert!(
        !h.contains("timer_regs_get_compare_value"),
        "WriteOnly getter yok"
    );
    assert!(!h.contains("timer_regs_compare_read"));
    assert!(h.contains("static inline void timer_regs_set_compare_value(uint16_t value) {"));
}

#[test]
fn c_trigger_and_clear_helpers_for_pulse_fields() {
    let h = emit_c(&timer(), &opts());
    assert!(h.contains("static inline void timer_regs_trigger_ctrl_clear(void) {"));
    assert!(!h.contains("timer_regs_set_ctrl_clear"));
    assert!(h.contains("static inline void timer_regs_clear_status_expired(void) {\n    *(volatile uint32_t *)TIMER_REGS_STATUS = 0x00010000U;\n}"));
    assert!(h.contains("uint32_t word = *(volatile uint32_t *)TIMER_REGS_CTRL & 0x0000001DU;"));
}

#[test]
fn c_header_carries_doc_comments() {
    let h = emit_c(&timer(), &opts());
    assert!(h.contains(" * Timer block.\n * Counts ticks | compares.\n */"));
    assert!(h.contains("/* Control word. */"));
    assert!(h.contains("/* Run enable. */\nstatic inline bool timer_regs_get_ctrl_enable"));
}

// ═══ JSON ══════════════════════════════════════════════════════════

#[test]
fn json_follows_volt_regmap_1_schema() {
    let text = emit_json(&timer(), &opts());
    let v: serde_json::Value = serde_json::from_str(&text).expect("geçerli JSON");
    assert_eq!(v["schema"], "volt-regmap/1");
    assert_eq!(v["generator"], "volt 0.1.0");
    assert_eq!(v["source"], "timer.volt");
    assert_eq!(v["name"], "TimerRegs");
    assert_eq!(v["base"], 0x100);
    assert_eq!(v["bus"], "AXI4Lite");
    assert_eq!(v["doc"], "Timer block.\nCounts ticks | compares.");
    let regs = v["registers"].as_array().expect("registers dizi");
    assert_eq!(regs.len(), 5);
    let ctrl = &regs[0];
    assert_eq!(ctrl["name"], "ctrl");
    assert_eq!(ctrl["offset"], 0);
    assert_eq!(ctrl["address"], 0x100);
    assert_eq!(ctrl["access"], "rw");
    assert_eq!(ctrl["volatile"], false);
    assert_eq!(ctrl["doc"], "Control word.");
    let fields = ctrl["fields"].as_array().unwrap();
    assert_eq!(fields.len(), 4);
    assert_eq!(fields[1]["name"], "clear");
    assert_eq!(fields[1]["lsb"], 1);
    assert_eq!(fields[1]["width"], 1);
    assert_eq!(fields[1]["type"], "bool");
    assert_eq!(fields[1]["self_clearing"], true);
    assert_eq!(fields[1]["w1c"], false);
    assert_eq!(fields[1]["reserved"], false);
    assert_eq!(fields[2]["type"], "uint");
    assert_eq!(fields[2]["doc"], serde_json::Value::Null);
    assert_eq!(fields[3]["name"], "_reserved");
    assert_eq!(fields[3]["reserved"], true);
    assert_eq!(fields[3]["type"], "bits");
    assert_eq!(fields[3]["lsb"], 5);
    assert_eq!(fields[3]["width"], 27);
}

#[test]
fn json_access_and_volatile_per_register() {
    let text = emit_json(&timer(), &opts());
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    let by = |n: &str| {
        v["registers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["name"] == n)
            .unwrap()
            .clone()
    };
    assert_eq!(by("id")["access"], "ro");
    assert_eq!(by("id")["volatile"], false);
    assert_eq!(by("compare")["access"], "wo");
    assert_eq!(by("status")["access"], "ro");
    assert_eq!(by("status")["volatile"], true);
    assert_eq!(by("status")["fields"][1]["w1c"], true);
    assert_eq!(by("status")["address"], 0x10C);
}

// ═══ Markdown ══════════════════════════════════════════════════════

#[test]
fn markdown_has_summary_table_with_offset_name_access_description() {
    let md = emit_markdown(&timer(), &opts());
    assert!(
        md.starts_with("# `TimerRegs` register map\n\nTimer block.\nCounts ticks | compares.\n")
    );
    assert!(md.contains("- Base address: `0x0000_0100`"));
    assert!(md.contains("| Offset | Name | Access | Description |\n|---|---|---|---|\n"));
    assert!(md.contains("| `0x00` | `ctrl` | RW | Control word. |"));
    assert!(md.contains("| `0x04` | `id` | RO | Hard-wired identifier. |"));
    assert!(md.contains("| `0x08` | `compare` | WO |  |"));
    assert!(md.contains("| `0x0C` | `status` | RO, volatile | Live status. |"));
}

#[test]
fn markdown_bit_field_tables_list_high_to_low_with_attributes() {
    let md = emit_markdown(&timer(), &opts());
    assert!(
        md.contains("### `ctrl` — offset `0x00` (address `0x0000_0100`), RW\n\nControl word.\n")
    );
    let ctrl = md.split("### `ctrl`").nth(1).unwrap();
    let ctrl = ctrl.split("### `id`").next().unwrap();
    let rows: Vec<&str> = ctrl
        .lines()
        .filter(|l| l.starts_with("| ") && !l.starts_with("| Bits"))
        .collect();
    assert_eq!(
        rows,
        [
            "| 31:5 | — | `bits<27>` | reserved | Reads as 0; write 0. |",
            "| 4:2 | `mode` | `u3` | — |  |",
            "| 1 | `clear` | `bool` | self-clearing | Restart pulse. |",
            "| 0 | `enable` | `bool` | — | Run enable. |",
        ]
    );
    assert!(md.contains("| 16 | `expired` | `bool` | w1c | Counter reached compare. |"));
}

#[test]
fn markdown_escapes_pipes_in_descriptions() {
    let src = r#"
@mmio(base = 0, bus = AXI4Lite)
module P {
    in clk : clock
    out o : bool
    /// a | b
    @reg(offset = 0x00, access = ReadWrite)
    r : { x : bool, @reserved : bits<31> }
    o = regs.r.x
}
"#;
    let r = parse(FileId(0), src);
    let md = emit_markdown(&r.regmaps[0], &opts());
    assert!(md.contains("| `0x00` | `r` | RW | a \\| b |"));
}
