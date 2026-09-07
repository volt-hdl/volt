//! Kontrat denetimi testleri (F4a).
//!
//! ADIM 1 kuralları: her kontrat ifadesi Bool tipinde olmalı (E5004),
//! kapsam kuralları (requires/ensures → port, invariant → port +
//! register, cover/assert/assume → hepsi; ihlal E1001) ve domain
//! tutarlılığı (kontrat sinyalleri aynı alanda olmalı, E3001).

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

/// Tek saatli, kontrat testleri için tipik modül gövdesi.
fn uart_module(contracts: &str) -> String {
    format!(
        "module Uart {{\n    in  clk   : clock\n    in  speed : u8\n    \
         in  start : bool\n    out busy  : bool\n\n{contracts}\n    \
         reg busy_r : bool = false\n\n    on clk {{\n        \
         busy_r <= start\n    }}\n\n    busy = busy_r\n}}\n"
    )
}

// ═══ E5004 — kontrat ifadesi Bool olmalı ══════════════════════════

#[test]
fn requires_comparison_is_clean() {
    let result = check(&uart_module("    requires: speed <= 2\n"));
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn requires_arithmetic_is_e5004() {
    assert!(
        codes(&uart_module("    requires: speed + 1\n")).contains(&"E5004"),
        "aritmetik ifade Bool değil"
    );
}

#[test]
fn invariant_u8_port_is_e5004() {
    assert!(
        codes(&uart_module("    invariant: speed\n")).contains(&"E5004"),
        "u8 port Bool değil"
    );
}

#[test]
fn invariant_bool_port_is_clean() {
    let result = check(&uart_module("    invariant: start\n"));
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn cover_int_literal_is_e5004() {
    assert!(
        codes(&uart_module("    cover: 1\n")).contains(&"E5004"),
        "sayısal literal Bool değil"
    );
}

#[test]
fn invariant_logical_negation_is_clean() {
    let result = check(&uart_module("    invariant: !(start && busy)\n"));
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn ensures_implication_shape_is_clean() {
    // Volt'ta '->' yok; !a || b biçimi kullanılır.
    let result = check(&uart_module("    ensures: !start || busy\n"));
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn assert_contract_non_bool_is_e5004() {
    assert!(codes(&uart_module("    assert: speed * 2\n")).contains(&"E5004"));
}

#[test]
fn assume_contract_non_bool_is_e5004() {
    assert!(codes(&uart_module("    assume: speed\n")).contains(&"E5004"));
}

#[test]
fn assume_bool_expression_is_clean() {
    let result = check(&uart_module("    assume: speed == 0\n"));
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn e5004_reported_once_per_contract() {
    let found = codes(&uart_module("    requires: speed\n    ensures: speed\n"));
    assert_eq!(
        found.iter().filter(|c| **c == "E5004").count(),
        2,
        "her kontrat kendi tanısını almalı: {found:?}"
    );
}

#[test]
fn contract_error_expression_no_e5004_cascade() {
    // Bilinmeyen isim E1001 alır; tip Error olduğundan E5004 kaskadı yok.
    let found = codes(&uart_module("    invariant: unknown_signal\n"));
    assert!(found.contains(&"E1001"), "{found:?}");
    assert!(!found.contains(&"E5004"), "kaskad bastırılmalı: {found:?}");
}

// ═══ Kapsam kuralları ═════════════════════════════════════════════

#[test]
fn requires_may_reference_ports_only() {
    let found = codes(&uart_module("    requires: busy_r\n"));
    assert!(
        found.contains(&"E1001"),
        "requires register göremez: {found:?}"
    );
}

#[test]
fn ensures_register_is_e1001() {
    let found = codes(&uart_module("    ensures: !busy_r\n"));
    assert!(
        found.contains(&"E1001"),
        "ensures register göremez: {found:?}"
    );
}

#[test]
fn invariant_register_is_allowed() {
    let result = check(&uart_module("    invariant: !(busy_r && start)\n"));
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn invariant_let_binding_is_e1001() {
    let src = "module M {\n    in  clk : clock\n    in  a : bool\n    out q : bool\n\n    \
               invariant: idle\n    let idle : bool = !a\n    q = idle\n}\n";
    let found = codes(src);
    assert!(
        found.contains(&"E1001"),
        "invariant let bağlaması göremez: {found:?}"
    );
}

#[test]
fn cover_may_reference_everything() {
    let src = "module M {\n    in  clk : clock\n    in  a : bool\n    out q : bool\n\n    \
               reg r : bool = false\n    let idle : bool = !a\n\n    \
               cover: a && r && idle\n\n    on clk {\n        r <= a\n    }\n\n    \
               q = idle\n}\n";
    let result = check(src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn requires_const_is_allowed() {
    let src = "const LIMIT : u8 = 2\n\nmodule M {\n    in  clk : clock\n    \
               in  speed : u8\n    out q : u8\n\n    requires: speed <= LIMIT\n\n    \
               q = speed\n}\n";
    let result = check(src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

// ═══ Domain kuralı — kontrat sinyalleri aynı alanda ═══════════════

const TWO_DOMAINS: &str = "domain Fast { clock = posedge, reset = sync active_high }\n\
                           domain Slow { clock = posedge, reset = sync active_high }\n";

#[test]
fn contract_mixing_two_domains_is_e3001() {
    let src = format!(
        "{TWO_DOMAINS}\nmodule M {{\n    in  fast_clk : clock @Fast\n    \
         in  slow_clk : clock @Slow\n    in  fast_sig : bool  @Fast\n    \
         in  slow_sig : bool  @Slow\n\n    invariant: !(fast_sig && slow_sig)\n}}\n"
    );
    let found = codes(&src);
    assert!(
        found.contains(&"E3001"),
        "kontrat iki alanı birleştiremez: {found:?}"
    );
}

#[test]
fn contract_single_domain_signals_clean() {
    let src = format!(
        "{TWO_DOMAINS}\nmodule M {{\n    in  fast_clk : clock @Fast\n    \
         in  slow_clk : clock @Slow\n    in  a : bool @Fast\n    \
         in  b : bool @Fast\n\n    invariant: !(a && b)\n}}\n"
    );
    let result = check(&src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}
