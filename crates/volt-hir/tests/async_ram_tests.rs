//! AsyncDualPortRam (ADR-0049): domain-aware çift saatli bellek —
//! isim çözümleme, tip kontrolü, K8 alan denetimi ve W3006 kullanım
//! kısıtı. Yazma portu Src (wr_clk), okuma portu Dst (rd_clk) rolünde;
//! adresler kendi alanlarında kalır, bellek dizisi CDC sınırıdır.

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

const TWO_DOMAINS: &str = "domain Fast { clock = posedge, reset = sync active_high }\n\
                           domain Slow { clock = posedge, reset = sync active_high }\n";

/// Yazma tarafı Fast, okuma tarafı Slow olan temel modül; `inst`
/// örnekleme satırı, `extra` ek port bildirimleri.
fn ram_module(inst: &str, extra_ports: &str, out_domain: &str) -> String {
    format!(
        "{TWO_DOMAINS}module M {{\n    in  fast_clk : clock @Fast\n    \
         in  slow_clk : clock @Slow\n    in  wa : bits<4> @Fast\n    \
         in  wd : u8 @Fast\n    in  we : bool @Fast\n    \
         in  ra : bits<4> @Slow\n{extra_ports}    out rd : u8 @{out_domain}\n\n\
         {inst}\n\n    rd = m.rd_data\n}}\n"
    )
}

const RAM_OK: &str = "    let m = AsyncDualPortRam<u8, 16> { wr_clk: fast_clk, wr_addr: wa, \
                      wr_data: wd, wr_en: we, rd_clk: slow_clk, rd_addr: ra }";

// ═══ Temiz geçmesi gerekenler ═════════════════════════════════════

#[test]
fn async_dual_port_ram_resolves_and_typechecks_clean() {
    let result = check(&ram_module(RAM_OK, "", "Slow"));
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn async_dual_port_ram_warns_w3006_on_every_instance() {
    // Aynı adrese diğer saatten yazılırken okunan değer TANIMSIZ; adres
    // çakışması statik bilinemediğinden her örneklemede hatırlatılır.
    let result = check(&ram_module(RAM_OK, "", "Slow"));
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "W3006")
        .expect("W3006 bekleniyor");
    assert!(
        diag.message.contains("AsyncDualPortRam"),
        "mesaj primitifi adlandırmalı: {}",
        diag.message
    );
    assert!(
        diag.message.contains("undefined"),
        "mesaj tanımsız okumayı söylemeli: {}",
        diag.message
    );
    diag.validate().expect("5 parça kuralı");
}

#[test]
fn w3006_for_async_ram_is_not_the_port_b_wins_text() {
    // DualPortRam'in "B portu kazanır" metni çift saatli bellekte
    // yanlış olurdu: burada iki YAZICI değil, yazıcı + okuyucu çakışır.
    let result = check(&ram_module(RAM_OK, "", "Slow"));
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "W3006")
        .expect("W3006 bekleniyor");
    assert!(
        !diag.message.contains("port B"),
        "çift saatli metin ayrı olmalı: {}",
        diag.message
    );
}

#[test]
fn same_clock_on_both_sides_is_accepted() {
    // Tek alanda kullanım hata değildir (DualPortRam tercih edilmeli
    // ama derleyici bunu zorlamaz).
    let inst = "    let m = AsyncDualPortRam<u8, 16> { wr_clk: fast_clk, wr_addr: wa, \
                wr_data: wd, wr_en: we, rd_clk: fast_clk, rd_addr: wa }";
    let src = format!(
        "{TWO_DOMAINS}module M {{\n    in  fast_clk : clock @Fast\n    \
         in  slow_clk : clock @Slow\n    in  wa : bits<4> @Fast\n    in  wd : u8 @Fast\n    \
         in  we : bool @Fast\n    out rd : u8 @Fast\n    out sq : bool @Slow\n\n{inst}\n\n    \
         rd = m.rd_data\n    sq = slow_clk == slow_clk\n}}\n"
    );
    let result = check(&src);
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

// ═══ K8 — port alanları saat bağlamasından gelir ══════════════════

#[test]
fn wr_addr_from_read_domain_is_e3001() {
    let inst = "    let m = AsyncDualPortRam<u8, 16> { wr_clk: fast_clk, wr_addr: ra, \
                wr_data: wd, wr_en: we, rd_clk: slow_clk, rd_addr: ra }";
    let c = codes(&ram_module(inst, "", "Slow"));
    assert!(c.contains(&"E3001"), "E3001 bekleniyor: {c:?}");
}

#[test]
fn rd_addr_from_write_domain_is_e3001() {
    let inst = "    let m = AsyncDualPortRam<u8, 16> { wr_clk: fast_clk, wr_addr: wa, \
                wr_data: wd, wr_en: we, rd_clk: slow_clk, rd_addr: wa }";
    let c = codes(&ram_module(inst, "", "Slow"));
    assert!(c.contains(&"E3001"), "E3001 bekleniyor: {c:?}");
}

#[test]
fn wr_data_from_read_domain_is_e3001() {
    let inst = "    let m = AsyncDualPortRam<u8, 16> { wr_clk: fast_clk, wr_addr: wa, \
                wr_data: sd, wr_en: we, rd_clk: slow_clk, rd_addr: ra }";
    let c = codes(&ram_module(inst, "    in  sd : u8 @Slow\n", "Slow"));
    assert!(c.contains(&"E3001"), "E3001 bekleniyor: {c:?}");
}

#[test]
fn wr_en_from_read_domain_is_e3001() {
    let inst = "    let m = AsyncDualPortRam<u8, 16> { wr_clk: fast_clk, wr_addr: wa, \
                wr_data: wd, wr_en: se, rd_clk: slow_clk, rd_addr: ra }";
    let c = codes(&ram_module(inst, "    in  se : bool @Slow\n", "Slow"));
    assert!(c.contains(&"E3001"), "E3001 bekleniyor: {c:?}");
}

#[test]
fn rd_data_output_carries_read_domain() {
    // rd_data hedef (Slow) alanındadır; Fast çıkışa atamak E3001.
    let c = codes(&ram_module(RAM_OK, "", "Fast"));
    assert!(c.contains(&"E3001"), "E3001 bekleniyor: {c:?}");
}

#[test]
fn e3001_on_binding_points_at_the_binding_line() {
    let inst = "    let m = AsyncDualPortRam<u8, 16> { wr_clk: fast_clk, wr_addr: wa, \
                wr_data: wd, wr_en: we, rd_clk: slow_clk, rd_addr: wa }";
    let result = check(&ram_module(inst, "", "Slow"));
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E3001")
        .expect("E3001 bekleniyor");
    diag.validate().expect("5 parça kuralı");
    assert!(diag.primary_span().is_some(), "konum taşımalı");
}

// ═══ Generic / port doğrulaması ═══════════════════════════════════

#[test]
fn depth_not_power_of_two_is_e2025() {
    let inst = "    let m = AsyncDualPortRam<u8, 12> { wr_clk: fast_clk, wr_addr: wa, \
                wr_data: wd, wr_en: we, rd_clk: slow_clk, rd_addr: ra }";
    let c = codes(&ram_module(inst, "", "Slow"));
    assert!(c.contains(&"E2025"), "E2025 bekleniyor: {c:?}");
}

#[test]
fn addr_width_mismatch_is_type_error() {
    // 16 derinlik 4 bit adres ister; bits<3> bağlamak tip hatası.
    let inst = "    let m = AsyncDualPortRam<u8, 16> { wr_clk: fast_clk, wr_addr: na, \
                wr_data: wd, wr_en: we, rd_clk: slow_clk, rd_addr: ra }";
    let c = codes(&ram_module(inst, "    in  na : bits<3> @Fast\n", "Slow"));
    assert!(
        c.iter().any(|c| c.starts_with("E2")),
        "tip hatası bekleniyor: {c:?}"
    );
}

#[test]
fn unknown_port_is_e1009() {
    // DualPortRam'in a_addr'ı burada yok: iki primitif ayrı port tablosu.
    let inst = "    let m = AsyncDualPortRam<u8, 16> { wr_clk: fast_clk, a_addr: wa, \
                wr_data: wd, wr_en: we, rd_clk: slow_clk, rd_addr: ra }";
    let c = codes(&ram_module(inst, "", "Slow"));
    assert!(c.contains(&"E1009"), "E1009 bekleniyor: {c:?}");
}

#[test]
fn binding_the_rd_data_output_is_e1009() {
    let inst = "    let m = AsyncDualPortRam<u8, 16> { wr_clk: fast_clk, wr_addr: wa, \
                wr_data: wd, wr_en: we, rd_clk: slow_clk, rd_addr: ra, rd_data: wd }";
    let c = codes(&ram_module(inst, "", "Slow"));
    assert!(c.contains(&"E1009"), "E1009 bekleniyor: {c:?}");
}

#[test]
fn dual_port_ram_stays_single_clock() {
    // Regresyon (ADR-0049 "mevcut DualPortRam'e dokunma"): tek saatli
    // primitif wr_clk/rd_clk portu kazanmadı.
    let inst = "    let m = DualPortRam<u8, 16> { wr_clk: fast_clk, a_addr: wa, \
                a_wr_data: wd, a_wr_en: we, b_addr: ra, b_wr_data: wd, b_wr_en: we }";
    let src = format!(
        "{TWO_DOMAINS}module M {{\n    in  fast_clk : clock @Fast\n    \
         in  slow_clk : clock @Slow\n    in  wa : bits<4> @Fast\n    in  wd : u8 @Fast\n    \
         in  we : bool @Fast\n    in  ra : bits<4> @Fast\n    out rd : u8 @Fast\n    \
         out sq : bool @Slow\n\n{inst}\n\n    rd = m.b_rd_data\n    sq = slow_clk == slow_clk\n}}\n"
    );
    let c = codes(&src);
    assert!(c.contains(&"E1009"), "E1009 bekleniyor: {c:?}");
}
