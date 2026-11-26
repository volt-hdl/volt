//! `prev()` SV üretimi — ADR-0040: Inline/Separate modda `$past`,
//! Immediate (verify) modda yardımcı register zinciri.

mod typed;

use volt_span::FileId;
use volt_sv_emit::SvaMode;

fn sv(src: &str, mode: SvaMode) -> String {
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse hatasız olmalı: {:?}",
        parsed.error_codes()
    );
    let out = typed::emit_full(&parsed.ast, "test.volt", src, mode);
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
    let out = typed::emit_full(&parsed.ast, "test.volt", &src, SvaMode::Separate);
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
    let out = typed::emit_full(&parsed.ast, "test.volt", &src, SvaMode::Separate);
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

/// `prev()` çağrılarının yardımcı register bildirimleri ve tanılar.
fn helper_decls(out: &volt_sv_emit::EmitOutput) -> (Vec<String>, Vec<&'static str>) {
    let decls = out
        .sv
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("logic") && l.contains(" past_"))
        .map(str::to_string)
        .collect();
    let codes = out.diagnostics.iter().map(|d| d.code.as_str()).collect();
    (decls, codes)
}

/// Tip denetimi argümana genişlik vermiyorsa (boyutsuz literal) yardımcı
/// register bildirilmez ve E2005 argümanı gösterir (#413). Önceden üretici
/// kendi genişliğini bulamayınca register'ı 1 bit bildiriyordu
/// (`unwrap_or(1)`, `logic past_e1_1;`).
#[test]
fn a_prev_argument_without_a_width_is_e2005_not_one_bit() {
    let src = format!("{FOLLOWER}    invariant: prev(5) == data || busy\n}}");
    let parsed = volt_syntax::parser::parse(FileId(0), &src);
    for mode in [SvaMode::Immediate, SvaMode::Simulation] {
        let out = typed::emit_full(&parsed.ast, "test.volt", &src, mode);
        let (decls, codes) = helper_decls(&out);
        assert!(decls.is_empty(), "{mode:?}: {decls:?}");
        assert_eq!(codes, ["E2005"], "{mode:?}");
        let d = &out.diagnostics[0];
        assert!(
            d.message.to_string().contains("prev()"),
            "{mode:?}: {}",
            d.message
        );
        let span = d.primary_span().expect("konum").span;
        assert_eq!(&src[span.start as usize..span.end as usize], "5");
    }
}

/// Tip denetiminin tablosu olmadan (`emit_full`) üretici `prev()`'in
/// genişliğini kendisi hesaplamaz: E2005, register yok (#413, tek kaynak).
#[test]
fn without_the_type_checker_prev_has_no_width() {
    let src = format!("{FOLLOWER}    invariant: prev(data) == data || busy\n}}");
    let parsed = volt_syntax::parser::parse(FileId(0), &src);
    let out = volt_sv_emit::emit_full(&parsed.ast, "test.volt", &src, SvaMode::Immediate);
    let (decls, codes) = helper_decls(&out);
    assert!(decls.is_empty(), "{decls:?}");
    assert_eq!(codes, ["E2005"]);
}

/// Yardımcı register tip denetiminin argümana verdiği genişlikte ve
/// işarettedir (#413, ADR-0040: `prev(x: T) -> T`); bildirim biçimi
/// sv-mapping.md §15'inki (`logic [W-1:0] past_x_1;`), işaretli tipte
/// `logic signed`. Elle: u8 + u8 doğal 9 bit (ADR-0025), i8, i8 + i8 doğal
/// 9 bit işaretli, Trit 2 bit işaretli (ADR-0062).
#[test]
fn helper_registers_have_the_width_and_sign_of_the_argument() {
    let src = "module S {
    in  clk : clock
    in  a   : u8
    in  b   : u8
    in  sa  : i8
    in  sb  : i8
    in  t   : Trit
    out y   : u8
    reg r : u8 = 0
    on clk { r <= a }
    y = r
    invariant: prev(a + b) > 255
    invariant: prev(sa) < sb
    invariant: prev(sa + sb, 2) < 0
    invariant: prev(t) < t
}";
    for mode in [SvaMode::Immediate, SvaMode::Simulation] {
        let out = sv(src, mode);
        for decl in [
            "logic [8:0] past_e1_1;",
            "logic signed [7:0] past_sa_1;",
            "logic signed [8:0] past_e2_2;",
            "logic signed [1:0] past_t_1;",
        ] {
            assert!(out.contains(decl), "{mode:?}: `{decl}` yok:\n{out}");
        }
        // Karşılaştırma `prev()`'in genişliğinde: 255 dokuz bitte.
        assert!(out.contains("past_e1_1 > 9'd255"), "{mode:?}: {out}");
        // Kaynak register'ın genişliğinde, taşmasıyla yazılır.
        assert!(!out.contains("past_e1_1 <= a + b;"), "{mode:?}: {out}");
    }
}

/// `--emit=sva`: `$past`'in argümanı öz-belirlenimlidir; `prev(a + b)`
/// doğal genişlikte (9 bit) hesaplanır ve karşılaştırma o genişliktedir
/// (#413'ün örneği: önceden `$past(a + b) != 8'h2C`).
#[test]
fn sva_past_of_a_sum_keeps_its_carry() {
    let src = "module Q {
    in  clk : clock
    in  a   : u8
    in  b   : u8
    out y   : u8
    reg r : u8 = 0
    on clk { r <= a }
    y = r
    invariant: prev(a + b) != 300
}";
    let out = sv(src, SvaMode::Inline);
    assert!(!out.contains("8'h2C"), "{out}");
    assert!(out.contains("9'd300"), "{out}");
    assert!(!out.contains("$past(a + b)"), "{out}");
}

/// Generic parametre derinliği (`prev(x, D)`, `G<3>`) monomorflamada
/// literal olur; zincir üç halkadır, 1'e düşmez (#413'ün `prev_depth`
/// incelemesi). Sabit olmayan öteki derinlikler isim çözümlemede E5017.
#[test]
fn a_generic_depth_builds_the_whole_chain() {
    let src = "module G<const D: u32> {
    in  clk : clock
    in  x   : u8
    out y   : u8
    reg r : u8 = 0
    on clk { r <= x }
    y = r
    invariant: prev(x, D) == r
}
module Top {
    in  clk : clock
    in  x   : u8
    out y   : u8
    let g = G<3> { clk: clk, x: x }
    y = g.y
}";
    let out = sv(src, SvaMode::Simulation);
    for k in 1..=3 {
        assert!(out.contains(&format!("logic [7:0] past_x_{k};")), "{out}");
    }
    assert!(!out.contains("past_x_4"), "{out}");
    assert!(out.contains("past_x_3 == r"), "{out}");
}
