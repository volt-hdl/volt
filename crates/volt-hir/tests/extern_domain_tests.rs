//! Extern modül sınırında domain anotasyonu ve CDC denetimi (ADR-0047).
//!
//! `extern module` içinde tanımsız `@Ad` sembolik domain parametresidir;
//! örneklemede saat bağlantısı gerçek alana bağlar (K8), diğer portlar
//! o haritaya göre denetlenir. E3001 (yanlış alan), E3014 (aynı sembolik
//! alana iki farklı saat), E3002 (saatsiz sembolik alan), E3010 (çok
//! saatli extern'de anotasyonsuz port).

use volt_hir::{analyze, AnalysisResult, DefKind, Ty};
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

fn errors(result: &AnalysisResult) -> Vec<&'static str> {
    result
        .diagnostics
        .iter()
        .filter(|d| !d.code.is_warning())
        .map(|d| d.code.as_str())
        .collect()
}

fn count(result: &AnalysisResult, code: &str) -> usize {
    result
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == code)
        .count()
}

const DOMAINS: &str = "domain Fast { clock = posedge, reset = sync active_high }\n\
                       domain Slow { clock = posedge, reset = sync active_high }\n";

/// İki sembolik alanlı asenkron FIFO sarmalayıcısı.
const FIFO: &str = "extern module ExtFifo {\n    \
    in  wr_clk   : clock @Src\n    in  wr_data  : u8    @Src\n    \
    in  wr_en    : bool  @Src\n    out wr_full  : bool  @Src\n    \
    in  rd_clk   : clock @Dst\n    out rd_data  : u8    @Dst\n    \
    in  rd_en    : bool  @Dst\n    out rd_empty : bool  @Dst\n}\n";

/// İki saat portu tek sembolik alanda (tek saatli SV çekirdeği).
const REGFILE: &str = "extern module ExtRegFile {\n    \
    in  wr_clk  : clock @Core\n    in  rd_clk  : clock @Core\n    \
    in  wr_data : u8    @Core\n    out rd_data : u8    @Core\n}\n";

/// Doğru bağlanmış köprü: yazma tarafı Fast, okuma tarafı Slow.
const GOOD_BRIDGE: &str = "module Bridge {\n    \
    in  fast_clk : clock @Fast\n    in  fast_d   : u8    @Fast\n    \
    in  fast_en  : bool  @Fast\n    out fast_full: bool  @Fast\n    \
    in  slow_clk : clock @Slow\n    in  slow_en  : bool  @Slow\n    \
    out slow_d   : u8    @Slow\n    out slow_empty : bool @Slow\n    \
    let f = ExtFifo {\n        wr_clk: fast_clk,\n        wr_data: fast_d,\n        \
    wr_en: fast_en,\n        rd_clk: slow_clk,\n        rd_en: slow_en,\n    }\n    \
    fast_full = f.wr_full\n    slow_d = f.rd_data\n    slow_empty = f.rd_empty\n}\n";

fn src(parts: &[&str]) -> String {
    parts.concat()
}

// ═══ İsim çözümleme: extern portları ve sembolik alanlar ═══════════

#[test]
fn extern_ports_are_declared_as_defs() {
    let result = check(&src(&[DOMAINS, FIFO]));
    let names: Vec<&str> = result
        .resolve
        .defs
        .iter()
        .filter(|d| matches!(d.kind, DefKind::Port { .. }))
        .map(|d| d.name.as_str())
        .collect();
    assert!(names.contains(&"wr_clk"), "{names:?}");
    assert!(names.contains(&"rd_data"), "{names:?}");
}

#[test]
fn extern_unknown_annotation_becomes_symbolic_domain_param() {
    let result = check(&src(&[DOMAINS, FIFO]));
    let params: Vec<&str> = result
        .resolve
        .defs
        .iter()
        .filter(|d| d.kind == DefKind::DomainParam)
        .map(|d| d.name.as_str())
        .collect();
    assert!(params.contains(&"Src"), "{params:?}");
    assert!(params.contains(&"Dst"), "{params:?}");
}

#[test]
fn extern_same_symbolic_name_resolves_to_one_def() {
    let result = check(&src(&[DOMAINS, FIFO]));
    let src_defs = result
        .resolve
        .defs
        .iter()
        .filter(|d| d.kind == DefKind::DomainParam && d.name == "Src")
        .count();
    assert_eq!(src_defs, 1, "dört @Src tek tanıma çözülmeli");
}

#[test]
fn extern_symbolic_domain_is_not_e3002_at_declaration() {
    let result = check(&src(&[DOMAINS, FIFO]));
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn module_unknown_domain_is_still_e3002() {
    // Regresyon: sembolik alanlar yalnız extern içinde; modülde tanımsız
    // @Ad hâlâ E3002.
    let result = check(
        "module M {\n    in clk : clock @Nowhere\n    in a : u8\n    out b : u8\n    b = a\n}\n",
    );
    assert!(
        errors(&result).contains(&"E3002"),
        "{:?}",
        result.error_codes()
    );
}

#[test]
fn extern_ports_do_not_raise_unused_warnings() {
    let result = check(&src(&[DOMAINS, FIFO]));
    assert_eq!(count(&result, "W1001"), 0, "{:?}", result.error_codes());
}

#[test]
fn extern_duplicate_port_is_e1003() {
    let result = check(
        "extern module X {\n    in clk : clock\n    in a : u8\n    in a : u8\n    out q : u8\n}\n",
    );
    assert!(
        errors(&result).contains(&"E1003"),
        "{:?}",
        result.error_codes()
    );
}

#[test]
fn extern_concrete_domain_annotation_resolves_to_domain_decl() {
    let result = check(&src(&[
        DOMAINS,
        "extern module X {\n    in clk : clock @Fast\n    in a : u8 @Fast\n    out q : u8 @Fast\n}\n",
    ]));
    assert!(!result.has_errors(), "{:?}", result.error_codes());
    let params = result
        .resolve
        .defs
        .iter()
        .filter(|d| d.kind == DefKind::DomainParam)
        .count();
    assert_eq!(
        params, 0,
        "@Fast gerçek domain — sembolik parametre açılmamalı"
    );
}

#[test]
fn extern_annotation_may_name_a_clock_port() {
    // Modüllerdeki gibi @clk_adı anotasyonu extern'de de geçerli.
    let result = check(&src(&[
        DOMAINS,
        "extern module X {\n    in a_clk : clock\n    in b_clk : clock\n    in a : u8 @a_clk\n    out q : u8 @b_clk\n}\n",
    ]));
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

// ═══ Tip kontrolü: extern port tipleri ════════════════════════════

#[test]
fn extern_clock_ports_are_typed_as_clock() {
    let result = check(&src(&[DOMAINS, FIFO]));
    let wr_clk = result
        .resolve
        .defs
        .iter()
        .position(|d| d.name == "wr_clk")
        .map(|i| volt_hir::DefId(i as u32))
        .expect("wr_clk tanımı");
    let ty = result
        .typeck
        .def_types
        .get(&wr_clk)
        .copied()
        .expect("extern portu tiplenmeli");
    assert!(matches!(result.typeck.types.ty(ty), Ty::Clock));
}

// ═══ Extern bildirimi denetimleri ═════════════════════════════════

#[test]
fn extern_symbolic_domain_without_clock_port_is_e3002() {
    let result = check(
        "extern module X {\n    in clk : clock @Src\n    in a : u8 @Src\n    out q : u8 @Dst\n}\n",
    );
    assert_eq!(count(&result, "E3002"), 1, "{:?}", result.error_codes());
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E3002")
        .expect("E3002");
    assert!(diag.message.contains("Dst"), "{}", diag.message);
    assert!(
        diag.help.as_deref().unwrap_or("").contains("clock"),
        "{:?}",
        diag.help
    );
}

#[test]
fn extern_multi_clock_unannotated_port_is_e3010() {
    let result = check(
        "extern module X {\n    in a_clk : clock\n    in b_clk : clock\n    in a : u8\n    out q : u8 @b_clk\n}\n",
    );
    assert_eq!(count(&result, "E3010"), 1, "{:?}", result.error_codes());
}

#[test]
fn extern_single_clock_unannotated_is_clean() {
    let result = check("extern module X {\n    in clk : clock\n    in a : u8\n    out q : u8\n}\n");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn extern_clockless_ports_are_timeless() {
    // Kombinasyonel extern: her alandan bağlanabilir, çıkışı her alana gider.
    let result = check(&src(&[
        DOMAINS,
        "extern module Inv {\n    in a : u8\n    out y : u8\n}\n",
        "module M {\n    in fast_clk : clock @Fast\n    in slow_clk : clock @Slow\n    \
         in fast_d : u8 @Fast\n    out slow_q : u8 @Slow\n    \
         let i = Inv { a: fast_d }\n    slow_q = i.y\n}\n",
    ]));
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

// ═══ K8 — örneklemede sembolik → gerçek alan haritası ═════════════

#[test]
fn extern_correct_wiring_is_clean() {
    let result = check(&src(&[DOMAINS, FIFO, GOOD_BRIDGE]));
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn extern_input_from_wrong_domain_is_e3001() {
    let bad = GOOD_BRIDGE.replace("wr_data: fast_d", "wr_data: slow_d_in");
    let bad = bad.replace(
        "in  fast_d   : u8    @Fast\n",
        "in  fast_d   : u8    @Fast\n    in  slow_d_in : u8   @Slow\n",
    );
    let result = check(&src(&[DOMAINS, FIFO, &bad]));
    assert_eq!(count(&result, "E3001"), 1, "{:?}", result.error_codes());
}

#[test]
fn extern_output_read_carries_bound_domain() {
    // f.rd_data @Dst → Slow; Fast çıkışına atamak E3001.
    let bad = GOOD_BRIDGE.replace("slow_d = f.rd_data", "fast_q = f.rd_data");
    let bad = bad.replace(
        "out fast_full: bool  @Fast\n",
        "out fast_full: bool  @Fast\n    out fast_q   : u8    @Fast\n",
    );
    let bad = bad.replace("out slow_d   : u8    @Slow\n", "");
    let result = check(&src(&[DOMAINS, FIFO, &bad]));
    assert_eq!(count(&result, "E3001"), 1, "{:?}", result.error_codes());
}

#[test]
fn extern_output_into_foreign_on_block_is_e3001() {
    let bad = GOOD_BRIDGE.replace(
        "slow_d = f.rd_data",
        "reg r : u8 = 0\n    on fast_clk { r <= f.rd_data }\n    slow_d = r",
    );
    let result = check(&src(&[DOMAINS, FIFO, &bad]));
    assert!(
        errors(&result).contains(&"E3001"),
        "{:?}",
        result.error_codes()
    );
}

#[test]
fn extern_single_clock_bound_to_slow_rejects_fast_data() {
    let result = check(&src(&[
        DOMAINS,
        "extern module Dly {\n    in clk : clock\n    in d : u8\n    out q : u8\n}\n",
        "module M {\n    in fast_clk : clock @Fast\n    in slow_clk : clock @Slow\n    \
         in fast_d : u8 @Fast\n    out slow_q : u8 @Slow\n    \
         let x = Dly { clk: slow_clk, d: fast_d }\n    slow_q = x.q\n}\n",
    ]));
    assert_eq!(count(&result, "E3001"), 1, "{:?}", result.error_codes());
}

#[test]
fn extern_unbound_symbolic_clock_has_no_false_positive() {
    // wr_clk bağlanmamış: @Src bilinmiyor, wr_data denetlenemez — sessiz.
    let bad = GOOD_BRIDGE.replace("wr_clk: fast_clk,\n", "");
    let result = check(&src(&[DOMAINS, FIFO, &bad]));
    assert_eq!(count(&result, "E3001"), 0, "{:?}", result.error_codes());
}

// ═══ E3014 — aynı sembolik alana iki farklı saat ═════════════════

const RF_BAD: &str = "module M {\n    \
    in  fast_clk : clock @Fast\n    in  fast_d   : u8    @Fast\n    \
    in  slow_clk : clock @Slow\n    out slow_q   : u8    @Slow\n    \
    let rf = ExtRegFile {\n        wr_clk: fast_clk,\n        rd_clk: slow_clk,\n        \
    wr_data: fast_d,\n    }\n    slow_q = rf.rd_data\n}\n";

#[test]
fn extern_same_symbolic_domain_two_clocks_is_e3014() {
    let result = check(&src(&[DOMAINS, REGFILE, RF_BAD]));
    assert_eq!(count(&result, "E3014"), 1, "{:?}", result.error_codes());
}

#[test]
fn e3014_suppresses_cascading_e3001() {
    // Tek kök neden, tek hata: çakışan alan Error'a düşer, port
    // denetimleri E3001 üretmez.
    let result = check(&src(&[DOMAINS, REGFILE, RF_BAD]));
    assert_eq!(count(&result, "E3001"), 0, "{:?}", result.error_codes());
}

#[test]
fn e3014_has_five_parts_and_names_the_symbolic_domain() {
    let result = check(&src(&[DOMAINS, REGFILE, RF_BAD]));
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E3014")
        .expect("E3014");
    assert!(diag.message.contains("@Core"), "{}", diag.message);
    assert!(!diag.help.as_deref().unwrap_or("").is_empty(), "help boş");
    assert!(
        diag.notes.iter().any(|n| n.text.contains("ADR-0047")),
        "{:?}",
        diag.notes
    );
    assert!(
        diag.spans.len() >= 2,
        "ilk saat bağlaması ikincil etiket taşımalı"
    );
}

#[test]
fn extern_same_symbolic_domain_same_clock_domain_is_clean() {
    let good = RF_BAD.replace("rd_clk: slow_clk", "rd_clk: fast_clk");
    let good = good.replace("out slow_q   : u8    @Slow", "out slow_q   : u8    @Fast");
    let result = check(&src(&[DOMAINS, REGFILE, &good]));
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn module_two_clock_ports_same_domain_bound_apart_is_e3014() {
    // K8 sıkılaştırması modüller için de geçerli: iki saat portu aynı
    // @Fast alanında bildirilmiş, farklı alanlardan saatlerle bağlanmış.
    let result = check(&src(&[
        DOMAINS,
        "module Inner {\n    in a_clk : clock @Fast\n    in b_clk : clock @Fast\n    \
         in d : u8 @Fast\n    out q : u8 @Fast\n    reg r : u8 = 0\n    \
         on a_clk { r <= d }\n    q = r\n}\n",
        "module Outer {\n    in fast_clk : clock @Fast\n    in slow_clk : clock @Slow\n    \
         in fast_d : u8 @Fast\n    out fast_q : u8 @Fast\n    \
         let i = Inner { a_clk: fast_clk, b_clk: slow_clk, d: fast_d }\n    fast_q = i.q\n}\n",
    ]));
    assert_eq!(count(&result, "E3014"), 1, "{:?}", result.error_codes());
}

// ═══ İnceleme bulguları (H1/M1/M2/M4) ═════════════════════════════

#[test]
fn symbolic_domain_is_not_shadowed_by_root_struct() {
    // H1: kök kapsamdaki `struct Src` sembolik @Src'yi gölgelemez.
    let result = check(&src(&[
        DOMAINS,
        "struct Src {\n    a : u8\n}\n",
        FIFO,
        GOOD_BRIDGE,
    ]));
    assert!(!result.has_errors(), "{:?}", result.error_codes());
    let params = result
        .resolve
        .defs
        .iter()
        .filter(|d| d.kind == DefKind::DomainParam && d.name == "Src")
        .count();
    assert_eq!(params, 1);
}

#[test]
fn symbolic_domain_is_not_shadowed_by_root_const_and_still_checks() {
    // H1'in sessiz yarısı: gölgeleme olsaydı beklenen alan Timeless'a düşer,
    // yanlış bağlama denetimsiz kalırdı.
    let bad = GOOD_BRIDGE.replace("wr_data: fast_d", "wr_data: slow_d_in");
    let bad = bad.replace(
        "in  fast_d   : u8    @Fast\n",
        "in  fast_d   : u8    @Fast\n    in  slow_d_in : u8   @Slow\n",
    );
    let result = check(&src(&[DOMAINS, "const Src : u8 = 0\n", FIFO, &bad]));
    assert_eq!(count(&result, "E3001"), 1, "{:?}", result.error_codes());
    assert_eq!(count(&result, "E3002"), 0, "{:?}", result.error_codes());
}

#[test]
fn extern_annotation_on_non_clock_port_is_e3002() {
    // M1: modülle aynı kural — `@sel` clock tipinde değil.
    let result = check(
        "extern module X {\n    in clk : clock\n    in sel : bool\n    in data : u8 @sel\n    out q : u8 @clk\n}\n",
    );
    assert_eq!(count(&result, "E3002"), 1, "{:?}", result.error_codes());
}

#[test]
fn extern_port_named_like_root_item_has_no_shadow_warning() {
    // M2: extern'in gövdesi yok, gölgeleme karışıklık yaratamaz.
    let result = check(
        "const addr : u8 = 0\nextern module Ram {\n    in clk : clock\n    in addr : u4\n    out q : u8\n}\n",
    );
    assert_eq!(count(&result, "W1002"), 0, "{:?}", result.error_codes());
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn two_instances_of_one_extern_keep_separate_mappings() {
    let m = "module M {\n    \
        in  fast_clk : clock @Fast\n    in  fast_d : u8 @Fast\n    in  fast_en : bool @Fast\n    \
        in  slow_clk : clock @Slow\n    in  slow_d : u8 @Slow\n    in  slow_en : bool @Slow\n    \
        out fast_q : u8 @Fast\n    out slow_q : u8 @Slow\n    \
        let a = ExtFifo {\n        wr_clk: fast_clk,\n        wr_data: fast_d,\n        wr_en: fast_en,\n        rd_clk: slow_clk,\n        rd_en: slow_en,\n    }\n    \
        let b = ExtFifo {\n        wr_clk: slow_clk,\n        wr_data: slow_d,\n        wr_en: slow_en,\n        rd_clk: fast_clk,\n        rd_en: fast_en,\n    }\n    \
        slow_q = a.rd_data\n    fast_q = b.rd_data\n}\n";
    let result = check(&src(&[DOMAINS, FIFO, m]));
    assert!(!result.has_errors(), "{:?}", result.error_codes());
    // Çapraz okuma: b.rd_data @Dst := Fast, Slow çıkışına atamak E3001.
    let bad = m
        .replace("fast_q = b.rd_data", "fast_q = a.rd_data")
        .replace("slow_q = a.rd_data", "slow_q = b.rd_data");
    let result = check(&src(&[DOMAINS, FIFO, &bad]));
    assert_eq!(count(&result, "E3001"), 2, "{:?}", result.error_codes());
}

#[test]
fn three_clocks_on_one_symbolic_domain_report_one_e3014() {
    let result = check(&src(&[
        DOMAINS,
        "domain Mid { clock = posedge, reset = sync active_high }\n",
        "extern module Tri {\n    in a_clk : clock @Core\n    in b_clk : clock @Core\n    \
         in c_clk : clock @Core\n    in d : u8 @Core\n    out q : u8 @Core\n}\n",
        "module M {\n    in fast_clk : clock @Fast\n    in slow_clk : clock @Slow\n    \
         in mid_clk : clock @Mid\n    in fast_d : u8 @Fast\n    out fast_q : u8 @Fast\n    \
         let t = Tri { a_clk: fast_clk, b_clk: slow_clk, c_clk: mid_clk, d: fast_d }\n    \
         fast_q = t.q\n}\n",
    ]));
    assert_eq!(count(&result, "E3014"), 1, "{:?}", result.error_codes());
    assert_eq!(count(&result, "E3001"), 0, "{:?}", result.error_codes());
}

#[test]
fn bundle_field_annotations_in_two_externs_keep_separate_symbolic_domains() {
    // M4: struct port alanının @Src anotasyonu iki extern'e klonlanır;
    // span paylaşımı tanımları ezseydi ikinci extern'de anahtar bölünür,
    // yanlış bağlama denetimsiz kalırdı.
    let result = check(&src(&[
        DOMAINS,
        "struct port WrSide {\n    in wr_clk : clock @Src\n    in wr_data : u8 @Src\n}\n",
        "extern module A {\n    in ws : WrSide\n    in rd_clk : clock @Dst\n    out rd_data : u8 @Dst\n}\n",
        "extern module B {\n    in ws : WrSide\n    in rd_clk : clock @Dst\n    out rd_data : u8 @Dst\n}\n",
        "module M {\n    in fast_clk : clock @Fast\n    in slow_clk : clock @Slow\n    \
         in fast_d : u8 @Fast\n    out slow_q : u8 @Slow\n    out fast_q : u8 @Fast\n    \
         let a = A { ws_wr_clk: fast_clk, ws_wr_data: fast_d, rd_clk: slow_clk }\n    \
         let b = B { ws_wr_clk: slow_clk, ws_wr_data: fast_d, rd_clk: fast_clk }\n    \
         slow_q = a.rd_data\n    fast_q = b.rd_data\n}\n",
    ]));
    // Yalnız b.ws_wr_data yanlış (Slow bekliyor, Fast geldi).
    assert_eq!(count(&result, "E3001"), 1, "{:?}", result.error_codes());
    assert_eq!(count(&result, "E3002"), 0, "{:?}", result.error_codes());
}
