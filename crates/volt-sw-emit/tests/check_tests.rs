//! Register haritası tutarlılık denetimi (ADR-0063) — Seviye 2'nin
//! kütüphane yarısı. Her test üretilmiş bir dosyayı MUTASYONA uğratır ve
//! `check_file`'ın ayrışmayı doğru türde yakaladığını (ya da yorum/biçim
//! değişikliğini ayrışma SAYMADIĞINI) doğrular. Üç biçim: `.h`, `.rs`,
//! `.json`.

use volt_ast::mmio::RegMap;
use volt_span::FileId;
use volt_sw_emit::check::{
    check_file, parse, regmap_hash, view_hash, view_of, Cause, DriftKind, Format, Report,
    Unsupported,
};
use volt_sw_emit::{EmitOpts, SwKind};
use volt_syntax::parser::parse as parse_volt;

const TIMER: &str = r#"
/// Timer block.
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

const WIDE_DECL: &str =
    "    @reg(offset = 0x10, access = ReadWrite)\n    wide : { all : bits<32> }\n";

fn map_of(src: &str) -> RegMap {
    let r = parse_volt(FileId(0), src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    r.regmaps.into_iter().next().expect("harita")
}

fn timer() -> RegMap {
    map_of(TIMER)
}

/// `wide` register'ı olmayan eski tasarım (bayat dosya senaryosu).
fn timer_without_wide() -> RegMap {
    map_of(&TIMER.replace(WIDE_DECL, ""))
}

fn opts() -> EmitOpts {
    EmitOpts {
        source: "timer.volt".to_string(),
        version: "0.1.0".to_string(),
    }
}

fn render(format: Format, map: &RegMap) -> String {
    format.kind().render(map, &opts())
}

fn check(format: Format, text: &str) -> Report {
    check_file(&[timer()], format, text, &opts()).expect("Volt üretimi okunmalı")
}

/// Metinde `from` tam bir kez geçmeli; `to` ile değiştirilir.
fn mutate(text: &str, from: &str, to: &str) -> String {
    assert_eq!(
        text.matches(from).count(),
        1,
        "mutasyon kalıbı tek olmalı: {from:?}"
    );
    text.replacen(from, to, 1)
}

/// (tür, konu, dosya, RTL) — karşılaştırmayı okunur kılar.
fn drift(r: &Report) -> Vec<(DriftKind, String, Option<String>, Option<String>)> {
    r.drift
        .iter()
        .map(|d| (d.kind, d.subject.clone(), d.file.clone(), d.rtl.clone()))
        .collect()
}

fn only(r: &Report, kind: DriftKind, subject: &str) {
    assert_eq!(r.drift.len(), 1, "tek ayrışma beklenirdi: {:#?}", drift(r));
    assert_eq!(r.drift[0].kind, kind, "{:#?}", drift(r));
    assert_eq!(r.drift[0].subject, subject, "{:#?}", drift(r));
}

fn some(s: &str) -> Option<String> {
    Some(s.to_string())
}

// ═══ Temiz dosya: hızlı yol ═════════════════════════════════════════

#[test]
fn pristine_files_match_in_all_three_formats_via_fast_path() {
    for format in [Format::C, Format::Rust, Format::Json] {
        let r = check(format, &render(format, &timer()));
        assert!(r.is_match(), "{format:?}: {:#?}", drift(&r));
        assert!(r.fast_path, "{format:?}: görünüm hash'i eşit olmalı");
        assert!(
            r.code_compared,
            "{format:?}: aynı sürüm → kod karşılaştırılır"
        );
        assert_eq!(r.design_hash, r.content_hash);
        assert_eq!(r.parsed.header.hash, r.design_hash);
        assert_eq!(r.parsed.header.version, "0.1.0");
        assert_eq!(r.parsed.header.source, "timer.volt");
        assert_eq!(r.cause, None);
    }
}

#[test]
fn parsed_view_equals_design_view_in_all_three_formats() {
    let want = view_of(&timer());
    for format in [Format::C, Format::Rust, Format::Json] {
        let parsed = parse(format, &render(format, &timer())).expect("okunmalı");
        assert_eq!(parsed.view, want, "{format:?}");
        assert!(parsed.inconsistencies.is_empty(), "{format:?}");
    }
}

// ═══ Hash ═══════════════════════════════════════════════════════════

#[test]
fn regmap_hash_is_16_hex_and_ignores_docs_and_order() {
    let h = regmap_hash(&timer());
    assert_eq!(h.len(), 16);
    assert!(h
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    let undocumented = map_of(&TIMER.replace("/// Control word.\n", ""));
    assert_eq!(regmap_hash(&undocumented), h, "doc yorumu hash'e girmez");
    let mut view = view_of(&timer());
    view.registers.reverse();
    assert_eq!(view_hash(&view), h, "bildirim sırası hash'e girmez");
}

#[test]
fn regmap_hash_changes_with_offset_access_or_field() {
    let h = regmap_hash(&timer());
    let shifted = map_of(&TIMER.replace("offset = 0x10", "offset = 0x14"));
    let access = map_of(&TIMER.replace(
        "@reg(offset = 0x08, access = WriteOnly)",
        "@reg(offset = 0x08, access = ReadWrite)",
    ));
    let field = map_of(&TIMER.replace(
        "mode   : u3,\n        @reserved : bits<27>",
        "mode   : u4,\n        @reserved : bits<26>",
    ));
    for (name, m) in [("offset", shifted), ("access", access), ("field", field)] {
        assert_ne!(regmap_hash(&m), h, "{name} hash'i değiştirmeli");
    }
}

// ═══ C başlığı (.h) mutasyonları ═══════════════════════════════════

fn h() -> String {
    render(Format::C, &timer())
}

#[test]
fn c_offset_change_is_caught() {
    let text = mutate(
        &mutate(
            &h(),
            "(TIMER_REGS_BASE + 0x0CU)",
            "(TIMER_REGS_BASE + 0x14U)",
        ),
        "TIMER_REGS_STATUS_OFFSET 0x0CU",
        "TIMER_REGS_STATUS_OFFSET 0x14U",
    );
    let r = check(Format::C, &text);
    only(&r, DriftKind::Offset, "TIMER_REGS_STATUS");
    assert_eq!(r.drift[0].file, some("0x14"));
    assert_eq!(r.drift[0].rtl, some("0x0C"));
    assert_eq!(
        r.cause,
        Some(Cause::Edited),
        "hash satırı bugünkü tasarımın"
    );
}

#[test]
fn c_address_macro_edit_alone_is_caught_as_offset_and_inconsistency() {
    // Erişimciler adres makrosunu kullanır — `_OFFSET` sabiti doğru kalsa
    // bile makro yanlışsa sürücü yanlış adrese yazar.
    let text = mutate(
        &h(),
        "(TIMER_REGS_BASE + 0x0CU)",
        "(TIMER_REGS_BASE + 0x14U)",
    );
    let kinds: Vec<DriftKind> = check(Format::C, &text)
        .drift
        .iter()
        .map(|d| d.kind)
        .collect();
    assert_eq!(kinds, [DriftKind::Inconsistent, DriftKind::Offset]);
}

#[test]
fn c_field_mask_change_is_caught() {
    let text = mutate(
        &h(),
        "TIMER_REGS_CTRL_MODE_MASK 0x7U",
        "TIMER_REGS_CTRL_MODE_MASK 0xFU",
    );
    let r = check(Format::C, &text);
    only(&r, DriftKind::FieldMask, "TIMER_REGS_CTRL.MODE");
    assert_eq!(r.drift[0].file, some("0xF (4 bits)"));
    assert_eq!(r.drift[0].rtl, some("0x7 (3 bits)"));
}

#[test]
fn c_register_mask_change_is_caught() {
    let text = mutate(
        &h(),
        "TIMER_REGS_CTRL_MASK 0x0000001FU",
        "TIMER_REGS_CTRL_MASK 0x0000003FU",
    );
    let r = check(Format::C, &text);
    only(&r, DriftKind::Mask, "TIMER_REGS_CTRL");
    assert_eq!(r.drift[0].rtl, some("0x0000001F"));
}

#[test]
fn c_access_change_is_caught() {
    // Bayat başlık: `id` eskiden ReadWrite idi, yazma erişimcisi duruyor.
    let text = mutate(
        &h(),
        "static inline uint32_t timer_regs_id_read(void) {",
        "static inline void timer_regs_id_write(uint32_t word) {\n    \
         *(volatile uint32_t *)TIMER_REGS_ID = word & TIMER_REGS_ID_MASK;\n}\n\
         static inline uint32_t timer_regs_id_read(void) {",
    );
    let r = check(Format::C, &text);
    only(&r, DriftKind::Access, "TIMER_REGS_ID");
    assert_eq!(r.drift[0].file, some("ReadWrite"));
    assert_eq!(r.drift[0].rtl, some("ReadOnly"));
}

#[test]
fn c_reset_change_is_caught() {
    let text = mutate(
        &h(),
        "TIMER_REGS_CTRL_RESET 0x00000000U",
        "TIMER_REGS_CTRL_RESET 0x00000004U",
    );
    let r = check(Format::C, &text);
    only(&r, DriftKind::Reset, "TIMER_REGS_CTRL");
    assert_eq!(r.drift[0].file, some("0x00000004"));
    assert_eq!(r.drift[0].rtl, some("0x00000000"));
}

#[test]
fn c_base_change_is_caught() {
    let text = mutate(
        &h(),
        "TIMER_REGS_BASE 0x00000100U",
        "TIMER_REGS_BASE 0x00000200U",
    );
    only(&check(Format::C, &text), DriftKind::Base, "TIMER_REGS_BASE");
}

#[test]
fn c_field_position_change_is_caught() {
    let text = mutate(
        &h(),
        "TIMER_REGS_CTRL_MODE_SHIFT 2U",
        "TIMER_REGS_CTRL_MODE_SHIFT 3U",
    );
    only(
        &check(Format::C, &text),
        DriftKind::FieldLsb,
        "TIMER_REGS_CTRL.MODE",
    );
}

#[test]
fn c_field_behavior_change_is_caught() {
    // Tetikleyici erişimci `clear_` oldu: self-clearing → W1C.
    let text = mutate(
        &h(),
        "timer_regs_trigger_ctrl_clear(void)",
        "timer_regs_clear_ctrl_clear(void)",
    );
    let r = check(Format::C, &text);
    only(&r, DriftKind::FieldAccess, "TIMER_REGS_CTRL.CLEAR");
    assert_eq!(r.drift[0].file, some("W1C"));
    assert_eq!(r.drift[0].rtl, some("self-clearing"));
}

/// `/* <reg> @ ...` bloğunun metni (bir sonraki register ya da C++ kapanışına kadar).
fn c_block<'a>(text: &'a str, reg: &str) -> &'a str {
    let start = text.find(&format!("/* {reg} @ ")).expect("blok başı");
    let rest = &text[start..];
    let end = rest[1..]
        .find("\n/* ")
        .map(|i| i + 2)
        .or_else(|| rest.find("#ifdef __cplusplus\n}"))
        .expect("blok sonu");
    &rest[..end]
}

#[test]
fn c_deleted_register_is_reported_missing() {
    let full = h();
    let text = full.replacen(c_block(&full, "wide"), "", 1);
    let r = check(Format::C, &text);
    only(&r, DriftKind::MissingRegister, "TIMER_REGS_WIDE");
    assert_eq!(r.drift[0].rtl, some("0x10"));
    assert_eq!(r.drift[0].file, None);
}

#[test]
fn c_added_register_is_reported_extra() {
    let full = h();
    let extra = c_block(&full, "wide")
        .replace("wide", "spare")
        .replace("WIDE", "SPARE")
        .replace("0x10U", "0x14U");
    let text = full.replacen(
        "#ifdef __cplusplus\n}",
        &format!("{extra}#ifdef __cplusplus\n}}"),
        1,
    );
    let r = check(Format::C, &text);
    only(&r, DriftKind::ExtraRegister, "TIMER_REGS_SPARE");
    assert_eq!(r.drift[0].file, some("0x14"));
    assert_eq!(r.drift[0].rtl, None);
}

#[test]
fn c_comment_only_changes_are_not_drift() {
    let text = mutate(
        &h(),
        "/* Control word. */",
        "/* Kontrol sözcüğü — Türkçe açıklama. */",
    );
    let text = mutate(
        &text,
        "/* Run enable. */\nstatic inline bool",
        "/* #define TIMER_REGS_CTRL_OFFSET 0x40U */\nstatic inline bool",
    );
    let text = format!("/* Copyright 2026 Firmware Team. SPDX-License-Identifier: MIT */\n// Vendored from the FPGA repository.\n{text}");
    let r = check(Format::C, &text);
    assert!(r.is_match(), "{:#?}", drift(&r));
    assert!(r.code_compared);
}

#[test]
fn c_whitespace_and_formatting_changes_are_not_drift() {
    let text = h()
        .replace('\n', "\r\n")
        .replace("    return", "\treturn")
        .replace(
            "#define TIMER_REGS_CTRL_MASK 0x0000001FU",
            "#define   TIMER_REGS_CTRL_MASK    0x0000001FU",
        );
    let text = text.replace("}\r\n", "}\r\n\r\n");
    let r = check(Format::C, &text);
    assert!(r.is_match(), "{:#?}", drift(&r));
}

#[test]
fn c_hand_edit_in_accessor_body_is_caught_by_code_comparison() {
    // Görünüm (sabitler, erişimci adları) aynı; setter'ın koruma maskesi değişti.
    let text = mutate(
        &h(),
        "uint32_t word = *(volatile uint32_t *)TIMER_REGS_CTRL & 0x0000001DU;",
        "uint32_t word = *(volatile uint32_t *)TIMER_REGS_CTRL & 0x0000001FU;",
    );
    let r = check(Format::C, &text);
    assert!(r.fast_path, "görünüm aynı kalmalı");
    only(&r, DriftKind::Code, "TIMER_REGS");
    assert!(
        r.drift[0].file.as_deref().unwrap().contains("0x0000001FU"),
        "{:#?}",
        drift(&r)
    );
    assert!(r.drift[0].rtl.as_deref().unwrap().contains("0x0000001DU"));
    assert_eq!(r.cause, Some(Cause::Edited));
}

#[test]
fn accessor_bodies_of_another_volt_version_are_not_compared() {
    // Bilinen sınır (ADR-0063): başka sürümün üretimi yalnız görünümle denetlenir.
    let text = mutate(
        &h(),
        "// Generated by Volt 0.1.0 from",
        "// Generated by Volt 0.0.9 from",
    );
    let text = mutate(&text, "& 0x0000001DU;", "& 0x0000001FU;");
    let r = check(Format::C, &text);
    assert!(r.is_match());
    assert!(!r.code_compared);
}

#[test]
fn stale_header_from_an_older_design_is_classified_stale() {
    let old = render(Format::C, &timer_without_wide());
    let r = check(Format::C, &old);
    only(&r, DriftKind::MissingRegister, "TIMER_REGS_WIDE");
    assert_eq!(r.cause, Some(Cause::Stale));
    assert_eq!(r.parsed.header.hash, r.content_hash);
    assert_ne!(r.content_hash, r.design_hash);
}

#[test]
fn file_newer_than_the_design_reports_extra_register() {
    let r = check_file(&[timer_without_wide()], Format::C, &h(), &opts()).unwrap();
    only(&r, DriftKind::ExtraRegister, "TIMER_REGS_WIDE");
}

#[test]
fn renamed_module_is_reported() {
    let text = h()
        .replace("TIMER_REGS_", "TMR_")
        .replace("timer_regs_", "tmr_");
    let r = check(Format::C, &text);
    assert_eq!(r.drift[0].kind, DriftKind::Module, "{:#?}", drift(&r));
    assert_eq!(r.drift[0].file, some("TMR"));
    assert_eq!(r.drift[0].rtl, some("TIMER_REGS"));
}

// ═══ Rust sürücüsü (.rs) mutasyonları ══════════════════════════════

fn rs() -> String {
    render(Format::Rust, &timer())
}

#[test]
fn rust_offset_change_is_caught() {
    let text = mutate(
        &rs(),
        "pub const STATUS_OFFSET: usize = 0x0C;",
        "pub const STATUS_OFFSET: usize = 0x10;",
    );
    let r = check(Format::Rust, &text);
    only(&r, DriftKind::Offset, "TIMER_REGS_STATUS");
    assert_eq!(r.drift[0].file, some("0x10"));
}

#[test]
fn rust_field_mask_change_is_caught() {
    let text = mutate(
        &rs(),
        "pub const CTRL_MODE_MASK: u32 = 0x0000_0007;",
        "pub const CTRL_MODE_MASK: u32 = 0x0000_0003;",
    );
    only(
        &check(Format::Rust, &text),
        DriftKind::FieldMask,
        "TIMER_REGS_CTRL.MODE",
    );
}

#[test]
fn rust_access_change_is_caught() {
    // Setter'ı silinmiş RW register → dosyaya göre ReadOnly.
    let text = mutate(
        &rs(),
        "pub fn set_ctrl_raw(&mut self, word: u32) {",
        "fn set_ctrl_raw(&mut self, word: u32) {",
    );
    let r = check(Format::Rust, &text);
    only(&r, DriftKind::Access, "TIMER_REGS_CTRL");
    assert_eq!(r.drift[0].file, some("ReadOnly"));
    assert_eq!(r.drift[0].rtl, some("ReadWrite"));
}

#[test]
fn rust_reset_change_is_caught() {
    let text = mutate(
        &rs(),
        "pub const WIDE_RESET: u32 = 0x0000_0000;",
        "pub const WIDE_RESET: u32 = 0xDEAD_BEEF;",
    );
    let r = check(Format::Rust, &text);
    only(&r, DriftKind::Reset, "TIMER_REGS_WIDE");
    assert_eq!(r.drift[0].file, some("0xDEADBEEF"));
}

#[test]
fn rust_deleted_and_added_registers_are_reported() {
    let old = render(Format::Rust, &timer_without_wide());
    only(
        &check(Format::Rust, &old),
        DriftKind::MissingRegister,
        "TIMER_REGS_WIDE",
    );
    let r = check_file(&[timer_without_wide()], Format::Rust, &rs(), &opts()).unwrap();
    only(&r, DriftKind::ExtraRegister, "TIMER_REGS_WIDE");
}

#[test]
fn rust_comment_and_whitespace_changes_are_not_drift() {
    let text = rs()
        .replace("/// Control word.", "/// Denetim sözcüğü (çeviri).")
        .replace("    pub const", "        pub const")
        .replace('\n', "\n\n");
    let text = format!("// SPDX-License-Identifier: Apache-2.0\n{text}");
    let r = check(Format::Rust, &text);
    assert!(r.is_match(), "{:#?}", drift(&r));
    assert!(r.code_compared);
}

#[test]
fn rust_hand_edit_in_getter_body_is_caught_by_code_comparison() {
    let text = mutate(
        &rs(),
        "((self.read(Self::CTRL_OFFSET) >> 2) & 0x7) as u8",
        "((self.read(Self::CTRL_OFFSET) >> 3) & 0x7) as u8",
    );
    let r = check(Format::Rust, &text);
    only(&r, DriftKind::Code, "TIMER_REGS");
}

// ═══ regmap.json mutasyonları ══════════════════════════════════════

fn json() -> serde_json::Value {
    serde_json::from_str(&render(Format::Json, &timer())).unwrap()
}

fn check_json(v: &serde_json::Value) -> Report {
    check(Format::Json, &serde_json::to_string_pretty(v).unwrap())
}

#[test]
fn json_offset_change_is_caught() {
    let mut v = json();
    v["registers"][3]["offset"] = serde_json::json!(0x14);
    v["registers"][3]["address"] = serde_json::json!(0x114);
    let r = check_json(&v);
    only(&r, DriftKind::Offset, "TIMER_REGS_STATUS");
}

#[test]
fn json_address_not_matching_offset_is_an_inconsistency() {
    let mut v = json();
    v["registers"][1]["address"] = serde_json::json!(0x140);
    only(&check_json(&v), DriftKind::Inconsistent, "TIMER_REGS_ID");
}

#[test]
fn json_width_change_is_caught_as_mask() {
    let mut v = json();
    v["registers"][0]["fields"][2]["width"] = serde_json::json!(4);
    let kinds: Vec<DriftKind> = check_json(&v).drift.iter().map(|d| d.kind).collect();
    assert_eq!(kinds, [DriftKind::Mask, DriftKind::FieldMask]);
}

#[test]
fn json_access_change_is_caught() {
    let mut v = json();
    v["registers"][2]["access"] = serde_json::json!("rw");
    let r = check_json(&v);
    only(&r, DriftKind::Access, "TIMER_REGS_COMPARE");
    assert_eq!(r.drift[0].rtl, some("WriteOnly"));
}

#[test]
fn json_reset_change_is_caught() {
    let mut v = json();
    v["registers"][0]["reset"] = serde_json::json!(1);
    only(&check_json(&v), DriftKind::Reset, "TIMER_REGS_CTRL");
}

#[test]
fn json_deleted_and_added_registers_are_reported() {
    let mut v = json();
    let wide = v["registers"].as_array_mut().unwrap().remove(4);
    only(
        &check_json(&v),
        DriftKind::MissingRegister,
        "TIMER_REGS_WIDE",
    );

    let mut v = json();
    let mut spare = wide;
    spare["name"] = serde_json::json!("spare");
    spare["offset"] = serde_json::json!(0x20);
    spare["address"] = serde_json::json!(0x120);
    v["registers"].as_array_mut().unwrap().push(spare);
    let r = check_json(&v);
    only(&r, DriftKind::ExtraRegister, "TIMER_REGS_SPARE");
    assert_eq!(r.drift[0].file, some("0x20"));
}

#[test]
fn json_doc_and_formatting_changes_are_not_drift() {
    let mut v = json();
    v["registers"][0]["doc"] = serde_json::json!("Başka bir açıklama");
    v["doc"] = serde_json::Value::Null;
    let compact = serde_json::to_string(&v).unwrap();
    let r = check(Format::Json, &compact);
    assert!(r.is_match(), "{:#?}", drift(&r));
    assert!(r.code_compared);
}

#[test]
fn json_hand_edit_outside_the_view_is_caught_by_value_comparison() {
    // `volatile` görünümde yok; aynı sürümün dosyasında değer karşılaştırması görür.
    let mut v = json();
    v["registers"][3]["volatile"] = serde_json::json!(false);
    let r = check_json(&v);
    only(&r, DriftKind::Code, "TIMER_REGS");
    assert!(r.drift[0].file.as_deref().unwrap().contains("volatile"));
}

// ═══ Volt dışı / desteklenmeyen dosyalar ════════════════════════════

#[test]
fn handwritten_header_is_rejected_explicitly() {
    let text = "#ifndef GPIO_H\n#define GPIO_H\n#define GPIO_BASE 0x40000000U\n#define GPIO_CTRL_OFFSET 0x00U\n#endif\n";
    assert_eq!(
        check_file(&[timer()], Format::C, text, &opts()).unwrap_err(),
        Unsupported::NotVolt
    );
    assert_eq!(
        parse(Format::Rust, "pub struct Gpio;\n").unwrap_err(),
        Unsupported::NotVolt
    );
    assert_eq!(
        parse(Format::Json, "{\"registers\": []}").unwrap_err(),
        Unsupported::NotVolt
    );
    assert_eq!(
        parse(Format::Json, "not json at all").unwrap_err(),
        Unsupported::NotVolt
    );
}

#[test]
fn pre_hash_volt_output_is_reported_as_old_volt() {
    let text: String = h().lines().skip(2).map(|l| format!("{l}\n")).collect();
    assert_eq!(parse(Format::C, &text).unwrap_err(), Unsupported::OldVolt);
    let mut v = json();
    v.as_object_mut().unwrap().remove("regmap_hash");
    assert_eq!(
        parse(Format::Json, &v.to_string()).unwrap_err(),
        Unsupported::OldVolt
    );
}

#[test]
fn signed_file_missing_its_base_is_malformed() {
    let text = mutate(&h(), "#define TIMER_REGS_BASE 0x00000100U\n", "");
    assert!(matches!(
        parse(Format::C, &text).unwrap_err(),
        Unsupported::Malformed(_)
    ));
}

#[test]
fn format_is_chosen_by_extension_only_for_driver_files() {
    use std::path::Path;
    assert_eq!(Format::from_path(Path::new("fw/gpio.h")), Some(Format::C));
    assert_eq!(Format::from_path(Path::new("gpio.rs")), Some(Format::Rust));
    assert_eq!(
        Format::from_path(Path::new("gpio.json")),
        Some(Format::Json)
    );
    assert_eq!(Format::from_path(Path::new("gpio.md")), None);
    assert_eq!(Format::from_path(Path::new("gpio.hpp")), None);
    assert_eq!(Format::of_kind(SwKind::Markdown), None);
}

// ═══ İnceleme bulguları (regresyon) ════════════════════════════════

/// `irq` + `status_rx` ile `irq_status` + `rx2` aynı `_SHIFT` önekini paylaşır.
const AMBIGUOUS: &str = r#"
@mmio(base = 0x0000_0000, bus = AXI4Lite)
module Irqs {
    in  clk : clock
    out o   : bool

    @reg(offset = 0x00, access = ReadWrite)
    irq : { status_rx : bool, @reserved : bits<31> }

    @reg(offset = 0x04, access = ReadWrite)
    irq_status : { tx : bool, rx2 : bool, @reserved : bits<30> }

    o = regs.irq.status_rx
}
"#;

#[test]
fn ambiguous_field_constant_names_resolve_against_the_design() {
    let map = map_of(AMBIGUOUS);
    for format in [Format::C, Format::Rust, Format::Json] {
        let text = format.kind().render(&map, &opts());
        let r = check_file(std::slice::from_ref(&map), format, &text, &opts()).unwrap();
        assert!(r.is_match(), "{format:?}: {:#?}", drift(&r));
    }
}

#[test]
fn nonstandard_address_macro_of_another_version_is_an_inconsistency() {
    let text = mutate(
        &h(),
        "// Generated by Volt 0.1.0 from",
        "// Generated by Volt 0.0.9 from",
    );
    for macro_text in ["(0x0000010CU)", "((TIMER_REGS_BASE) + 0x0CU)"] {
        let edited = mutate(&text, "(TIMER_REGS_BASE + 0x0CU)", macro_text);
        let r = check(Format::C, &edited);
        if macro_text.contains("BASE") {
            assert!(
                r.is_match(),
                "parantezli yazım aynı biçim: {:#?}",
                drift(&r)
            );
        } else {
            only(&r, DriftKind::Inconsistent, "TIMER_REGS_STATUS");
        }
    }
}

#[test]
fn c_formatter_line_breaks_and_spaced_directives_are_not_drift() {
    let text = h()
        .replace(
            "static inline uint32_t timer_regs_ctrl_read(void) {",
            "static inline uint32_t\ntimer_regs_ctrl_read(void)\n{",
        )
        .replace("#define TIMER_REGS_ID_MASK", "#  define TIMER_REGS_ID_MASK");
    let r = check(Format::C, &text);
    assert!(r.is_match(), "{:#?}", drift(&r));
}

#[test]
fn rust_joined_attributes_and_nested_comments_are_not_drift() {
    let text = rs()
        .replace("#[must_use]\n    pub fn", "#[must_use] pub fn")
        .replace(
            "use core::ptr",
            "/* outer /* inner */ still a comment */\nuse core::ptr",
        );
    let r = check(Format::Rust, &text);
    assert!(r.is_match(), "{:#?}", drift(&r));
}

#[test]
fn utf8_bom_is_formatting_not_drift() {
    for format in [Format::C, Format::Rust, Format::Json] {
        let text = format!("\u{feff}{}", render(format, &timer()));
        let r = check(format, &text);
        assert!(r.is_match(), "{format:?}: {:#?}", drift(&r));
        let (s, _) = r.parsed.header.span;
        assert!(text[s..].starts_with(&r.parsed.header.hash) || format != Format::Json);
    }
}

#[test]
fn out_of_range_values_are_malformed_or_inconsistent_not_truncated() {
    let mut v = json();
    v["base"] = serde_json::json!(u64::MAX);
    assert!(matches!(
        parse(Format::Json, &v.to_string()).unwrap_err(),
        Unsupported::Malformed(_)
    ));
    let mut v = json();
    v["registers"][0]["reset"] = serde_json::json!(1u64 << 32);
    assert!(matches!(
        parse(Format::Json, &v.to_string()).unwrap_err(),
        Unsupported::Malformed(_)
    ));
    let mut v = json();
    v["registers"][0]["fields"][0]["lsb"] = serde_json::json!(1u64 << 32);
    assert!(matches!(
        parse(Format::Json, &v.to_string()).unwrap_err(),
        Unsupported::Malformed(_)
    ));

    let text = mutate(
        &h(),
        "TIMER_REGS_CTRL_RESET 0x00000000U",
        "TIMER_REGS_CTRL_RESET 0x100000000U",
    );
    let r = check(Format::C, &text);
    assert!(
        r.drift.iter().any(|d| d.kind == DriftKind::Inconsistent),
        "{:#?}",
        drift(&r)
    );
}
