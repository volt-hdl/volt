//! Üretilen SV adlarının çakışması (ADR-0090): emitter'ın kurduğu ad bir
//! kullanıcı adıyla ya da başka bir kurulan adla aynıysa E1003 (iki
//! kaynak, iki konum); yalnız formal/sim çıktısında yaşayan yardımcılar
//! (`past_*`, `volt_hits_*`) çakışırsa `_2`, `_3`… alır. Çakışma yoksa
//! adlar değişmez.

use std::collections::HashMap;

use volt_diagnostics::{Diagnostic, ErrorCode};
use volt_span::FileId;
use volt_sv_emit::{emit_full, EmitOutput, SvaMode};

fn emit(src: &str, mode: SvaMode) -> EmitOutput {
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse hatasız olmalı: {:?}",
        parsed.error_codes()
    );
    emit_full(&parsed.ast, "test.volt", src, mode)
}

fn errors(out: &EmitOutput) -> Vec<&Diagnostic> {
    out.diagnostics
        .iter()
        .filter(|d| d.code.as_str().starts_with('E'))
        .collect()
}

/// Modül düzeyinde (4 boşluk girinti) her `logic`/`longint` adı bir kez.
fn assert_unique_decls(sv: &str) {
    let mut seen: HashMap<&str, usize> = HashMap::new();
    for line in sv.lines() {
        let Some(rest) = line.strip_prefix("    ") else {
            continue;
        };
        if let Some(decl) = rest
            .strip_prefix("logic ")
            .or_else(|| rest.strip_prefix("longint "))
        {
            let name = decl
                .split(['=', ';'])
                .next()
                .unwrap_or_default()
                .split_whitespace()
                .last()
                .unwrap_or_default();
            *seen.entry(name).or_insert(0) += 1;
        }
    }
    let dups: Vec<_> = seen.iter().filter(|(_, n)| **n > 1).collect();
    assert!(dups.is_empty(), "çift bildirim {dups:?}\n{sv}");
}

const PAST_CLASH: &str = "\
module F {
    in  clk   : clock
    in  start : bool
    out busy  : bool
    invariant: busy == prev(start)
    cover: prev(start, 2)
    reg past_start_1 : bool = false
    on clk { past_start_1 <= start }
    busy = past_start_1
}
";

#[test]
fn prev_chain_base_avoids_user_register_in_verify_mode() {
    let out = emit(PAST_CLASH, SvaMode::Immediate);
    assert!(errors(&out).is_empty(), "{:?}", out.diagnostics);
    assert_unique_decls(&out.sv);
    assert!(out.sv.contains("logic past_start_2_1;"), "{}", out.sv);
    assert!(out.sv.contains("logic past_start_2_2;"), "{}", out.sv);
    assert!(out.sv.contains("busy == past_start_2_1"), "{}", out.sv);
}

#[test]
fn prev_chain_base_avoids_user_register_in_test_mode() {
    let out = emit(PAST_CLASH, SvaMode::Simulation);
    assert!(errors(&out).is_empty(), "{:?}", out.diagnostics);
    assert_unique_decls(&out.sv);
    assert!(out.sv.contains("past_start_2_1"), "{}", out.sv);
}

#[test]
fn prev_chain_keeps_its_name_without_a_clash() {
    let src = PAST_CLASH.replace("past_start_1", "busy_r");
    let out = emit(&src, SvaMode::Immediate);
    assert!(out.sv.contains("logic past_start_1;"), "{}", out.sv);
    assert!(out.sv.contains("logic past_start_2;"), "{}", out.sv);
    assert!(!out.sv.contains("past_start_2_1"), "{}", out.sv);
}

/// `prev(x)` ile `prev(x_2)`: ikinci zincirin tabanı `past_x_2`, halkası
/// `past_x_2_1` — birinci zincirin tabanı değişmemeli, adlar ayrı kalmalı.
#[test]
fn two_chains_with_suffix_like_sources_stay_distinct() {
    let src = "\
module F {
    in  clk : clock
    in  x   : bool
    in  x_2 : bool
    out y   : bool
    invariant: y == prev(x) || y == prev(x_2, 2)
    reg r : bool = false
    on clk { r <= x }
    y = r
}
";
    let out = emit(src, SvaMode::Immediate);
    assert!(errors(&out).is_empty(), "{:?}", out.diagnostics);
    assert_unique_decls(&out.sv);
    assert!(out.sv.contains("logic past_x_1;"), "{}", out.sv);
    assert!(out.sv.contains("logic past_x_2_2;"), "{}", out.sv);
}

const HITS_CLASH: &str = "\
module C {
    in  clk : clock
    in  x   : bool
    out o   : bool
    cover: x
    reg volt_hits_cov_0 : bool = false
    on clk { volt_hits_cov_0 <= x }
    o = volt_hits_cov_0
}
";

#[test]
fn cover_counter_avoids_user_register_in_test_mode() {
    let out = emit(HITS_CLASH, SvaMode::Simulation);
    assert!(errors(&out).is_empty(), "{:?}", out.diagnostics);
    assert_unique_decls(&out.sv);
    assert!(
        out.sv.contains("longint volt_hits_cov_0_2 = 0;"),
        "{}",
        out.sv
    );
    // Rapor kimlikle yapılır: sayaç adı değişse de kimlik aynı.
    assert!(
        out.sv.contains("(\"C.cov_0\", volt_hits_cov_0_2)"),
        "{}",
        out.sv
    );
}

#[test]
fn cover_counter_keeps_its_name_without_a_clash() {
    let src = HITS_CLASH
        .replace("reg volt_hits_cov_0", "reg seen")
        .replace(
            "volt_hits_cov_0 <= x }\n    o = volt_hits_cov_0",
            "seen <= x }\n    o = seen",
        );
    let out = emit(&src, SvaMode::Simulation);
    assert!(
        out.sv.contains("longint volt_hits_cov_0 = 0;"),
        "{}",
        out.sv
    );
}

const TIMER: &str = "\
module Timer {
    in  clk : clock
    out irq : bool
    reg c : u4 = 0
    on clk { c <= c + 1 }
    irq = c == 0
}
";

#[test]
fn instance_output_clash_names_both_sources_with_two_labels() {
    let src = format!(
        "{TIMER}module Soc {{\n    in  clk : clock\n    out timer_irq : bool\n    out o : bool\n    \
         let timer = Timer {{ clk }}\n    timer_irq = true\n    o = timer.irq\n}}\n"
    );
    let out = emit(&src, SvaMode::None);
    let errs = errors(&out);
    assert_eq!(errs.len(), 1, "{:?}", out.diagnostics);
    let d = errs[0];
    assert_eq!(d.code, ErrorCode::E1003);
    assert!(
        d.message
            .contains("'timer_irq' is both port 'timer_irq' and output 'irq' of instance 'timer'"),
        "{}",
        d.message
    );
    let primary = d.primary_span().expect("birincil etiket");
    let secondary: Vec<_> = d.spans.iter().filter(|s| !s.primary).collect();
    assert_eq!(secondary.len(), 1, "iki konum");
    // Birincil: adı kuran örnek; ikincil: kullanıcının portu (önce yazılmış).
    assert!(secondary[0].span.start < primary.span.start);
}

#[test]
fn collision_free_design_has_no_new_diagnostic() {
    let src = format!(
        "{TIMER}module Soc {{\n    in  clk : clock\n    out timer_flag : bool\n    \
         let timer = Timer {{ clk }}\n    timer_flag = timer.irq\n}}\n"
    );
    let out = emit(&src, SvaMode::None);
    assert!(errors(&out).is_empty(), "{:?}", out.diagnostics);
    assert!(out.sv.contains("logic timer_irq;"), "{}", out.sv);
}

/// Yerleşik primitifin izleyici sayacı gövde üretilirken yazılır;
/// kaynakta SONRA gelen kullanıcı `let`'inden de kaçınmalı.
#[test]
fn builtin_cover_counter_avoids_a_later_user_let() {
    let src = "\
domain Fast {
    clock = posedge
    reset = sync active_high
}
domain Slow {
    clock = posedge
    reset = sync active_high
}
module B {
    in  wclk : clock @Fast
    in  rclk : clock @Slow
    in  din  : u8 @Fast
    in  push : bool @Fast
    in  pop  : bool @Slow
    out dout : u8 @Slow
    out seen : bool @Slow
    let f = AsyncFifo<u8, 4> { wr_clk: wclk, wr_data: din, wr_en: push, rd_clk: rclk, rd_en: pop }
    dout = f.rd_data
    let volt_hits_f_cov_1 = f.rd_empty
    seen = volt_hits_f_cov_1
}
";
    let out = emit(src, SvaMode::Simulation);
    assert!(errors(&out).is_empty(), "{:?}", out.diagnostics);
    assert_unique_decls(&out.sv);
    assert!(
        out.sv.contains("longint volt_hits_f_cov_1_2 = 0;"),
        "{}",
        out.sv
    );
    assert!(
        out.sv.contains("longint volt_hits_f_cov_0 = 0;"),
        "{}",
        out.sv
    );
}

/// Gövdede ÜRETİLEN adlar da (örnek çıkışı `past_start_1`) `prev()`
/// zincirinin kaçındığı adlardır.
#[test]
fn prev_chain_avoids_a_generated_instance_output() {
    let src = "\
module P {
    in  clk     : clock
    out start_1 : bool
    start_1 = true
}
module F {
    in  clk   : clock
    in  start : bool
    out busy  : bool
    invariant: busy == prev(start)
    let past = P { clk }
    busy = past.start_1
}
";
    let out = emit(src, SvaMode::Immediate);
    assert!(errors(&out).is_empty(), "{:?}", out.diagnostics);
    assert_unique_decls(&out.sv);
    assert!(
        out.sv.contains("logic past_start_1;"),
        "örnek çıkışı: {}",
        out.sv
    );
    assert!(out.sv.contains("logic past_start_2_1;"), "{}", out.sv);
}
