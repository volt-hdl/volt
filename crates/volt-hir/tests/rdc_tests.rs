//! RDC — reset alanı denetimi (ADR-0065): E3003 (R5 paylaşım, R6
//! yakınsama, ham port türü, ad çakışması), E3010 (belirsiz ham port),
//! W3009 (birim kökünde async sözleşme), W3010 (R5' senkron paylaşım).

use volt_diagnostics::Diagnostic;
use volt_hir::analyze;
use volt_syntax::{parse, FileId};

const ASYNC_TWO: &str = "domain Fast { clock = posedge, reset = async active_low }\n\
                         domain Slow { clock = posedge, reset = async active_low }\n";
const SYNC_TWO: &str = "domain Fast { clock = posedge, reset = sync active_high }\n\
                        domain Slow { clock = posedge, reset = sync active_high }\n";

fn diags(src: &str) -> Vec<Diagnostic> {
    let parsed = parse(FileId(0), src);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.error_codes());
    analyze(&parsed.ast).diagnostics
}

/// Yalnız RDC ilgili kodlar (E3003/E3010/W3009/W3010), sırasıyla.
fn rdc_codes(src: &str) -> Vec<&'static str> {
    diags(src)
        .iter()
        .map(|d| d.code.as_str())
        .filter(|c| matches!(*c, "E3003" | "E3010" | "W3009" | "W3010"))
        .collect()
}

/// `Fast`/`Slow` alanlı, iki saatin de register sürdüğü modül.
fn two_clock(domains: &str, extra_ports: &str) -> String {
    format!(
        "{domains}module M {{\n    in fast_clk : clock @Fast\n    in slow_clk : clock @Slow\n\
         {extra_ports}    in a : u8 @Fast\n    out qa : u8 @Fast\n    out qb : u8 @Slow\n\
         reg(fast_clk) ra : u8 = 0\n    reg(slow_clk) rb : u8 = 0\n\
         on fast_clk {{ ra <= a }}\n    on slow_clk {{ rb <= rb + 1 }}\n    qa = ra\n    qb = rb\n}}\n"
    )
}

// ═══ R5 — tek async portun iki saatte paylaşımı (E3003) ═══════════

#[test]
fn async_reset_shared_by_two_used_clocks_is_e3003() {
    let d = diags(&two_clock(ASYNC_TWO, ""));
    let e: Vec<_> = d.iter().filter(|d| d.code.as_str() == "E3003").collect();
    assert_eq!(
        e.len(),
        1,
        "{:?}",
        d.iter().map(|d| d.code.as_str()).collect::<Vec<_>>()
    );
    assert!(e[0].message.contains("'rst_n'"), "{}", e[0].message);
    let help = e[0].help.as_deref().unwrap_or_default();
    assert!(
        help.contains("in rst_n : reset(async, active_low)"),
        "{help}"
    );
    // Birincil + iki alan reset etiketi + ikinci saat etiketi.
    assert_eq!(e[0].spans.len(), 4);
}

#[test]
fn e3003_suppresses_the_root_warning_of_the_same_module() {
    assert_eq!(rdc_codes(&two_clock(ASYNC_TWO, "")), ["E3003"]);
}

#[test]
fn unused_clock_does_not_count_as_a_second_domain() {
    // Saatin flop'u yoksa reset'i örnekleyen de yok (ui/pass/13 deseni).
    let src = format!(
        "{ASYNC_TWO}module M {{\n    in fast_clk : clock @Fast\n    in slow_clk : clock @Slow\n\
         in a : bool @Fast\n    out q : bool @Slow\n    q = sync(a, slow_clk)\n}}\n"
    );
    assert_eq!(rdc_codes(&src), ["W3009"]);
}

#[test]
fn different_polarities_use_different_ports_and_are_not_shared() {
    let src = two_clock(
        "domain Fast { clock = posedge, reset = async active_low }\n\
         domain Slow { clock = posedge, reset = async active_high }\n",
        "",
    );
    assert_eq!(rdc_codes(&src), ["W3009"]);
}

#[test]
fn async_and_sync_sharing_one_port_is_e3003() {
    let src = two_clock(
        "domain Fast { clock = posedge, reset = async active_high }\n\
         domain Slow { clock = posedge, reset = sync active_high }\n",
        "",
    );
    assert_eq!(rdc_codes(&src), ["E3003"]);
}

#[test]
fn reset_none_domain_is_never_shared() {
    let src = two_clock(
        "domain Fast { clock = posedge, reset = async active_low }\n\
         domain Slow { clock = posedge, reset = none }\n",
        "",
    );
    assert_eq!(rdc_codes(&src), ["W3009"]);
}

// ═══ R5' — senkron paylaşım (W3010) ═══════════════════════════════

#[test]
fn sync_reset_shared_is_a_warning_not_an_error() {
    let d = diags(&two_clock(SYNC_TWO, ""));
    let w: Vec<_> = d.iter().filter(|d| d.code.as_str() == "W3010").collect();
    assert_eq!(w.len(), 1);
    assert!(!d.iter().any(|d| d.code.as_str() == "E3003"));
    assert!(
        w[0].message.contains("'fast_clk', 'slow_clk'"),
        "{}",
        w[0].message
    );
}

#[test]
fn implicit_clock_domains_default_to_a_shared_sync_rst() {
    // Anotasyonsuz iki saat: örtük alanlar sv-emit'te `sync active_high`.
    let src = "module M {\n    in a_clk : clock\n    in b_clk : clock\n    out qa : u8 @a_clk\n\
               out qb : u8 @b_clk\n    reg(a_clk) ra : u8 = 0\n    reg(b_clk) rb : u8 = 0\n\
               on a_clk { ra <= ra + 1 }\n    on b_clk { rb <= rb + 1 }\n    qa = ra\n    qb = rb\n}\n";
    let d = diags(src);
    let codes: Vec<_> = d.iter().map(|d| d.code.as_str()).collect();
    if codes.iter().any(|c| c.starts_with('E')) {
        // Örtük alan adına anotasyon desteklenmiyorsa bu test anlamsız
        // kalır; o durumda yalnız W3010'un hataya dönüşmediği denetlenir.
        assert!(!codes.contains(&"E3003"), "{codes:?}");
        return;
    }
    assert_eq!(
        codes.iter().filter(|c| **c == "W3010").count(),
        1,
        "{codes:?}"
    );
}

#[test]
fn domain_without_reset_field_uses_the_default_sync_reset() {
    let src = two_clock(
        "domain Fast { clock = posedge }\ndomain Slow { clock = posedge }\n",
        "",
    );
    assert_eq!(rdc_codes(&src), ["W3010"]);
}

// ═══ Ham port bağlaması ═══════════════════════════════════════════

#[test]
fn one_unannotated_raw_port_feeds_every_domain() {
    let src = two_clock(ASYNC_TWO, "    in rst_n : reset(async, active_low)\n");
    assert!(rdc_codes(&src).is_empty(), "{:?}", rdc_codes(&src));
}

#[test]
fn raw_port_without_kind_takes_the_domain_kind() {
    let src = two_clock(ASYNC_TWO, "    in rst_n : reset\n");
    assert!(rdc_codes(&src).is_empty(), "{:?}", rdc_codes(&src));
}

#[test]
fn raw_port_without_kind_feeding_two_different_kinds_is_e3003() {
    let src = two_clock(
        "domain Fast { clock = posedge, reset = async active_low }\n\
         domain Slow { clock = posedge, reset = sync active_high }\n",
        "    in r : reset\n",
    );
    assert_eq!(rdc_codes(&src), ["E3003"]);
}

#[test]
fn raw_port_kind_mismatch_names_both_kinds() {
    let src = two_clock(ASYNC_TWO, "    in rst_n : reset(async, active_high)\n");
    let d = diags(&src);
    let e: Vec<_> = d.iter().filter(|d| d.code.as_str() == "E3003").collect();
    // İki alan aynı türde: tek bildirim satırı başına bir kez... iki ayrı
    // alan bildirimi olduğu için iki tanı.
    assert_eq!(
        e.len(),
        2,
        "{:?}",
        e.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    assert!(
        e[0].message.contains("reset(async, active_high)"),
        "{}",
        e[0].message
    );
    assert!(
        e[0].message.contains("reset(async, active_low)"),
        "{}",
        e[0].message
    );
}

#[test]
fn annotated_raw_ports_feed_only_their_domain() {
    let src = two_clock(
        ASYNC_TWO,
        "    in fr : reset(async, active_low) @Fast\n    in sr : reset(async, active_low) @Slow\n",
    );
    assert!(rdc_codes(&src).is_empty(), "{:?}", rdc_codes(&src));
}

#[test]
fn raw_port_named_like_an_unfed_auto_port_is_e3003() {
    let src = two_clock(ASYNC_TWO, "    in rst_n : reset(async, active_low) @Fast\n");
    let codes = rdc_codes(&src);
    assert!(codes.contains(&"E3003"), "{codes:?}");
}

#[test]
fn raw_port_with_another_name_leaves_the_unfed_domain_on_its_auto_port() {
    let src = two_clock(
        ASYNC_TWO,
        "    in ext_rst_n : reset(async, active_low) @Fast\n",
    );
    // Slow kendi otomatik portunda: kökte W3009, paylaşım yok.
    assert_eq!(rdc_codes(&src), ["W3009"]);
}

#[test]
fn two_unannotated_raw_ports_are_each_e3010() {
    let src = two_clock(
        ASYNC_TWO,
        "    in ra_n : reset(async, active_low)\n    in rb_n : reset(async, active_low)\n",
    );
    assert_eq!(rdc_codes(&src), ["E3010", "E3010"]);
}

#[test]
fn raw_port_in_a_multi_clock_module_is_not_an_ambiguous_signal() {
    // Veri portu olsaydı K3 gereği E3010 olurdu (saat alanı belirsiz).
    let src = two_clock(ASYNC_TWO, "    in rst_n : reset(async, active_low)\n");
    assert!(!rdc_codes(&src).contains(&"E3010"));
}

#[test]
fn raw_port_is_not_reported_as_unused_input() {
    let src = "domain D { clock = posedge, reset = async active_low }\nmodule M {\n\
               in clk : clock @D\n    in rst_n : reset(async, active_low)\n    out q : u8\n\
               reg r : u8 = 0\n    on clk { r <= r + 1 }\n    q = r\n}\n";
    let all: Vec<_> = diags(src).iter().map(|d| d.code.as_str()).collect();
    assert!(all.is_empty(), "{all:?}");
}

#[test]
fn raw_port_annotation_must_name_a_domain() {
    let src = "module M {\n    in clk : clock\n    in r : reset @Nope\n    out q : u8\n\
               reg r2 : u8 = 0\n    on clk { r2 <= r2 + 1 }\n    q = r2\n}\n";
    let codes: Vec<_> = diags(src).iter().map(|d| d.code.as_str()).collect();
    assert!(codes.contains(&"E3002"), "{codes:?}");
}

// ═══ R1 — birim kökü sözleşmesi (W3009) ═══════════════════════════

const CORE_ASYNC: &str = "domain Core { clock = posedge, reset = async active_low }\n";
const CHILD: &str = "module Child {\n    in clk : clock @Core\n    in d : u8\n    out q : u8\n\
                     reg r : u8 = 0\n    on clk { r <= d }\n    q = r\n}\n";

#[test]
fn root_with_async_domain_warns_once() {
    let src = format!(
        "{CORE_ASYNC}module Top {{\n    in clk : clock @Core\n    in clk2 : clock @Core\n\
         out q : u8\n    reg(clk) r : u8 = 0\n    on clk {{ r <= r + 1 }}\n    q = r\n}}\n"
    );
    let d = diags(&src);
    let w: Vec<_> = d.iter().filter(|d| d.code.as_str() == "W3009").collect();
    assert_eq!(w.len(), 1);
    assert!(w[0]
        .help
        .as_deref()
        .unwrap_or_default()
        .contains("in rst_n : reset(async, active_low)"));
}

#[test]
fn instantiated_module_does_not_warn_only_the_root_does() {
    let src = format!(
        "{CORE_ASYNC}{CHILD}module Top {{\n    in clk : clock @Core\n    in d : u8\n    out q : u8\n\
         let u = Child {{ clk: clk, d: d }}\n    q = u.q\n}}\n"
    );
    let d = diags(&src);
    let w: Vec<_> = d.iter().filter(|d| d.code.as_str() == "W3009").collect();
    assert_eq!(w.len(), 1, "yalnız Top");
    assert!(w[0].notes.iter().any(|n| n.text.contains("'Top'")));
}

#[test]
fn root_with_a_raw_port_is_silent() {
    let src = format!(
        "{CORE_ASYNC}{CHILD}module Top {{\n    in clk : clock @Core\n\
         in rst_n : reset(async, active_low)\n    in d : u8\n    out q : u8\n\
         let u = Child {{ clk: clk, d: d }}\n    q = u.q\n}}\n"
    );
    assert!(rdc_codes(&src).is_empty(), "{:?}", rdc_codes(&src));
}

#[test]
fn sync_root_is_silent() {
    let src = "module Top {\n    in clk : clock\n    out q : u8\n    reg r : u8 = 0\n\
               on clk { r <= r + 1 }\n    q = r\n}\n";
    assert!(rdc_codes(src).is_empty(), "{:?}", rdc_codes(src));
}

// ═══ R6 — yakınsama (E3003) ═══════════════════════════════════════

const RAW_CHILD: &str = "module RawChild {\n    in clk : clock @Core\n    in rst_n : reset(async, active_low)\n\
                         in d : u8\n    out q : u8\n    reg r : u8 = 0\n    on clk { r <= d }\n    q = r\n}\n";

fn top_with(body: &str) -> String {
    format!(
        "{CORE_ASYNC}{RAW_CHILD}module Top {{\n    in clk : clock @Core\n    in clk2 : clock @Core\n\
         in rst_n : reset(async, active_low)\n    in d : u8 @Core\n    out q : u8 @Core\n\
         out q2 : u8 @Core\n{body}\n}}\n"
    )
}

#[test]
fn parent_and_child_synchronizing_on_one_clock_is_e3003() {
    let src = top_with(
        "    reg(clk) r : u8 = 0\n    on clk { r <= d }\n    q = r\n\
         let u = RawChild { clk: clk, rst_n: rst_n, d: d }\n    q2 = u.q",
    );
    let d = diags(&src);
    let e: Vec<_> = d.iter().filter(|d| d.code.as_str() == "E3003").collect();
    assert_eq!(
        e.len(),
        1,
        "{:?}",
        d.iter().map(|d| d.code.as_str()).collect::<Vec<_>>()
    );
    assert!(
        e[0].message.contains("synchronized twice on 'clk'"),
        "{}",
        e[0].message
    );
    assert!(e[0].message.contains("instance 'u'"), "{}", e[0].message);
}

#[test]
fn two_siblings_synchronizing_on_one_clock_is_e3003() {
    let src = top_with(
        "    let u = RawChild { clk: clk, rst_n: rst_n, d: d }\n\
         let v = RawChild { clk: clk, rst_n: rst_n, d: d }\n    q = u.q\n    q2 = v.q",
    );
    assert_eq!(rdc_codes(&src), ["E3003"]);
}

#[test]
fn siblings_on_different_clocks_do_not_converge() {
    let src = top_with(
        "    let u = RawChild { clk: clk, rst_n: rst_n, d: d }\n\
         let v = RawChild { clk: clk2, rst_n: rst_n, d: d }\n    q = u.q\n    q2 = v.q",
    );
    assert!(rdc_codes(&src).is_empty(), "{:?}", rdc_codes(&src));
}

#[test]
fn convergence_through_a_pass_through_level_is_found_at_the_top() {
    let mid = "module Mid {\n    in clk : clock @Core\n    in rst_n : reset(async, active_low)\n\
               in d : u8\n    out q : u8\n    let w = RawChild { clk: clk, rst_n: rst_n, d: d }\n\
               q = w.q\n}\n";
    let src = format!(
        "{CORE_ASYNC}{RAW_CHILD}{mid}module Top {{\n    in clk : clock @Core\n\
         in rst_n : reset(async, active_low)\n    in d : u8\n    out q : u8\n    out q2 : u8\n\
         reg(clk) r : u8 = 0\n    on clk {{ r <= d }}\n    q = r\n\
         let m = Mid {{ clk: clk, rst_n: rst_n, d: d }}\n    q2 = m.q\n}}\n"
    );
    let d = diags(&src);
    let e: Vec<_> = d.iter().filter(|d| d.code.as_str() == "E3003").collect();
    assert_eq!(e.len(), 1);
    assert!(e[0].message.contains("instance 'm'"), "{}", e[0].message);
}

#[test]
fn shorthand_bindings_are_followed() {
    let src = top_with(
        "    reg(clk) r : u8 = 0\n    on clk { r <= d }\n    q = r\n\
         let u = RawChild { clk, rst_n, d }\n    q2 = u.q",
    );
    assert_eq!(rdc_codes(&src), ["E3003"]);
}

#[test]
fn data_signal_cannot_drive_a_raw_reset_port() {
    // R4 yapısal olarak imkânsız (ADR-0065): ham porta veri bağlanamaz.
    let src = format!(
        "{CORE_ASYNC}{RAW_CHILD}module Top {{\n    in clk : clock @Core\n    in flag : bool\n\
         in d : u8\n    out q : u8\n    let u = RawChild {{ clk: clk, rst_n: flag, d: d }}\n    q = u.q\n}}\n"
    );
    let codes: Vec<_> = diags(&src).iter().map(|d| d.code.as_str()).collect();
    assert!(codes.contains(&"E2003"), "{codes:?}");
}

// ═══ Reset taşımayan bağlamalar (ADR-0065 R5' inceltmesi) ═════════

/// `rdc_codes` + kaynakta RDC dışı hata olmadığı güvencesi (yeni testler
/// yalnız RDC kodlarına baktığı için gizli tip hatası sonucu bozmasın).
fn checked_rdc_codes(src: &str) -> Vec<&'static str> {
    let other: Vec<_> = diags(src)
        .iter()
        .map(|d| d.code.as_str())
        .filter(|c| c.starts_with('E') && !matches!(*c, "E3003" | "E3010"))
        .collect();
    assert!(other.is_empty(), "RDC dışı hata: {other:?}");
    rdc_codes(src)
}

/// `AsyncDualPortRam` yazma tarafı `fast_clk`'te, okuma `slow_clk`'te;
/// `extra` gövdeye eklenir.
fn ram_bridge(extra: &str) -> String {
    format!(
        "{SYNC_TWO}module M {{\n    in fast_clk : clock @Fast\n    in slow_clk : clock @Slow\n\
         in wa : bits<4> @Fast\n    in wd : u8 @Fast\n    in ra : bits<4> @Slow\n\
         out rd : u8 @Slow\n    out qa : u8 @Fast\n\
         let m = AsyncDualPortRam<u8, 16> {{ wr_clk: fast_clk, wr_addr: wa, wr_data: wd, \
         wr_en: true, rd_clk: slow_clk, rd_addr: ra }}\n    rd = m.rd_data\n{extra}}}\n"
    )
}

#[test]
fn clock_feeding_only_a_ram_write_side_samples_no_reset() {
    // Yazma tarafı yalnız bellek dizisidir (reset'siz, ADR-0049): `rst`'yi
    // yalnız okuma register'ı örnekler — paylaşım yok.
    let src = ram_bridge("    qa = 0\n");
    assert!(
        checked_rdc_codes(&src).is_empty(),
        "{:?}",
        checked_rdc_codes(&src)
    );
}

#[test]
fn ram_write_clock_with_its_own_register_still_shares_the_reset() {
    let src = ram_bridge("    reg(fast_clk) r : u8 = 0\n    on fast_clk { r <= wd }\n    qa = r\n");
    assert_eq!(checked_rdc_codes(&src), ["W3010"]);
}

#[test]
fn ram_write_clock_synchronizing_a_bit_samples_the_reset() {
    // `sync(_, fast_clk)` aşamaları fast alanının reset'iyle sıfırlanır.
    let src = ram_bridge("    qa = 0\n    wire f : bool\n    f = sync(rd[0], fast_clk)\n");
    assert_eq!(checked_rdc_codes(&src), ["W3010"]);
}

const EXT: &str = "extern module Ext {\n    in wr_clk : clock @Src\n    in d : u8 @Src\n\
                   in rd_clk : clock @Dst\n    out q : u8 @Dst\n}\n";

#[test]
fn clocks_bound_only_to_an_extern_sample_no_reset() {
    // Extern'in reset portu yok (ADR-0065 durum tespiti): Volt'un `rst`'si
    // oraya ulaşmaz, iki saat de reset örneklemez.
    let src = format!(
        "{SYNC_TWO}{EXT}module M {{\n    in fast_clk : clock @Fast\n    in slow_clk : clock @Slow\n\
         in d : u8 @Fast\n    out q : u8 @Slow\n\
         let e = Ext {{ wr_clk: fast_clk, d: d, rd_clk: slow_clk }}\n    q = e.q\n}}\n"
    );
    assert!(
        checked_rdc_codes(&src).is_empty(),
        "{:?}",
        checked_rdc_codes(&src)
    );
}

#[test]
fn extern_module_next_to_flops_on_both_clocks_still_shares_the_reset() {
    let src = format!(
        "{SYNC_TWO}{EXT}module M {{\n    in fast_clk : clock @Fast\n    in slow_clk : clock @Slow\n\
         in d : u8 @Fast\n    out q : u8 @Slow\n    out qa : u8 @Fast\n    out qb : u8 @Slow\n\
         reg(fast_clk) ra : u8 = 0\n    reg(slow_clk) rb : u8 = 0\n\
         on fast_clk {{ ra <= d }}\n    on slow_clk {{ rb <= rb + 1 }}\n    qa = ra\n    qb = rb\n\
         let e = Ext {{ wr_clk: fast_clk, d: d, rd_clk: slow_clk }}\n    q = e.q\n}}\n"
    );
    assert_eq!(checked_rdc_codes(&src), ["W3010"]);
}

#[test]
fn parent_chain_feeding_only_a_ram_write_side_does_not_converge() {
    // Top'un `clk` zinciri yalnız reset'siz yazma tarafına gider (ölü
    // mantık); aynı ham reset'i `clk`'te senkronlayan tek zincir çocuğunki.
    let src = format!(
        "{CORE_ASYNC}domain Rd {{ clock = posedge, reset = async active_low }}\n{RAW_CHILD}\
         module Top {{\n    in clk : clock @Core\n    in rd_clk : clock @Rd\n\
         in rst_n : reset(async, active_low)\n    in d : u8 @Core\n    in wa : bits<4> @Core\n\
         in ra : bits<4> @Rd\n    out q : u8 @Core\n    out rd : u8 @Rd\n\
         let m = AsyncDualPortRam<u8, 16> {{ wr_clk: clk, wr_addr: wa, wr_data: d, wr_en: true, \
         rd_clk: rd_clk, rd_addr: ra }}\n    rd = m.rd_data\n\
         let u = RawChild {{ clk: clk, rst_n: rst_n, d: d }}\n    q = u.q\n}}\n"
    );
    assert!(
        checked_rdc_codes(&src).is_empty(),
        "{:?}",
        checked_rdc_codes(&src)
    );
}

// ═══ ADR-0065 R5' yükseltme kararı: W3010 uyarı kalır ════════════

const DUAL: &str = "module Dual {\n    in sys_clk : clock @Fast\n    in slow_clk : clock @Slow\n\
                    RAW    in a : u8 @Fast\n    out qa : u8 @Fast\n    out qb : u8 @Slow\n\
                    reg(sys_clk) ra : u8 = 0\n    reg(slow_clk) rb : u8 = 0\n\
                    on sys_clk { ra <= a }\n    on slow_clk { rb <= rb + 1 }\n    qa = ra\n    qb = rb\n}\n";

fn dual_under_parent(child_raw: bool) -> String {
    let (raw, bind) = if child_raw {
        ("in rst : reset(sync, active_high)\n", ", rst: rst")
    } else {
        ("", "")
    };
    format!(
        "{SYNC_TWO}{}module Top {{\n    in fast_clk : clock @Fast\n    in slow_clk : clock @Slow\n\
         in rst : reset(sync, active_high)\n    in a : u8 @Fast\n    out qa : u8 @Fast\n\
         out qb : u8 @Slow\n    out ta : u8 @Fast\n    out tb : u8 @Slow\n\
         reg(fast_clk) ra : u8 = 0\n    reg(slow_clk) rb : u8 = 0\n\
         on fast_clk {{ ra <= a }}\n    on slow_clk {{ rb <= rb + 1 }}\n    ta = ra\n    tb = rb\n\
         let u = Dual {{ sys_clk: fast_clk, slow_clk: slow_clk, a: a{bind} }}\n\
         qa = u.qa\n    qb = u.qb\n}}\n",
        DUAL.replace("RAW", raw)
    )
}

#[test]
fn multi_domain_child_under_a_synchronizing_parent_has_no_error_free_form() {
    // İki alanda da reset'li flop'u olan çocuk, aynı saatlerde kendi flop'u
    // olan ebeveynin altında: otomatik portla W3010 (çocukta), ham portla
    // R6 yakınsaması (her saat için E3003). W3010 hata olsaydı bu yaygın
    // yapı yazılamazdı — ADR-0065 "Aşama 5" kararının ölçümü.
    assert_eq!(checked_rdc_codes(&dual_under_parent(false)), ["W3010"]);
    assert_eq!(
        checked_rdc_codes(&dual_under_parent(true)),
        ["E3003", "E3003"]
    );
}

// ═══ Belirlenimcilik ══════════════════════════════════════════════

#[test]
fn rdc_diagnostics_are_identical_across_runs() {
    let src = format!(
        "{}{}",
        two_clock(
            ASYNC_TWO,
            "    in ra_n : reset(async, active_low)\n    in rb_n : reset\n"
        ),
        top_with(
            "    let u = RawChild { clk: clk, rst_n: rst_n, d: d }\n\
             let v = RawChild { clk: clk, rst_n: rst_n, d: d }\n    q = u.q\n    q2 = v.q"
        )
        .replace(
            CORE_ASYNC,
            "domain Core { clock = posedge, reset = async active_low }\n"
        )
    );
    let first: Vec<String> = diags(&src).iter().map(|d| format!("{d:?}")).collect();
    for _ in 0..5 {
        let again: Vec<String> = diags(&src).iter().map(|d| format!("{d:?}")).collect();
        assert_eq!(first, again);
    }
}
