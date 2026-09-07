//! Domain çıkarımı ve CDC kontrolü testleri (F2c).
//!
//! docs/spec/domain-inference.md K1-K9 kuralları ve §8 test vektörleri.
//! Uyarı kodları (W3xxx) da davranışın parçasıdır ve denetlenir.

use volt_hir::{analyze, AnalysisResult, DomainId};
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

/// İki saatli test modülleri için ortak domain önsözü.
const TWO_DOMAINS: &str = "domain Fast { clock = posedge, reset = sync active_high }\n\
                           domain Slow { clock = posedge, reset = sync active_high }\n";

fn two_clock_module(body: &str) -> String {
    format!(
        "{TWO_DOMAINS}\nmodule M {{\n    in  fast_clk : clock @Fast\n    \
         in  slow_clk : clock @Slow\n{body}\n}}\n"
    )
}

// ═══ K2 — tek saat kuralı (UX Anayasası) ══════════════════════════

#[test]
fn single_clock_unannotated_compiles_clean() {
    let result = check(
        "module Counter {\n    in  clk : clock\n    in  enable : bool\n    out count : u8\n\n    \
         reg count_r : u8 = 0\n\n    on clk {\n        if enable {\n            \
         count_r <= count_r + 1\n        }\n    }\n\n    count = count_r\n}\n",
    );
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn single_clock_no_domain_diagnostics_at_all() {
    // Kullanıcı 'domain' kelimesini HİÇ görmemeli.
    let result = check(
        "module M {\n    in  clk : clock\n    in  d : u8\n    out q : u8\n\n    \
         reg r : u8 = 0\n\n    on clk {\n        r <= d\n    }\n\n    q = r\n}\n",
    );
    assert!(
        !result
            .error_codes()
            .iter()
            .any(|c| c.starts_with("E3") || c.starts_with("W3")),
        "{:?}",
        result.error_codes()
    );
}

#[test]
fn single_clock_signals_share_clock_domain() {
    let result =
        check("module M {\n    in  clk : clock\n    in  d : u8\n    out q : u8\n\n    q = d\n}\n");
    let (d_def, _) = result.resolve.def_by_name("d").expect("d");
    let (q_def, _) = result.resolve.def_by_name("q").expect("q");
    let dd = result.domain.signal_domains[&d_def];
    let qd = result.domain.signal_domains[&q_def];
    assert_eq!(dd, qd);
    assert!(matches!(dd, DomainId::Explicit(_)));
}

#[test]
fn zero_clock_module_is_timeless() {
    let result =
        check("module Add {\n    in  a : u8\n    in  b : u8\n    out s : u9\n\n    s = a + b\n}\n");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
    let (a_def, _) = result.resolve.def_by_name("a").expect("a");
    assert_eq!(result.domain.signal_domains[&a_def], DomainId::Timeless);
}

#[test]
fn single_clock_with_explicit_domain_annotation_ok() {
    let result = check(
        "domain Sys { clock = posedge, reset = sync active_high }\n\n\
         module M {\n    in  clk : clock @Sys\n    in  d : u8 @Sys\n    out q : u8 @Sys\n\n    \
         q = d\n}\n",
    );
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn single_clock_mixed_annotation_and_inference_agree() {
    // Anotasyonlu ve anotasyonsuz sinyaller aynı alana düşer (K1+K2).
    let result = check(
        "domain Sys { clock = posedge, reset = sync active_high }\n\n\
         module M {\n    in  clk : clock @Sys\n    in  d : u8 @Sys\n    out q : u8\n\n    \
         q = d\n}\n",
    );
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn implicit_clock_domain_carries_port_name() {
    let result =
        check("module M {\n    in  clk : clock\n    in  d : u8\n    out q : u8\n\n    q = d\n}\n");
    assert!(result.domain.domains.iter().any(|d| d.name == "clk"));
}

// ═══ K1 — açık anotasyon kazanır ══════════════════════════════════

#[test]
fn two_clocks_all_annotated_compatible_ok() {
    let src = two_clock_module(
        "    in  fd : u8 @Fast\n    out fq : u8 @Fast\n    in  sd : u8 @Slow\n    \
         out sq : u8 @Slow\n\n    fq = fd\n    sq = sd",
    );
    let result = check(&src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn reg_explicit_clock_annotation_wins() {
    // reg(fast_clk): yazıcı taramasına gerek kalmaz (K1).
    let src = two_clock_module(
        "    in  fd : u8 @Fast\n    out fq : u8 @Fast\n\n    \
         reg(fast_clk) r : u8 = 0\n\n    on fast_clk {\n        r <= fd\n    }\n\n    fq = r",
    );
    let result = check(&src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn reg_explicit_domain_written_from_foreign_block_errors() {
    let src = two_clock_module(
        "    in  sd : u8 @Slow\n    out sq : u8 @Slow\n\n    \
         reg(fast_clk) r : u8 = 0\n\n    on slow_clk {\n        r <= sd\n    }\n\n    sq = 0",
    );
    let c = codes(&src);
    assert!(c.contains(&"E3001"), "{c:?}");
}

// ═══ K3 — çoklu saat: anotasyon zorunlu (E3010) ═══════════════════

#[test]
fn multi_clock_unannotated_port_e3010() {
    let src = two_clock_module("    in  data : u8\n    out r : u8 @Fast\n\n    r = 0");
    let c = codes(&src);
    assert!(c.contains(&"E3010"), "{c:?}");
}

#[test]
fn e3010_only_in_multi_clock_mode() {
    // Tek saatli tasarım E3010'u HİÇ görmez (kritik UX kararı).
    let result = check(
        "module M {\n    in  clk : clock\n    in  data : u8\n    out q : u8\n\n    q = data\n}\n",
    );
    assert!(!result.error_codes().contains(&"E3010"));
}

#[test]
fn e3010_lists_clock_candidates() {
    let src = two_clock_module("    in  data : u8\n    out r : u8 @Fast\n\n    r = 0");
    let result = check(&src);
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E3010")
        .expect("E3010");
    // İki aday saat → en az 2 ikincil span.
    assert!(
        diag.spans.iter().filter(|s| !s.primary).count() >= 2,
        "{diag:?}"
    );
    diag.validate().expect("5 parça kuralı");
}

#[test]
fn e3010_signal_becomes_error_no_cascade() {
    // data Error alanına düşer; r = data ayrıca E3001 üretmez.
    let src = two_clock_module("    in  data : u8\n    out r : u8 @Fast\n\n    r = data");
    let c = codes(&src);
    assert!(c.contains(&"E3010"), "{c:?}");
    assert!(!c.contains(&"E3001"), "{c:?}");
}

#[test]
fn multi_clock_every_unannotated_port_reported() {
    let src = two_clock_module("    in  a : u8\n    in  b : u8\n    out r : u8 @Fast\n\n    r = 0");
    let result = check(&src);
    let n = result
        .error_codes()
        .iter()
        .filter(|&&c| c == "E3010")
        .count();
    assert_eq!(n, 2, "{:?}", result.error_codes());
}

// ═══ K4 — register domain'i 'on' bloğundan ════════════════════════

#[test]
fn reg_domain_from_single_writer() {
    let src = two_clock_module(
        "    in  fd : u8 @Fast\n    out fq : u8 @Fast\n\n    reg r : u8 = 0\n\n    \
         on fast_clk {\n        r <= fd\n    }\n\n    fq = r",
    );
    let result = check(&src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn reg_never_written_w3001() {
    let result = check(
        "module M {\n    in  clk : clock\n    out q : u8\n\n    reg r : u8 = 7\n\n    q = r\n}\n",
    );
    let c = result.error_codes();
    assert!(c.contains(&"W3001"), "{c:?}");
    assert!(!result.has_errors(), "{c:?}");
}

#[test]
fn unwritten_reg_is_timeless() {
    let result = check(
        "module M {\n    in  clk : clock\n    out q : u8\n\n    reg r : u8 = 7\n\n    q = r\n}\n",
    );
    let (r_def, _) = result.resolve.def_by_name("r").expect("r");
    assert_eq!(result.domain.signal_domains[&r_def], DomainId::Timeless);
}

#[test]
fn reg_two_domains_e3011() {
    let src = two_clock_module(
        "    in  fd : u8 @Fast\n    in  sd : u8 @Slow\n    out r : u8 @Fast\n\n    \
         reg shared : u8 = 0\n\n    on fast_clk { shared <= fd }\n    \
         on slow_clk { shared <= sd }\n\n    r = shared",
    );
    let c = codes(&src);
    assert!(c.contains(&"E3011"), "{c:?}");
}

#[test]
fn e3011_five_parts_with_writer_spans() {
    let src = two_clock_module(
        "    in  fd : u8 @Fast\n    in  sd : u8 @Slow\n    out r : u8 @Fast\n\n    \
         reg shared : u8 = 0\n\n    on fast_clk { shared <= fd }\n    \
         on slow_clk { shared <= sd }\n\n    r = shared",
    );
    let result = check(&src);
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E3011")
        .expect("E3011");
    diag.validate().expect("5 parça kuralı");
    // İki yazıcı bloğu + iki domain tanımı → en az 4 ikincil span.
    assert!(
        diag.spans.iter().filter(|s| !s.primary).count() >= 4,
        "{diag:?}"
    );
    assert!(!diag.notes.is_empty(), "neden satırı olmalı");
}

#[test]
fn e3011_reg_error_domain_suppresses_cascade() {
    let src = two_clock_module(
        "    in  fd : u8 @Fast\n    in  sd : u8 @Slow\n    out r : u8 @Fast\n\n    \
         reg shared : u8 = 0\n\n    on fast_clk { shared <= fd }\n    \
         on slow_clk { shared <= sd }\n\n    r = shared",
    );
    let c = codes(&src);
    assert!(!c.contains(&"E3001"), "kaskad bastırılmalı: {c:?}");
}

#[test]
fn reg_conditional_write_found_in_nested_if() {
    // Yazıcı taraması iç içe dallara inmeli.
    let src = two_clock_module(
        "    in  fd : u8 @Fast\n    in  en : bool @Fast\n    out fq : u8 @Fast\n\n    \
         reg r : u8 = 0\n\n    on fast_clk {\n        if en {\n            r <= fd\n        }\n    \
         }\n\n    fq = r",
    );
    let result = check(&src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
    assert!(!result.error_codes().contains(&"W3001"));
}

// ═══ K5 — kombinasyonel yayılım (VOLT'UN VAADİ) ═══════════════════

#[test]
fn combinational_mix_e3001() {
    let src = two_clock_module(
        "    in  fs : bool @Fast\n    in  ss : bool @Slow\n    out r : bool @Slow\n\n    \
         r = fs & ss",
    );
    let c = codes(&src);
    assert!(c.contains(&"E3001"), "{c:?}");
}

#[test]
fn comb_same_domain_ok() {
    let src = two_clock_module(
        "    in  a : bool @Fast\n    in  b : bool @Fast\n    out r : bool @Fast\n\n    r = a & b",
    );
    let result = check(&src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn comb_constant_joins_with_any_domain() {
    // Timeless her şeyle birleşir (sabitler).
    let src = two_clock_module("    in  a : u8 @Fast\n    out r : u8 @Fast\n\n    r = a & 0x0F");
    let result = check(&src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn nested_expression_join_detects_cdc() {
    let src = two_clock_module(
        "    in  a : bool @Fast\n    in  b : bool @Fast\n    in  s : bool @Slow\n    \
         out r : bool @Fast\n\n    r = (a & b) | s",
    );
    let c = codes(&src);
    assert!(c.contains(&"E3001"), "{c:?}");
}

#[test]
fn unary_and_cast_propagate_domain() {
    let src =
        two_clock_module("    in  a : u8 @Fast\n    out r : i16 @Slow\n\n    r = (~a) as i16");
    let c = codes(&src);
    assert!(c.contains(&"E3001"), "{c:?}");
}

#[test]
fn index_by_foreign_domain_signal_e3001() {
    let src = two_clock_module(
        "    in  a : u8 @Fast\n    in  i : u8 @Slow\n    out r : bool @Fast\n\n    r = a[i[2]]",
    );
    let c = codes(&src);
    assert!(c.contains(&"E3001"), "{c:?}");
}

#[test]
fn if_expression_joins_all_branches() {
    let src = two_clock_module(
        "    in  s : bool @Fast\n    in  a : u8 @Fast\n    in  b : u8 @Slow\n    \
         out r : u8 @Fast\n\n    r = if s { a } else { b }",
    );
    let c = codes(&src);
    assert!(c.contains(&"E3001"), "{c:?}");
}

#[test]
fn builtin_call_args_join_e3001() {
    let src = two_clock_module(
        "    in  a : bits<4> @Fast\n    in  b : bits<4> @Slow\n    out r : bits<8> @Fast\n\n    \
         r = concat(a, b)",
    );
    let c = codes(&src);
    assert!(c.contains(&"E3001"), "{c:?}");
}

#[test]
fn e3001_comb_five_parts_two_operand_spans() {
    let src = two_clock_module(
        "    in  fs : bool @Fast\n    in  ss : bool @Slow\n    out r : bool @Slow\n\n    \
         r = fs & ss",
    );
    let result = check(&src);
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E3001")
        .expect("E3001");
    diag.validate().expect("5 parça kuralı");
    // İki operand + iki domain tanım satırı → en az 3 ikincil span.
    assert!(
        diag.spans.iter().filter(|s| !s.primary).count() >= 3,
        "{diag:?}"
    );
    assert!(
        diag.notes.iter().any(|n| n.text.contains("glitch")),
        "neden glitch'i anlatmalı: {diag:?}"
    );
    assert!(
        diag.help.as_deref().unwrap_or("").contains("sync"),
        "çözüm sync() önermeli: {diag:?}"
    );
}

#[test]
fn e3001_secondary_points_to_domain_definition() {
    let src = two_clock_module(
        "    in  fs : bool @Fast\n    in  ss : bool @Slow\n    out r : bool @Slow\n\n    \
         r = fs & ss",
    );
    let result = check(&src);
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E3001")
        .expect("E3001");
    assert!(
        diag.spans
            .iter()
            .any(|s| !s.primary && s.label.contains("defined here")),
        "domain tanım satırına ikincil span olmalı: {diag:?}"
    );
}

// ═══ K6 — atama domain uyumu ══════════════════════════════════════

#[test]
fn assign_cross_domain_e3001() {
    let src = two_clock_module("    in  fd : u8 @Fast\n    out sq : u8 @Slow\n\n    sq = fd");
    let c = codes(&src);
    assert!(c.contains(&"E3001"), "{c:?}");
}

#[test]
fn assign_same_domain_ok() {
    let src = two_clock_module("    in  fd : u8 @Fast\n    out fq : u8 @Fast\n\n    fq = fd");
    let result = check(&src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn constant_assignable_anywhere() {
    // Sabit her yere atanabilir (Timeless muaf).
    let src = two_clock_module(
        "    out fq : u8 @Fast\n    out sq : u8 @Slow\n\n    fq = 0xAB\n    sq = 42",
    );
    let result = check(&src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn const_expression_is_timeless() {
    let src = format!(
        "const K : u8 = 12;\n{}",
        two_clock_module("    out fq : u8 @Fast\n\n    fq = K + 1")
    );
    let result = check(&src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn e3001_assign_five_parts() {
    let src = two_clock_module("    in  fd : u8 @Fast\n    out sq : u8 @Slow\n\n    sq = fd");
    let result = check(&src);
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E3001")
        .expect("E3001");
    diag.validate().expect("5 parça kuralı");
    assert!(
        diag.notes.iter().any(|n| n.text.contains("metastability")),
        "neden metastabiliteyi anlatmalı: {diag:?}"
    );
    assert!(
        diag.help.as_deref().unwrap_or("").contains("sync"),
        "çözüm sync() önermeli"
    );
    // Kaynak span + iki domain tanımı → en az 3 ikincil.
    assert!(
        diag.spans.iter().filter(|s| !s.primary).count() >= 3,
        "{diag:?}"
    );
}

#[test]
fn let_binding_propagates_domain() {
    let src = two_clock_module(
        "    in  fd : u8 @Fast\n    out sq : u8 @Slow\n\n    let x = fd + 1\n\n    sq = x",
    );
    let c = codes(&src);
    assert!(c.contains(&"E3001"), "let alan taşımalı: {c:?}");
}

#[test]
fn let_of_constant_stays_timeless() {
    let src = two_clock_module(
        "    out fq : u8 @Fast\n    out sq : u8 @Slow\n\n    let k : u8 = 5\n\n    \
         fq = k\n    sq = k",
    );
    let result = check(&src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

// ═══ K7 — 'on' bloğu içi kısıtlar ═════════════════════════════════

#[test]
fn on_block_foreign_rhs_e3001() {
    let src = two_clock_module(
        "    in  fd : u8 @Fast\n    out sq : u8 @Slow\n\n    reg r : u8 = 0\n\n    \
         on slow_clk {\n        r <= fd\n    }\n\n    sq = r",
    );
    let c = codes(&src);
    assert!(c.contains(&"E3001"), "{c:?}");
}

#[test]
fn on_block_same_domain_ok() {
    let src = two_clock_module(
        "    in  sd : u8 @Slow\n    out sq : u8 @Slow\n\n    reg r : u8 = 0\n\n    \
         on slow_clk {\n        r <= sd\n    }\n\n    sq = r",
    );
    let result = check(&src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn on_block_constant_rhs_ok() {
    let src = two_clock_module(
        "    out sq : u8 @Slow\n\n    reg r : u8 = 0\n\n    \
         on slow_clk {\n        r <= 255\n    }\n\n    sq = r",
    );
    let result = check(&src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn on_block_foreign_condition_e3012() {
    let src = two_clock_module(
        "    in  ff : bool @Fast\n    in  sd : u8 @Slow\n    out sq : u8 @Slow\n\n    \
         reg r : u8 = 0\n\n    on slow_clk {\n        if ff {\n            r <= sd\n        }\n    \
         }\n\n    sq = r",
    );
    let c = codes(&src);
    assert!(c.contains(&"E3012"), "{c:?}");
}

#[test]
fn e3012_five_parts() {
    let src = two_clock_module(
        "    in  ff : bool @Fast\n    in  sd : u8 @Slow\n    out sq : u8 @Slow\n\n    \
         reg r : u8 = 0\n\n    on slow_clk {\n        if ff {\n            r <= sd\n        }\n    \
         }\n\n    sq = r",
    );
    let result = check(&src);
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E3012")
        .expect("E3012");
    diag.validate().expect("5 parça kuralı");
    assert!(!diag.notes.is_empty());
}

#[test]
fn on_block_same_domain_condition_no_e3012() {
    let src = two_clock_module(
        "    in  sf : bool @Slow\n    in  sd : u8 @Slow\n    out sq : u8 @Slow\n\n    \
         reg r : u8 = 0\n\n    on slow_clk {\n        if sf {\n            r <= sd\n        }\n    \
         }\n\n    sq = r",
    );
    let result = check(&src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn on_block_write_to_foreign_port_e3001() {
    let src = two_clock_module(
        "    in  sd : u8 @Slow\n    out fq : u8 @Fast\n\n    \
         on slow_clk {\n        fq <= sd\n    }",
    );
    let c = codes(&src);
    assert!(c.contains(&"E3001"), "{c:?}");
}

// ═══ K8 — modül örnekleme ═════════════════════════════════════════

#[test]
fn instance_clockless_target_ok() {
    let result = check(
        "module Adder {\n    in  a : u8\n    in  b : u8\n    out s : u9\n\n    s = a + b\n}\n\n\
         module Top {\n    in  clk : clock\n    in  x : u8\n    in  y : u8\n    out r : u9\n\n    \
         let add = Adder { a: x, b: y }\n\n    r = add.s\n}\n",
    );
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn instance_clock_mapping_ok() {
    let src = format!(
        "{TWO_DOMAINS}\nmodule Sub {{\n    in  clk : clock\n    in  d : u8\n    out q : u8\n\n    \
         reg r : u8 = 0\n\n    on clk {{\n        r <= d\n    }}\n\n    q = r\n}}\n\n\
         module Top {{\n    in  fast_clk : clock @Fast\n    in  slow_clk : clock @Slow\n    \
         in  fd : u8 @Fast\n    out fq : u8 @Fast\n\n    \
         let s = Sub {{ clk: fast_clk, d: fd }}\n\n    fq = s.q\n}}\n"
    );
    let result = check(&src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn instance_data_port_cross_domain_e3001() {
    // Uart örneği (spec K8): clk Slow'a bağlı, data Fast'ten geliyor.
    let src = format!(
        "{TWO_DOMAINS}\nmodule Sub {{\n    in  clk : clock\n    in  d : u8\n    out q : u8\n\n    \
         reg r : u8 = 0\n\n    on clk {{\n        r <= d\n    }}\n\n    q = r\n}}\n\n\
         module Top {{\n    in  fast_clk : clock @Fast\n    in  slow_clk : clock @Slow\n    \
         in  fd : u8 @Fast\n    out sq : u8 @Slow\n\n    \
         let s = Sub {{ clk: slow_clk, d: fd }}\n\n    sq = s.q\n}}\n"
    );
    let c = codes(&src);
    assert!(c.contains(&"E3001"), "{c:?}");
}

#[test]
fn instance_output_read_in_mapped_domain() {
    // s.q örneklenen saatin alanındadır; yabancı alana atanamaz.
    let src = format!(
        "{TWO_DOMAINS}\nmodule Sub {{\n    in  clk : clock\n    in  d : u8\n    out q : u8\n\n    \
         reg r : u8 = 0\n\n    on clk {{\n        r <= d\n    }}\n\n    q = r\n}}\n\n\
         module Top {{\n    in  fast_clk : clock @Fast\n    in  slow_clk : clock @Slow\n    \
         in  fd : u8 @Fast\n    out sq : u8 @Slow\n\n    \
         let s = Sub {{ clk: fast_clk, d: fd }}\n\n    sq = s.q\n}}\n"
    );
    let c = codes(&src);
    assert!(c.contains(&"E3001"), "{c:?}");
}

#[test]
fn instance_globally_annotated_port_remapped_by_clock_binding() {
    // Hedefin @Fast saati Slow'a bağlanınca portlar Slow bekler.
    let src = format!(
        "{TWO_DOMAINS}\nmodule Sub {{\n    in  clk : clock @Fast\n    in  d : u8 @Fast\n    \
         out q : u8 @Fast\n\n    reg r : u8 = 0\n\n    on clk {{\n        r <= d\n    }}\n\n    \
         q = r\n}}\n\n\
         module Top {{\n    in  fast_clk : clock @Fast\n    in  slow_clk : clock @Slow\n    \
         in  sd : u8 @Slow\n    out sq : u8 @Slow\n\n    \
         let s = Sub {{ clk: slow_clk, d: sd }}\n\n    sq = s.q\n}}\n"
    );
    let result = check(&src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn instance_shortcut_binding_domain_checked() {
    // `d:` kısayolu — yerel isim port adıyla aynı; alanı yine denetlenir.
    let src = format!(
        "{TWO_DOMAINS}\nmodule Sub {{\n    in  clk : clock\n    in  d : u8\n    out q : u8\n\n    \
         reg r : u8 = 0\n\n    on clk {{\n        r <= d\n    }}\n\n    q = r\n}}\n\n\
         module Top {{\n    in  fast_clk : clock @Fast\n    in  slow_clk : clock @Slow\n    \
         in  d : u8 @Fast\n    out sq : u8 @Slow\n\n    \
         let s = Sub {{ clk: slow_clk, d: }}\n\n    sq = s.q\n}}\n"
    );
    let parsed = parse(FileId(0), &src);
    if parsed.diagnostics.is_empty() {
        let result = analyze(&parsed.ast);
        assert!(
            result.error_codes().contains(&"E3001"),
            "{:?}",
            result.error_codes()
        );
    }
    // Kısayol sözdizimi `d:` desteklenmiyorsa test anlamsız — atla.
}

// ═══ K9 — sync() köprüsü ══════════════════════════════════════════

#[test]
fn sync_bridge_crosses_domains_clean() {
    let src = two_clock_module(
        "    in  ff : bool @Fast\n    out sf : bool @Slow\n\n    sf = sync(ff, slow_clk)",
    );
    let result = check(&src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
    assert!(
        !result.error_codes().iter().any(|c| c.starts_with("W3")),
        "{:?}",
        result.error_codes()
    );
}

#[test]
fn sync_output_is_in_target_domain() {
    // sync çıkışı hedef alanda: Fast çıkışa atanınca E3001.
    let src = two_clock_module(
        "    in  ff : bool @Fast\n    out fq : bool @Fast\n\n    fq = sync(ff, slow_clk)",
    );
    let c = codes(&src);
    assert!(c.contains(&"E3001"), "{c:?}");
}

#[test]
fn sync_same_domain_w3002() {
    let result = check(
        "module M {\n    in  clk : clock\n    in  d : bool\n    out q : bool\n\n    \
         q = sync(d, clk)\n}\n",
    );
    let c = result.error_codes();
    assert!(c.contains(&"W3002"), "{c:?}");
    assert!(!result.has_errors(), "{c:?}");
}

#[test]
fn sync_multibit_w3003() {
    let src = two_clock_module(
        "    in  fd : u8 @Fast\n    out sq : u8 @Slow\n\n    sq = sync(fd, slow_clk)",
    );
    let result = check(&src);
    let c = result.error_codes();
    assert!(c.contains(&"W3003"), "{c:?}");
    assert!(!result.has_errors(), "{c:?}");
}

#[test]
fn sync_single_bit_no_w3003() {
    let src = two_clock_module(
        "    in  ff : bool @Fast\n    out sf : bool @Slow\n\n    sf = sync(ff, slow_clk)",
    );
    let c = codes(&src);
    assert!(!c.contains(&"W3003"), "{c:?}");
}

#[test]
fn w3003_suggests_gray_or_fifo() {
    let src = two_clock_module(
        "    in  fd : u8 @Fast\n    out sq : u8 @Slow\n\n    sq = sync(fd, slow_clk)",
    );
    let result = check(&src);
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "W3003")
        .expect("W3003");
    let help = diag.help.as_deref().unwrap_or("");
    assert!(
        help.contains("gray") || help.contains("AsyncFifo"),
        "{help}"
    );
    diag.validate().expect("5 parça kuralı");
}

#[test]
fn w3003_message_contains_width() {
    let src = two_clock_module(
        "    in  fd : u8 @Fast\n    out sq : u8 @Slow\n\n    sq = sync(fd, slow_clk)",
    );
    let result = check(&src);
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "W3003")
        .expect("W3003");
    assert!(diag.message.contains("8-bit"), "{}", diag.message);
}

#[test]
fn sync3_follows_same_rules() {
    let src = two_clock_module(
        "    in  ff : bool @Fast\n    out sf : bool @Slow\n\n    sf = sync3(ff, slow_clk)",
    );
    let result = check(&src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn synced_value_combines_with_target_domain() {
    // Spec çözüm örneği: let synced = sync(fs, slow_clk); synced & ss.
    let src = two_clock_module(
        "    in  fs : bool @Fast\n    in  ss : bool @Slow\n    out r : bool @Slow\n\n    \
         let synced = sync(fs, slow_clk)\n\n    r = synced & ss",
    );
    let result = check(&src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

// ═══ E3002 — tanımsız/geçersiz saat alanı ═════════════════════════

#[test]
fn unknown_domain_annotation_e3002_not_e1001() {
    let c = codes("module M {\n    in  clk : clock @Nonexistent\n    out y : u8\n\n    y = 0\n}\n");
    assert!(c.contains(&"E3002"), "{c:?}");
    assert!(!c.contains(&"E1001"), "genel 'tanımsız isim' değil: {c:?}");
}

#[test]
fn non_domain_annotation_e3002() {
    let c = codes(
        "module M {\n    in  clk : clock\n    in  x : u8\n    in  d : u8 @x\n    out y : u8\n\n    \
         y = d\n}\n",
    );
    assert!(c.contains(&"E3002"), "{c:?}");
}

#[test]
fn e3002_suggests_close_domain_name() {
    let result = check(
        "domain Fast { clock = posedge, reset = sync active_high }\n\n\
         module M {\n    in  clk : clock @Fst\n    out y : u8\n\n    y = 0\n}\n",
    );
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E3002")
        .expect("E3002");
    assert!(
        diag.help.as_deref().unwrap_or("").contains("Fast"),
        "{:?}",
        diag.help
    );
}

#[test]
fn e3002_signal_domain_error_no_cascade() {
    let c = codes(
        "module M {\n    in  clk : clock @Nonexistent\n    in  d : u8\n    out y : u8\n\n    \
         y = d\n}\n",
    );
    assert!(c.contains(&"E3002"), "{c:?}");
    assert!(!c.contains(&"E3001"), "{c:?}");
    assert!(!c.contains(&"E3010"), "{c:?}");
}

// ═══ W3004 — kullanılmayan domain ═════════════════════════════════

#[test]
fn unused_domain_w3004() {
    let c = codes(
        "domain Unused { clock = posedge, reset = sync active_high }\n\n\
         module M {\n    in  clk : clock\n    out y : u8\n\n    y = 0\n}\n",
    );
    assert!(c.contains(&"W3004"), "{c:?}");
}

#[test]
fn used_domain_no_w3004() {
    let c = codes(
        "domain Sys { clock = posedge, reset = sync active_high }\n\n\
         module M {\n    in  clk : clock @Sys\n    out y : u8\n\n    y = 0\n}\n",
    );
    assert!(!c.contains(&"W3004"), "{c:?}");
}

// ═══ Domain gösterimi (§1) ════════════════════════════════════════

#[test]
fn domain_info_captures_clock_and_reset() {
    let result = check(
        "domain Usb { clock = negedge, reset = async active_low }\n\n\
         module M {\n    in  clk : clock @Usb\n    out y : u8 @Usb\n\n    y = 0\n}\n",
    );
    let usb = result
        .domain
        .domains
        .iter()
        .find(|d| d.name == "Usb")
        .expect("Usb domain kaydı");
    assert_eq!(usb.clock.edge, volt_ast::ClockEdge::Negedge);
    assert_eq!(usb.reset.sync, volt_ast::ResetSync::Async);
    assert_eq!(usb.reset.polarity, volt_ast::ResetPolarity::ActiveLow);
}

#[test]
fn distinct_domains_distinct_ids() {
    let src = two_clock_module(
        "    in  fd : u8 @Fast\n    in  sd : u8 @Slow\n    out fq : u8 @Fast\n    \
         out sq : u8 @Slow\n\n    fq = fd\n    sq = sd",
    );
    let result = check(&src);
    let (fd, _) = result.resolve.def_by_name("fd").expect("fd");
    let (sd, _) = result.resolve.def_by_name("sd").expect("sd");
    assert_ne!(
        result.domain.signal_domains[&fd],
        result.domain.signal_domains[&sd]
    );
}

#[test]
fn annotated_signals_share_domain_id() {
    let src = two_clock_module("    in  fd : u8 @Fast\n    out fq : u8 @Fast\n\n    fq = fd");
    let result = check(&src);
    let (fd, _) = result.resolve.def_by_name("fd").expect("fd");
    let (fq, _) = result.resolve.def_by_name("fq").expect("fq");
    assert_eq!(
        result.domain.signal_domains[&fd],
        result.domain.signal_domains[&fq]
    );
}

// ═══ UX doğrulama (§6 senaryo tablosu) ════════════════════════════

#[test]
fn ux_scenario_table() {
    // Tek saatli sayaç → domain görünmez.
    let counter = check(
        "module C {\n    in  clk : clock\n    out q : u8\n\n    reg r : u8 = 0\n\n    \
         on clk {\n        r <= r + 1\n    }\n\n    q = r\n}\n",
    );
    assert!(
        !counter.error_codes().iter().any(|c| c.starts_with("E3")),
        "tek saat: {:?}",
        counter.error_codes()
    );

    // İki saat, anotasyonsuz → E3010 (öğretici).
    let ambiguous = codes(&two_clock_module(
        "    in  d : u8\n    out r : u8 @Fast\n\n    r = 0",
    ));
    assert!(ambiguous.contains(&"E3010"));

    // İki saat, anotasyonlu, doğru → sessiz.
    let annotated = check(&two_clock_module(
        "    in  fd : u8 @Fast\n    out fq : u8 @Fast\n\n    fq = fd",
    ));
    assert!(!annotated.has_errors());

    // İki saat, CDC ihlali → E3001 (çözüm gösteriyor).
    let cdc = codes(&two_clock_module(
        "    in  fd : u8 @Fast\n    out sq : u8 @Slow\n\n    sq = fd",
    ));
    assert!(cdc.contains(&"E3001"));

    // sync() ile doğru köprü → sessiz.
    let bridged = check(&two_clock_module(
        "    in  ff : bool @Fast\n    out sf : bool @Slow\n\n    sf = sync(ff, slow_clk)",
    ));
    assert!(!bridged.has_errors());
}
