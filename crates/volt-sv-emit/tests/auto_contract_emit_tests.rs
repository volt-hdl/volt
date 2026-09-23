//! Otomatik FSM / sayaç kontratlarının SV üretimi — ADR-0066.
//!
//! Kontratlar YALNIZ SVA'ya ve simülasyon izleyicisine gider: RTL bir
//! bayt değişmez. Her üretilen property'nin yorumu kuralı, kontrat
//! metnini ve kökenini ("generated from") taşır.

use volt_span::FileId;
use volt_sv_emit::{emit_full, EmitOutput, SvaMode};

fn emit(src: &str, mode: SvaMode) -> EmitOutput {
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.error_codes());
    let out = emit_full(&parsed.ast, "div.volt", src, mode);
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
    out
}

const DIV: &str = "\
module Div {
    in  clk   : clock
    in  go    : bool
    out pulse : bool

    reg tick_r : u4 = 0
    reg st_r   : u2 = 0

    on clk {
        if tick_r == 9 {
            tick_r <= 0
        } else {
            tick_r <= tick_r + 1
        }
        match st_r {
            0 => { if go { st_r <= 1 } }
            _ => { st_r <= 0 }
        }
    }
    pulse = tick_r == 9 && st_r == 1
}
";

#[test]
fn generated_rtl_is_identical_with_and_without_auto_contracts() {
    let with = emit(DIV, SvaMode::Separate);
    let without = emit(&format!("@no_auto_contracts\n{DIV}"), SvaMode::Separate);
    assert_eq!(with.sv, without.sv);
    assert!(with.sva_files.len() == 1 && without.sva_files.is_empty());
}

#[test]
fn sva_comment_names_rule_text_and_origin() {
    let out = emit(DIV, SvaMode::Separate);
    let sva = &out.sva_files[0].content;
    assert!(
        sva.contains(
            "// invariant (auto counter bound: tick_r <= 9) generated from div.volt:10 (wrap check on tick_r)"
        ),
        "{sva}"
    );
    assert!(
        sva.contains(
            "// cover (auto FSM transition: prev(st_r) == 0 && st_r == 1) generated from div.volt:16 (match on st_r, transition 0 -> 1)"
        ),
        "{sva}"
    );
    assert!(sva.contains("tick_r <= 4'd9;"), "{sva}");
}

#[test]
fn sva_props_carry_the_origin_for_reports() {
    let out = emit(DIV, SvaMode::Immediate);
    // Register bildirim sırası: tick_r (sayaç), sonra st_r (FSM).
    let rules: Vec<(&str, &str, &str)> = out
        .sva_props
        .iter()
        .map(|p| {
            let a = p.auto.as_ref().expect("hepsi otomatik");
            (p.name.as_str(), a.rule, a.text.as_str())
        })
        .collect();
    assert_eq!(
        rules,
        [
            ("inv_0", "counter bound", "tick_r <= 9"),
            ("cov_0", "counter wrap", "tick_r == 9"),
            ("cov_1", "FSM transition", "prev(st_r) == 0 && st_r == 1"),
            ("cov_2", "FSM transition", "prev(st_r) != 0 && st_r == 0"),
        ]
    );
}

#[test]
fn simulation_monitor_checks_auto_contracts_with_dpi_ids() {
    let out = emit(DIV, SvaMode::Simulation);
    assert!(
        out.sv.contains("volt_contract_fail(\"Div.inv_0\")"),
        "{}",
        out.sv
    );
    assert!(
        out.sv
            .contains("// invariant (auto counter bound: tick_r <= 9) generated from div.volt:10"),
        "{}",
        out.sv
    );
}

#[test]
fn user_contracts_keep_their_names_auto_ones_follow() {
    // Kullanıcı kontratları önce numaralanır: mevcut property adları
    // (verify raporları, sby görev eşlemesi) otomatik eklemeyle kaymaz.
    let src = DIV.replace("out pulse : bool\n", "out pulse : bool\n    cover: go\n");
    let out = emit(&src, SvaMode::Immediate);
    let first = &out.sva_props[0];
    assert_eq!(first.name, "cov_0");
    assert!(first.auto.is_none());
    assert!(out.sva_props[1..].iter().all(|p| p.auto.is_some()));
}

#[test]
fn mmio_contract_comment_points_at_the_user_attribute_not_the_synthetic_text() {
    // @mmio kontratı sentetik kaynaktan ayrıştırılır; "generated from"
    // kullanıcının `@mmio` satırını gösterir (ADR-0066 §4).
    let src = "@mmio(base = 0x4000_0000, bus = AXI4Lite)
module Regs {
    in  clk : clock
    out en  : bool

    @reg(offset = 0x00, access = ReadWrite)
    control : {
        enable : bool,
        @reserved : bits<31>,
    }

    en = regs.control.enable
}
";
    let out = emit(src, SvaMode::Separate);
    let sva = &out.sva_files[0].content;
    assert!(
        sva.contains("(auto @mmio register map: ")
            && sva.contains("generated from div.volt:1 (@mmio register map of Regs)"),
        "{sva}"
    );
    assert!(!sva.contains("from <mmio:"), "{sva}");
}
