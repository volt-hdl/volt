//! Güven seviyesi / bilgi akışı testleri — ADR-0052 (domain-inference.md
//! K11): trust_level ayrıştırma sonrası E3009, declassify + W3008, K11
//! saat boyutu takma adı, sınıflandırılmamış sinyal çıkarımı, K8
//! örnekleme, sync() etiket korunumu ve geriye uyumluluk.

use volt_hir::{analyze, AnalysisResult, DomainId};
use volt_span::SourceMap;
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

/// Yalnız akış/alan tanıları: kullanılmayan domain (W3004), port (W1001)
/// gibi bu özellikten bağımsız uyarılar test gürültüsüdür, elenir.
fn codes(src: &str) -> Vec<&'static str> {
    check(src)
        .error_codes()
        .into_iter()
        .filter(|c| matches!(*c, "E3001" | "E3009" | "E3010" | "W3008"))
        .collect()
}

fn count(src: &str, code: &str) -> usize {
    codes(src).iter().filter(|c| **c == code).count()
}

/// E3009 tanılarının birincil satırları (1 tabanlı).
fn e3009_lines(src: &str) -> Vec<u32> {
    let result = check(src);
    let mut map = SourceMap::new();
    map.add_file("t.volt", src.to_string());
    result
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == "E3009")
        .filter_map(|d| d.primary_span())
        .map(|s| map.line_col_utf8(s.span).0)
        .collect()
}

const DOMAINS: &str = "domain SecureCore { clock = posedge, reset = sync active_high, trust_level = secret }\n\
                       domain Internal   { clock = posedge, reset = sync active_high, trust_level = confidential }\n\
                       domain Debug      { clock = posedge, reset = sync active_high, trust_level = public }\n";

fn with_domains(body: &str) -> String {
    format!("{DOMAINS}\n{body}\n")
}

// ═══ Kural: yüksekten düşüğe akış yasak ═══════════════════════════

#[test]
fn secret_to_public_output_is_e3009() {
    let src = with_domains(
        "module M {\n    in  key : u8 @SecureCore\n    out dbg : u8 @Debug\n    dbg = key\n}",
    );
    assert_eq!(codes(&src), vec!["E3009"]);
    assert_eq!(e3009_lines(&src), vec![8]);
}

#[test]
fn secret_to_confidential_is_e3009() {
    let src = with_domains(
        "module M {\n    in  key : u8 @SecureCore\n    out cfg : u8 @Internal\n    cfg = key\n}",
    );
    assert_eq!(codes(&src), vec!["E3009"]);
}

#[test]
fn confidential_to_public_is_e3009() {
    let src = with_domains(
        "module M {\n    in  cfg : u8 @Internal\n    out dbg : u8 @Debug\n    dbg = cfg\n}",
    );
    assert_eq!(codes(&src), vec!["E3009"]);
}

#[test]
fn public_to_secret_is_free() {
    let src = with_domains(
        "module M {\n    in  ctl : bool @Debug\n    out gate : bool @SecureCore\n    gate = !ctl\n}",
    );
    assert!(codes(&src).is_empty(), "{:?}", codes(&src));
}

#[test]
fn same_level_is_free() {
    let src = with_domains(
        "module M {\n    in  a : u8 @SecureCore\n    out b : u8 @SecureCore\n    in  c : u8 @Debug\n    out d : u8 @Debug\n    b = a\n    d = c\n}",
    );
    assert!(codes(&src).is_empty(), "{:?}", codes(&src));
}

#[test]
fn constants_fit_every_level() {
    let src = with_domains(
        "module M {\n    out s : u8 @SecureCore\n    out p : u8 @Debug\n    s = 0\n    p = 255\n}",
    );
    assert!(codes(&src).is_empty(), "{:?}", codes(&src));
}

// ═══ K5 eşleniği: ifade en yüksek seviyeyi taşır ══════════════════

#[test]
fn mixed_expression_takes_highest_level() {
    // secret & public → secret; public hedefe akamaz.
    let src = with_domains(
        "module M {\n    in  key : u8 @SecureCore\n    in  pubv : u8 @Debug\n    out dbg : u8 @Debug\n    let mixed = key & pubv\n    dbg = mixed\n}",
    );
    assert_eq!(codes(&src), vec!["E3009"]);
    assert_eq!(e3009_lines(&src), vec![10]);
}

#[test]
fn mixed_expression_flows_upwards() {
    let src = with_domains(
        "module M {\n    in  key : u8 @SecureCore\n    in  pubv : u8 @Debug\n    out s : u8 @SecureCore\n    let mixed = key & pubv\n    s = mixed\n}",
    );
    assert!(codes(&src).is_empty(), "{:?}", codes(&src));
}

#[test]
fn slice_and_cast_keep_the_label() {
    let src = with_domains(
        "module M {\n    in  key : bits<128> @SecureCore\n    out dbg : u8 @Debug\n    dbg = key[7:0] as u8\n}",
    );
    assert_eq!(codes(&src), vec!["E3009"]);
}

#[test]
fn if_expression_condition_carries_the_label() {
    // `dbg = if key[0] { 1 } else { 0 }` = `dbg = key[0]`.
    let src = with_domains(
        "module M {\n    in  key : u8 @SecureCore\n    out dbg : u8 @Debug\n    dbg = if key[0] { 1 } else { 0 }\n}",
    );
    assert_eq!(codes(&src), vec!["E3009"]);
}

// ═══ K2/K4 eşleniği: register ve wire alanının seviyesini alır ═══

#[test]
fn register_in_secret_clock_domain_is_secret() {
    let src = with_domains(
        "module M {\n    in  clk : clock @SecureCore\n    in  d : u8 @Debug\n    out dbg : u8 @Debug\n    reg r : u8 = 0\n    on clk { r <= d }\n    dbg = r\n}",
    );
    // r, alanı (SecureCore) gereği secret; public çıkışa akamaz.
    assert_eq!(codes(&src), vec!["E3009"]);
    assert_eq!(e3009_lines(&src), vec![11]);
}

#[test]
fn write_of_secret_into_public_register_is_e3009_at_the_write() {
    let src = with_domains(
        "module M {\n    in  clk : clock @SecureCore\n    in  key : u8 @SecureCore\n    reg(Debug) dbg_r : u8 = 0\n    on clk { dbg_r <= key }\n}",
    );
    assert_eq!(codes(&src), vec!["E3009"]);
    assert_eq!(e3009_lines(&src), vec![9]);
}

// ═══ Sınıflandırılmamış sinyaller: yazılan en yüksek seviyeyi alır ═

#[test]
fn unclassified_register_cannot_launder_a_secret() {
    // clk anotasyonsuz: tmp sınıflandırılmamış → key yazılınca secret olur.
    let src = with_domains(
        "module M {\n    in  clk : clock\n    in  key : u8 @SecureCore\n    out dbg : u8 @Debug\n    reg tmp : u8 = 0\n    on clk { tmp <= key }\n    dbg = tmp\n}",
    );
    assert_eq!(codes(&src), vec!["E3009"]);
    assert_eq!(e3009_lines(&src), vec![11]);
}

#[test]
fn unclassified_register_fed_only_by_public_stays_free() {
    let src = with_domains(
        "module M {\n    in  clk : clock\n    in  key : u8 @SecureCore\n    in  ctl : u8 @Debug\n    out dbg : u8 @Debug\n    out s : u8 @SecureCore\n    reg tmp : u8 = 0\n    on clk { tmp <= ctl }\n    dbg = tmp\n    s = key\n}",
    );
    assert!(codes(&src).is_empty(), "{:?}", codes(&src));
}

#[test]
fn feedback_register_reaches_fixpoint() {
    // tmp <= tmp + key: geri besleme, sabit nokta tek turda secret'a çıkar.
    let src = with_domains(
        "module M {\n    in  clk : clock\n    in  key : u8 @SecureCore\n    out dbg : u8 @Debug\n    reg tmp : u8 = 0\n    on clk { tmp <= tmp + key }\n    dbg = tmp\n}",
    );
    assert_eq!(codes(&src), vec!["E3009"]);
}

#[test]
fn chain_of_unclassified_lets_propagates() {
    let src = with_domains(
        "module M {\n    in  key : u8 @SecureCore\n    out dbg : u8 @Debug\n    let a = key + 1\n    let b = a ^ 3\n    let c = b\n    dbg = c\n}",
    );
    assert_eq!(codes(&src), vec!["E3009"]);
    assert_eq!(e3009_lines(&src), vec![11]);
}

// ═══ Örtük akış: koşula bağlı yazma koşulun bilgisini taşır ═══════

#[test]
fn implicit_flow_through_if_condition_is_e3009() {
    let src = with_domains(
        "module M {\n    in  clk : clock\n    in  key : u8 @SecureCore\n    reg(Debug) dbg_r : bool = false\n    on clk {\n        if key[0] { dbg_r <= true } else { dbg_r <= false }\n    }\n}",
    );
    assert_eq!(count(&src, "E3009"), 2);
}

#[test]
fn implicit_flow_through_match_scrutinee_is_e3009() {
    let src = with_domains(
        "module M {\n    in  clk : clock\n    in  key : u2 @SecureCore\n    reg(Debug) dbg_r : bool = false\n    on clk {\n        match key {\n            0 => { dbg_r <= true }\n            _ => { dbg_r <= false }\n        }\n    }\n}",
    );
    assert_eq!(count(&src, "E3009"), 2);
}

#[test]
fn secret_index_into_public_array_is_e3009() {
    let src = with_domains(
        "module M {\n    in  clk : clock\n    in  key : u2 @SecureCore\n    in  v : bool @Debug\n    reg(Debug) tbl_r : [bool; 4] = [false; 4]\n    on clk { tbl_r[key] <= v }\n}",
    );
    assert_eq!(codes(&src), vec!["E3009"]);
}

// ═══ declassify — tek meşru düşürme, W3008 iz kaydı ═══════════════

#[test]
fn declassify_allows_the_flow_and_warns_w3008() {
    let src = with_domains(
        "module M {\n    in  key : u8 @SecureCore\n    out status : bool @Debug\n    status = declassify(key != 0, \"key presence only\")\n}",
    );
    assert_eq!(codes(&src), vec!["W3008"]);
}

#[test]
fn w3008_carries_reason_and_source_level() {
    let src = with_domains(
        "module M {\n    in  key : u8 @SecureCore\n    out status : bool @Debug\n    status = declassify(key != 0, \"key presence only\")\n}",
    );
    let result = check(&src);
    let w = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "W3008")
        .expect("W3008");
    assert!(w.message.contains("key presence only"), "{}", w.message);
    let primary = w.primary_span().expect("birincil etiket");
    assert!(
        primary.label.contains("secret") && primary.label.contains("public"),
        "{}",
        primary.label
    );
    assert!(primary.label.contains("@SecureCore"), "{}", primary.label);
}

#[test]
fn every_declassify_is_one_w3008() {
    let src = with_domains(
        "module M {\n    in  key : u8 @SecureCore\n    out a : bool @Debug\n    out b : bool @Debug\n    out c : u8 @Debug\n    a = declassify(key != 0, \"presence\")\n    b = declassify(key[0], \"parity bit\")\n    c = declassify(key, \"whole key, test build\")\n}",
    );
    assert_eq!(count(&src, "W3008"), 3);
    assert_eq!(count(&src, "E3009"), 0);
}

#[test]
fn declassified_value_is_public_afterwards() {
    // let ile bağlanan declassify sonucu public'tir, register'dan geçse de.
    let src = with_domains(
        "module M {\n    in  clk : clock\n    in  key : u8 @SecureCore\n    out dbg : bool @Debug\n    let p = declassify(key != 0, \"presence\")\n    reg r : bool = false\n    on clk { r <= p }\n    dbg = r\n}",
    );
    assert_eq!(codes(&src), vec!["W3008"]);
}

#[test]
fn declassify_of_unclassified_value_still_warns() {
    // trust_level yok, yine de iz kaydı.
    let src =
        "module M {\n    in  a : u8\n    out b : bool\n    b = declassify(a != 0, \"why not\")\n}";
    let result = check(src);
    assert_eq!(codes(src), vec!["W3008"]);
    let w = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "W3008")
        .unwrap();
    assert!(
        w.primary_span().unwrap().label.contains("unclassified"),
        "{}",
        w.primary_span().unwrap().label
    );
}

#[test]
fn declassify_only_covers_its_own_expression() {
    // declassify(a) & key → key hâlâ secret.
    let src = with_domains(
        "module M {\n    in  key : u8 @SecureCore\n    out dbg : u8 @Debug\n    dbg = declassify(key, \"partial\") & key\n}",
    );
    let c = codes(&src);
    assert!(c.contains(&"E3009") && c.contains(&"W3008"), "{c:?}");
}

// ═══ K9: sync() saati değiştirir, etiketi korur ═══════════════════

#[test]
fn sync_preserves_the_trust_label() {
    let src = "domain Fast { clock = posedge, reset = sync active_high, trust_level = secret }\n\
               domain Slow { clock = posedge, reset = sync active_high, trust_level = public }\n\
               module M {\n    in  fast_clk : clock @Fast\n    in  slow_clk : clock @Slow\n    in  flag : bool @Fast\n    out out_flag : bool @Slow\n    out_flag = sync(flag, slow_clk)\n}";
    // CDC köprüsü doğru (E3001 yok) ama gizli bit açık alana geçemez.
    assert_eq!(codes(src), vec!["E3009"]);
}

#[test]
fn sync_with_declassify_is_the_sanctioned_bridge() {
    let src = "domain Fast { clock = posedge, reset = sync active_high, trust_level = secret }\n\
               domain Slow { clock = posedge, reset = sync active_high, trust_level = public }\n\
               module M {\n    in  fast_clk : clock @Fast\n    in  slow_clk : clock @Slow\n    in  flag : bool @Fast\n    out out_flag : bool @Slow\n    out_flag = sync(declassify(flag, \"status bit\"), slow_clk)\n}";
    assert_eq!(codes(src), vec!["W3008"]);
}

// ═══ K11: trust_level'lı anotasyon saat alanı AÇMAZ ═══════════════

#[test]
fn trust_only_annotation_does_not_open_a_clock_domain() {
    // Tek saat, iki güven bölgesi: E3001 yok, sinyaller aynı saat alanında.
    let src = with_domains(
        "module M {\n    in  clk : clock @SecureCore\n    in  key : u8 @SecureCore\n    in  ctl : bool @Debug\n    out s : u8 @SecureCore\n    reg r : u8 = 0\n    on clk { if ctl { r <= key } }\n    s = r\n}",
    );
    let result = check(&src);
    assert!(codes(&src).is_empty(), "{:?}", result.error_codes());
    let clk_dom = result.domain.signal_domains.values().next().copied();
    assert!(clk_dom.is_some());
    // Bütün sinyaller tek alanda.
    let distinct: std::collections::HashSet<_> = result
        .domain
        .signal_domains
        .values()
        .filter_map(|d| match d {
            DomainId::Explicit(i) => Some(*i),
            _ => None,
        })
        .collect();
    assert_eq!(distinct.len(), 1, "{distinct:?}");
}

#[test]
fn trust_only_annotation_in_multi_clock_module_is_ambiguous_e3010() {
    let src = "domain Fast { clock = posedge, reset = sync active_high }\n\
               domain Slow { clock = posedge, reset = sync active_high }\n\
               domain Debug { clock = posedge, reset = sync active_high, trust_level = public }\n\
               module M {\n    in  fast_clk : clock @Fast\n    in  slow_clk : clock @Slow\n    in  d : u8 @Debug\n    out q : u8 @Fast\n    q = d\n}";
    assert!(codes(src).contains(&"E3010"), "{:?}", codes(src));
}

#[test]
fn trust_domain_anchored_by_a_clock_port_is_a_clock_domain() {
    // Debug bir clock portunda taşınıyor → gerçek saat alanı, E3001 korunur.
    let src = with_domains(
        "module M {\n    in  sclk : clock @SecureCore\n    in  dclk : clock @Debug\n    in  d : u8 @Debug\n    out q : u8 @SecureCore\n    q = d\n}",
    );
    assert!(codes(&src).contains(&"E3001"), "{:?}", codes(&src));
}

#[test]
fn annotation_without_trust_level_keeps_old_behaviour() {
    // Geriye uyumluluk: trust_level'sız, saatle taşınmayan anotasyon
    // eskisi gibi ayrı alandır (E3001).
    let src = "domain Other { clock = posedge, reset = sync active_high }\n\
               module M {\n    in  clk : clock\n    in  d : u8 @Other\n    out q : u8\n    reg r : u8 = 0\n    on clk { r <= d }\n    q = r\n}";
    assert!(codes(src).contains(&"E3001"), "{:?}", codes(src));
}

// ═══ K8: örnekleme ════════════════════════════════════════════════

#[test]
fn secret_into_public_input_port_of_instance_is_e3009() {
    let src = with_domains(
        "module Sink {\n    in  clk : clock\n    in  d : u8 @Debug\n    out q : u8 @Debug\n    q = d\n}\n\
         module Top {\n    in  clk : clock\n    in  key : u8 @SecureCore\n    out o : u8 @SecureCore\n    let s = Sink { clk: clk, d: key }\n    o = s.q\n}",
    );
    assert_eq!(codes(&src), vec!["E3009"]);
    assert_eq!(e3009_lines(&src), vec![15]);
}

#[test]
fn instance_output_carries_the_port_level() {
    let src = with_domains(
        "module Src {\n    in  clk : clock\n    in  k : u8 @SecureCore\n    out q : u8 @SecureCore\n    q = k\n}\n\
         module Top {\n    in  clk : clock\n    in  key : u8 @SecureCore\n    out dbg : u8 @Debug\n    let s = Src { clk: clk, k: key }\n    dbg = s.q\n}",
    );
    assert_eq!(codes(&src), vec!["E3009"]);
    assert_eq!(e3009_lines(&src), vec![16]);
}

#[test]
fn unclassified_submodule_cannot_launder_a_secret() {
    // Adder hiç trust bilmez; çıkışı girişlerinin en yükseğini taşır.
    let src = with_domains(
        "module Adder {\n    in  a : u8\n    in  b : u8\n    out sum : u8\n    sum = a + b\n}\n\
         module Top {\n    in  key : u8 @SecureCore\n    out dbg : u8 @Debug\n    let add = Adder { a: key, b: 1 }\n    dbg = add.sum\n}",
    );
    assert_eq!(codes(&src), vec!["E3009"]);
}

#[test]
fn unclassified_submodule_with_public_inputs_is_free() {
    let src = with_domains(
        "module Adder {\n    in  a : u8\n    in  b : u8\n    out sum : u8\n    sum = a + b\n}\n\
         module Top {\n    in  ctl : u8 @Debug\n    out dbg : u8 @Debug\n    let add = Adder { a: ctl, b: 1 }\n    dbg = add.sum\n}",
    );
    assert!(codes(&src).is_empty(), "{:?}", codes(&src));
}

#[test]
fn aliased_port_at_instantiation_binds_to_the_single_clock() {
    // K11 örneklemede: @Debug portu hedefte saat taşımıyor → üst modülün
    // saat alanından bağlanır, E3001 çıkmaz.
    let src = with_domains(
        "module Leaf {\n    in  clk : clock\n    in  ctl : bool @Debug\n    out q : bool @Debug\n    reg r : bool = false\n    on clk { r <= ctl }\n    q = r\n}\n\
         module Top {\n    in  clk : clock\n    in  ctl : bool @Debug\n    out q : bool @Debug\n    let l = Leaf { clk: clk, ctl: ctl }\n    q = l.q\n}",
    );
    assert!(codes(&src).is_empty(), "{:?}", codes(&src));
}

// ═══ Tanı biçimi (5 parça) ════════════════════════════════════════

#[test]
fn e3009_names_both_levels_and_points_at_both_domains() {
    let src = with_domains(
        "module M {\n    in  key : u8 @SecureCore\n    out dbg : u8 @Debug\n    dbg = key\n}",
    );
    let result = check(&src);
    let d = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E3009")
        .expect("E3009");
    assert_eq!(d.message, "secret data flows to a public output");
    let primary = d.primary_span().unwrap();
    assert_eq!(primary.label, "@Debug (public)");
    let labels: Vec<&str> = d.spans.iter().map(|s| s.label.as_str()).collect();
    assert!(labels.contains(&"@SecureCore (secret)"), "{labels:?}");
    assert!(labels.contains(&"source trust level here"), "{labels:?}");
    assert!(
        labels.contains(&"destination trust level here"),
        "{labels:?}"
    );
    assert!(d.help.as_deref().unwrap_or("").contains("declassify"));
    assert!(d
        .notes
        .iter()
        .any(|n| n.text.contains("higher trust level")));
}

#[test]
fn e3009_sink_kind_names_register_and_port() {
    let reg = with_domains(
        "module M {\n    in  clk : clock\n    in  key : u8 @SecureCore\n    reg(Debug) r : u8 = 0\n    on clk { r <= key }\n}",
    );
    let msg = |src: &str| {
        check(src)
            .diagnostics
            .into_iter()
            .find(|d| d.code.as_str() == "E3009")
            .map(|d| d.message)
            .unwrap_or_default()
    };
    assert_eq!(msg(&reg), "secret data flows to a public register");
    let port = with_domains(
        "module Sink {\n    in  d : u8 @Debug\n    out q : u8 @Debug\n    q = d\n}\n\
         module Top {\n    in  key : u8 @SecureCore\n    out o : u8 @SecureCore\n    let s = Sink { d: key }\n    o = s.q\n}",
    );
    assert_eq!(msg(&port), "secret data flows to a public port");
}

// ═══ Geriye uyumluluk ═════════════════════════════════════════════

#[test]
fn modules_without_trust_level_are_untouched() {
    // Aynı yapı, trust_level yok: ne E3009 ne W3008 — hiçbir tanı.
    let src = "domain A { clock = posedge, reset = sync active_high }\n\
               module M {\n    in  clk : clock @A\n    in  key : u8\n    out dbg : u8\n    reg r : u8 = 0\n    on clk { r <= key }\n    dbg = r\n}";
    assert!(codes(src).is_empty(), "{:?}", codes(src));
}

#[test]
fn trust_result_exposes_domain_levels() {
    let src = with_domains(
        "module M {\n    in  a : u8 @SecureCore\n    out b : u8 @SecureCore\n    b = a\n}",
    );
    let result = check(&src);
    let levels: Vec<Option<volt_ast::TrustLevel>> =
        result.domain.domains.iter().map(|d| d.trust).collect();
    assert_eq!(
        levels,
        vec![
            Some(volt_ast::TrustLevel::Secret),
            Some(volt_ast::TrustLevel::Confidential),
            Some(volt_ast::TrustLevel::Public)
        ]
    );
    assert!(result.domain.domains.iter().all(|d| d.trust_span.is_some()));
}

#[test]
fn trust_level_ordering_is_public_confidential_secret() {
    use volt_ast::TrustLevel::*;
    assert!(Public < Confidential && Confidential < Secret);
    assert_eq!(volt_ast::TrustLevel::parse("secret"), Some(Secret));
    assert_eq!(volt_ast::TrustLevel::parse("top_secret"), None);
    assert_eq!(Confidential.as_str(), "confidential");
}
