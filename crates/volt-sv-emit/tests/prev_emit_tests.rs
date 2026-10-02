//! `prev()` SV üretimi — ADR-0040: Inline/Separate modda `$past`,
//! Immediate (verify) modda yardımcı register zinciri.

use volt_span::FileId;
use volt_sv_emit::{emit_full, SvaMode};

fn sv(src: &str, mode: SvaMode) -> String {
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse hatasız olmalı: {:?}",
        parsed.error_codes()
    );
    let out = emit_full(&parsed.ast, "test.volt", src, mode);
    assert!(
        !out.diagnostics
            .iter()
            .any(|d| d.code.as_str().starts_with('E')),
        "emit hatasız olmalı: {:?}",
        out.diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>()
    );
    out.sv
}

const FOLLOWER: &str = "module F {\n    in  clk : clock\n    in  start : bool\n    in  data : u8\n    out busy : bool\n    reg busy_r : bool = false\n    on clk { busy_r <= start }\n    busy = busy_r\n";

#[test]
fn inline_mode_emits_past() {
    let out = sv(
        &format!("{FOLLOWER}    invariant: busy == prev(start)\n}}"),
        SvaMode::Inline,
    );
    assert!(out.contains("busy == $past(start)"), "{out}");
    assert!(!out.contains("past_start_1"), "{out}");
}

#[test]
fn inline_mode_emits_past_with_depth() {
    let out = sv(
        &format!("{FOLLOWER}    invariant: busy == prev(start, 3)\n}}"),
        SvaMode::Inline,
    );
    assert!(out.contains("$past(start, 3)"), "{out}");
}

#[test]
fn separate_mode_sva_file_uses_past_and_binds_signal() {
    let src = format!("{FOLLOWER}    invariant: busy == prev(start)\n}}");
    let parsed = volt_syntax::parser::parse(FileId(0), &src);
    let out = emit_full(&parsed.ast, "test.volt", &src, SvaMode::Separate);
    let file = out.sva_files.first().expect("sva dosyası");
    assert!(file.content.contains("$past(start)"), "{}", file.content);
    assert!(
        file.content.contains("input logic start") || file.content.contains("start,"),
        "{}",
        file.content
    );
    assert!(
        !file.content.contains("input") || !file.content.contains(" prev"),
        "prev bir port olmamalı: {}",
        file.content
    );
}

#[test]
fn immediate_mode_emits_helper_register_instead_of_past() {
    let out = sv(
        &format!("{FOLLOWER}    invariant: busy == prev(start)\n}}"),
        SvaMode::Immediate,
    );
    assert!(!out.contains("$past"), "{out}");
    assert!(out.contains("logic past_start_1;"), "{out}");
    assert!(out.contains("past_start_1 <= start;"), "{out}");
    assert!(out.contains("assert (busy == past_start_1);"), "{out}");
}

#[test]
fn immediate_helper_register_resets_to_zero() {
    let out = sv(
        &format!("{FOLLOWER}    invariant: busy == prev(start)\n}}"),
        SvaMode::Immediate,
    );
    assert!(out.contains("if (rst) begin"), "{out}");
    assert!(out.contains("past_start_1 <= '0;"), "{out}");
}

#[test]
fn immediate_depth_builds_a_shift_chain() {
    let out = sv(
        &format!("{FOLLOWER}    invariant: busy == prev(start, 3)\n}}"),
        SvaMode::Immediate,
    );
    assert!(out.contains("logic past_start_1;"), "{out}");
    assert!(out.contains("logic past_start_2;"), "{out}");
    assert!(out.contains("logic past_start_3;"), "{out}");
    assert!(out.contains("past_start_2 <= past_start_1;"), "{out}");
    assert!(out.contains("past_start_3 <= past_start_2;"), "{out}");
    assert!(out.contains("assert (busy == past_start_3);"), "{out}");
}

#[test]
fn immediate_shares_one_chain_for_repeated_signal() {
    let src = format!(
        "{FOLLOWER}    invariant: busy == prev(start)\n    cover: prev(start, 2) && !prev(start)\n}}"
    );
    let out = sv(&src, SvaMode::Immediate);
    assert_eq!(out.matches("logic past_start_1;").count(), 1, "{out}");
    assert_eq!(out.matches("logic past_start_2;").count(), 1, "{out}");
    assert!(
        out.contains("cover (past_start_2 && !past_start_1);"),
        "{out}"
    );
}

#[test]
fn immediate_helper_register_has_signal_width() {
    let out = sv(
        &format!("{FOLLOWER}    invariant: prev(data) == data || busy\n}}"),
        SvaMode::Immediate,
    );
    assert!(out.contains("logic [7:0] past_data_1;"), "{out}");
}

#[test]
fn immediate_compound_argument_gets_anonymous_register() {
    let out = sv(
        &format!("{FOLLOWER}    invariant: prev(start && busy) -> busy\n}}"),
        SvaMode::Immediate,
    );
    assert!(out.contains("logic past_e1_1;"), "{out}");
    assert!(out.contains("past_e1_1 <= start && busy;"), "{out}");
}

#[test]
fn immediate_nested_prev_chains_through_inner_register() {
    let out = sv(
        &format!("{FOLLOWER}    invariant: prev(prev(start)) == prev(start, 2)\n}}"),
        SvaMode::Immediate,
    );
    assert!(out.contains("logic past_e1_1;"), "{out}");
    assert!(out.contains("past_e1_1 <= past_start_1;"), "{out}");
}

#[test]
fn ui_pass_49_emits_both_flavours() {
    let src = include_str!("../../../tests/ui/pass/49_prev_contract.volt");
    let inline = sv(src, SvaMode::Inline);
    assert!(inline.contains("$past(start, 2)"), "{inline}");
    let imm = sv(src, SvaMode::Immediate);
    assert!(imm.contains("past_start_2"), "{imm}");
    assert!(!imm.contains("$past"), "{imm}");
}

#[test]
fn rtl_only_mode_has_no_helper_registers() {
    let out = sv(
        &format!("{FOLLOWER}    invariant: busy == prev(start)\n}}"),
        SvaMode::None,
    );
    assert!(!out.contains("past_start"), "{out}");
    assert!(!out.contains("$past"), "{out}");
}

// ═══ ADR-0098 eki: toplayıcılar her ifade biçimine iner ═══════════

/// Yalnız parça seçimi ve dizi literali içinde okunan sinyaller.
const SELECTS: &str = "module S {\n    in  clk : clock\n    in  a : u8\n    in  b : u8\n    \
                       out y : u8\n    reg r : u8 = 0\n    reg r2 : [u8; 2] = [0; 2]\n    \
                       on clk {\n        r <= a\n        r2[0] <= a\n        r2[1] <= b\n    }\n    \
                       y = r\n";

/// Ayrı `.sva` dosyasında kontratın parça seçimiyle okuduğu sinyal checker
/// portu olmalı; yoksa üretilen dosya bildirilmemiş adı okur (Verilator:
/// "Can't find definition of variable").
#[test]
fn separate_sva_checker_ports_include_part_select_and_array_literal_signals() {
    let src = format!(
        "{SELECTS}    invariant: r[0 +: 4] == r[0 +: 4]\n    \
         invariant: r2 == [prev(a), prev(b)]\n}}"
    );
    let parsed = volt_syntax::parser::parse(FileId(0), &src);
    let out = emit_full(&parsed.ast, "test.volt", &src, SvaMode::Separate);
    let file = out.sva_files.first().expect("sva dosyası");
    let ports: Vec<&str> = file
        .content
        .lines()
        .filter(|l| l.trim_start().starts_with("input "))
        .collect();
    for name in ["r", "r2", "a", "b"] {
        assert!(
            ports.iter().any(|l| l
                .trim_end_matches(',')
                .split_whitespace()
                .any(|w| w == name)),
            "'{name}' checker portu olmalı:\n{}",
            file.content
        );
    }
}

/// Dizi literali içindeki `prev()` Immediate/Simulation kipinde de
/// yardımcı register zincirine iner. Önceden toplanmıyordu ve `$past`
/// yedeğine düşüyordu: reset sonrası ilk çevrimde belgelenen `prev(x) == 0`
/// yerine reset öncesi değer denetleniyordu.
#[test]
fn prev_inside_an_array_literal_uses_the_helper_chain() {
    for mode in [SvaMode::Immediate, SvaMode::Simulation] {
        let out = sv(
            &format!("{SELECTS}    invariant: r2 == [prev(a), prev(b, 2)]\n}}"),
            mode,
        );
        assert!(!out.contains("$past"), "{mode:?}: {out}");
        assert!(out.contains("past_a_1 <= a;"), "{mode:?}: {out}");
        assert!(out.contains("past_b_2 <= past_b_1;"), "{mode:?}: {out}");
        assert!(out.contains("'{past_a_1, past_b_2}"), "{mode:?}: {out}");
    }
}
