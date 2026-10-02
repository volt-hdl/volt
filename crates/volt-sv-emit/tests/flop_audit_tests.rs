//! Üreticide varsayım yok (ADR-0098 eki 2): SV üretimi her flop'un saat
//! kenarını ve reset dalını kaynaktan türetir; türetemediği girdiyi
//! üretimden önce reddeder (flop denetimi). Bu testler ön ucu (parser
//! E0020'den sonraki HIR denetimleri: E3016, çıkış portu E0020) bilerek
//! atlar ve `emit()`'i doğrudan çağırır: denetim olmasaydı bu girdiler
//! `posedge` ya da boş reset dallı flop olurdu.

use volt_span::FileId;
use volt_sv_emit::{emit_full, SvaMode};

/// Ayrıştırır (parser hatasız olmalı), ön ucu atlayıp üretir; üretilen SV
/// ve tanı kodları.
fn emit_unchecked(src: &str, mode: SvaMode) -> (String, Vec<String>) {
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse hatasız olmalı: {:?}",
        parsed.error_codes()
    );
    let out = emit_full(&parsed.ast, "test.volt", src, mode);
    let codes = out
        .diagnostics
        .iter()
        .map(|d| d.code.as_str().to_string())
        .collect();
    (out.sv, codes)
}

fn messages(src: &str, mode: SvaMode) -> Vec<String> {
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    emit_full(&parsed.ast, "test.volt", src, mode)
        .diagnostics
        .iter()
        .map(|d| {
            let notes: Vec<&str> = d.notes.iter().map(|n| n.text.as_str()).collect();
            format!(
                "{} {} | {} | {}",
                d.code.as_str(),
                d.message,
                d.help.as_deref().unwrap_or(""),
                notes.join(" / ")
            )
        })
        .collect()
}

const ASYNC: &str = "domain Async { clock = none }\n";

#[test]
fn edgeless_on_block_is_refused_before_generation() {
    let src = format!(
        "{ASYNC}module M {{\n    in clk : clock @Async\n    in d : u8\n    out q : u8\n    \
         reg r : u8 = 0\n    on clk {{ r <= d }}\n    q = r\n}}\n"
    );
    let (sv, codes) = emit_unchecked(&src, SvaMode::None);
    assert_eq!(codes, ["E3016"], "{sv}");
    assert!(!sv.contains("posedge"), "{sv}");
    let msgs = messages(&src, SvaMode::None);
    assert!(msgs[0].contains("compiler bug"), "{msgs:?}");
}

#[test]
fn edgeless_sync_destination_and_capture_are_refused() {
    let dst = format!(
        "{ASYNC}domain Fast {{ clock = posedge }}\nmodule M {{\n    in aclk : clock @Async\n    \
         in fclk : clock @Fast\n    in d : bool @Fast\n    out q : bool @Async\n    q = sync(d, aclk)\n}}\n"
    );
    let (sv, codes) = emit_unchecked(&dst, SvaMode::None);
    assert_eq!(codes, ["E3016"], "{sv}");
    let src = format!(
        "{ASYNC}domain Fast {{ clock = posedge }}\nmodule M {{\n    in aclk : clock @Async\n    \
         in fclk : clock @Fast\n    in d : bool @Async\n    out q : bool @Fast\n    q = sync(d, fclk)\n}}\n"
    );
    let (sv, codes) = emit_unchecked(&src, SvaMode::None);
    assert_eq!(codes, ["E3016"], "{sv}");
}

#[test]
fn edgeless_builtin_clock_is_refused() {
    let src = format!(
        "{ASYNC}module M {{\n    in clk : clock @Async\n    in d : u8\n    in we : bool\n    \
         out q : u8\n    let f = SyncFifo<u8, 4> {{ clk: clk, wr_data: d, wr_en: we, rd_en: true }}\n    \
         q = f.rd_data\n}}\n"
    );
    let (sv, codes) = emit_unchecked(&src, SvaMode::None);
    assert_eq!(codes, ["E3016"], "{sv}");
}

#[test]
fn edgeless_contract_clock_is_refused_only_when_contracts_are_emitted() {
    let src = format!(
        "{ASYNC}module M {{\n    in clk : clock @Async\n    in a : u8\n    out y : u8\n    \
         y = a\n    invariant: y == a\n}}\n"
    );
    // Yalnız RTL: kontrat üretilmez, flop yok.
    let (_, codes) = emit_unchecked(&src, SvaMode::None);
    assert!(codes.is_empty(), "{codes:?}");
    for mode in [SvaMode::Separate, SvaMode::Inline, SvaMode::Immediate] {
        let (sv, codes) = emit_unchecked(&src, mode);
        assert_eq!(codes, ["E3016"], "{mode:?}: {sv}");
    }
}

#[test]
fn output_port_flop_in_a_reset_domain_is_refused() {
    let src = "module M {\n    in clk : clock\n    in d : u8\n    out q : u8\n    \
               on clk { q <= d }\n}\n";
    let (sv, codes) = emit_unchecked(src, SvaMode::None);
    assert_eq!(codes, ["E0020"], "{sv}");
    assert!(!sv.contains("if (rst) begin\n        end"), "{sv}");
}

#[test]
fn wire_flop_in_a_reset_domain_is_refused() {
    // Parser E0020'yi verir; tanısı atılan AST yine üretilmez.
    let src = "module M {\n    in clk : clock\n    in d : u8\n    out q : u8\n    \
               wire w : u8\n    on clk { w <= d }\n    q = w\n}\n";
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    let out = emit_full(&parsed.ast, "test.volt", src, SvaMode::None);
    let codes: Vec<&str> = out.diagnostics.iter().map(|d| d.code.as_str()).collect();
    assert_eq!(codes, ["E0020"], "{}", out.sv);
}

#[test]
fn reset_free_domain_and_registers_are_generated_unchanged() {
    // `reset = none` alanında reset dalı yoktur (kaynakta açıkça
    // reset'siz); register'lar reset değerini bildirimden alır.
    let src = "domain Free { clock = negedge, reset = none }\nmodule M {\n    \
               in clk : clock @Free\n    in d : u8 @Free\n    out q : u8 @Free\n    \
               reg r : u8 @Free = 3\n    on clk { r <= d }\n    q = r\n}\n";
    let (sv, codes) = emit_unchecked(src, SvaMode::None);
    assert!(codes.is_empty(), "{codes:?}");
    assert!(
        sv.contains("always_ff @(negedge clk) begin\n        r <= d;"),
        "{sv}"
    );
}

#[test]
fn clock_annotated_with_another_clock_takes_that_clock_domain() {
    // `@c1` başka bir saat portunun adı: alanı o saatinkidir. Eskiden
    // alan tablosunda bulunamayınca varsayılan (posedge) yazılıyordu.
    let src = "domain Neg { clock = negedge }\nmodule M {\n    in c1 : clock @Neg\n    \
               in c2 : clock @c1\n    in d : u8 @Neg\n    out q : u8 @Neg\n    \
               reg r : u8 @Neg = 0\n    on c2 { r <= d }\n    q = r\n}\n";
    let (sv, codes) = emit_unchecked(src, SvaMode::None);
    assert!(codes.is_empty(), "{codes:?}");
    assert!(sv.contains("always_ff @(negedge c2)"), "{sv}");
}

#[test]
fn clock_annotation_cycle_is_refused() {
    let src = "module M {\n    in c1 : clock @c2\n    in c2 : clock @c1\n    in d : u8 @c1\n    \
               out q : u8 @c1\n    reg r : u8 @c1 = 0\n    on c2 { r <= d }\n    q = r\n}\n";
    let (sv, codes) = emit_unchecked(src, SvaMode::None);
    assert_eq!(codes, ["E3002"], "{sv}");
}

#[test]
fn on_block_triggered_by_a_non_clock_signal_is_refused() {
    // Tetikleyici saat portu değil: alanı kaynaktan türetilemez. Eskiden
    // ilk saatin kenarı ve reset'i alınıyordu.
    let src = "module M {\n    in clk : clock\n    in d : bool\n    out q : u8\n    \
               reg r : u8 = 0\n    on d { r <= 1 }\n    q = r\n}\n";
    let (sv, codes) = emit_unchecked(src, SvaMode::None);
    assert_eq!(codes, ["E3002"], "{sv}");
}
