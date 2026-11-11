//! Kaskad bastırma penceresi (error-recovery.md §5) yalnız hatalar
//! içindir (#206). Pencere bir hatanın hemen ardındaki, aynı bozuk
//! bölgeden doğan hataları bastırır; bir uyarı pencere açmaz, ayrıştırma
//! bittikten sonra koşan geçitler (`@mmio` desugar'ı, pipeline desugar'ı)
//! pencereyi hiç kullanmaz.
//!
//! Önce `push_error` uyarılar için de `last_error_pos`'u kuruyordu: bir
//! uyarıdan bir-iki token sonraki hata yutuluyor, dosyanın son deyimindeki
//! bir uyarı da desugar hatalarının hepsini yutuyordu. Kaynak çıkış 0 ile
//! kabul ediliyordu. Beklenen kodlar şartnameden, elle.

use volt_span::FileId;
use volt_syntax::parser::{parse, ParseResult};

fn p(src: &str) -> ParseResult {
    parse(FileId(0), src)
}

fn count(r: &ParseResult, code: &str) -> usize {
    r.error_codes().iter().filter(|c| **c == code).count()
}

// ─── Uyarı, ardından hata ───

/// W0010 ile fazladan ')' aynı tokendadır: uyarı `c`'den sonra, ')'
/// konumunda üretilir; ')' bir deyim başlatamaz (E0001).
#[test]
fn w0010_then_error_on_the_same_token() {
    let r = p(
        "module W {\n in a : u8\n in b : u8\n in c : u8\n out r : u8\n \
               r = a & b | c )\n}\n",
    );
    assert_eq!(count(&r, "W0010"), 1, "{:?}", r.error_codes());
    assert_eq!(count(&r, "E0001"), 1, "{:?}", r.error_codes());
}

/// W0010 `invariant` tokenında üretilir; eksik ':' bir token sonra, `cnt`
/// konumundadır (E0001, ContractKind ":" Expr).
#[test]
fn w0010_then_error_one_token_later() {
    let r = p(
        "module A {\n in clk : clock\n in a : u8\n in b : u8\n in c : u8\n \
               out r : u8\n reg cnt : u8 = 0\n r = a & b | c\n invariant cnt < 200\n \
               on clk { cnt <= cnt + 1 }\n}\n",
    );
    assert_eq!(count(&r, "W0010"), 1, "{:?}", r.error_codes());
    assert_eq!(count(&r, "E0001"), 1, "{:?}", r.error_codes());
}

/// W0020 nitelik adında üretilir; eksik modül adı (`{`) iki token sonra.
#[test]
fn w0020_then_error_on_the_next_tokens() {
    let r = p("@bogus module {\n in a : u8\n out r : u8\n r = a\n}\n");
    assert_eq!(count(&r, "W0020"), 1, "{:?}", r.error_codes());
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.severity == volt_diagnostics::Severity::Error),
        "eksik modül adı hata olmalı: {:?}",
        r.error_codes()
    );
}

// ─── Son deyimde uyarı, ardından desugar hatası ───

const MMIO_TAIL: &str = "@mmio(base = 0x4000_0000, bus = AXI4Lite)\nmodule M {\n \
    in clk : clock\n in a : u8\n in b : u8\n in c : u8\n out o : u8\n \
    @reg(offset = 0x00, access = ReadWrite)\n ctrl : { en : bool, @reserved : bits<31>, }\n \
    @reg(offset = 0x04, access = ReadWrite)\n wide : { v : bits<40>, }\n \
    @reg(offset = 0x0A, access = ReadWrite)\n odd : { v : bits<8>, }\n \
    o = a & b | c\n}\n";

/// Dosyanın son deyiminde W0010; `@mmio` desugar'ı dosya bitince koşar
/// ve iki ayrı register için iki E0015 verir (ADR-0044: desteklenmeyen
/// alan tipi, hizasız offset).
#[test]
fn w0010_in_the_last_statement_then_mmio_desugar_errors() {
    let r = p(MMIO_TAIL);
    assert_eq!(count(&r, "W0010"), 1, "{:?}", r.error_codes());
    assert_eq!(count(&r, "E0015"), 2, "{:?}", r.error_codes());
}

/// Uyarısız da aynı: desugar tanılarının hepsi dosya sonu konumundadır;
/// ilki ikincisini yutmamalı.
#[test]
fn mmio_desugar_errors_at_the_same_position_are_all_reported() {
    let r = p(&MMIO_TAIL.replace("a & b | c", "(a & b) | c"));
    assert_eq!(count(&r, "W0010"), 0, "{:?}", r.error_codes());
    assert_eq!(count(&r, "E0015"), 2, "{:?}", r.error_codes());
}

const PIPELINE_TAIL: &str = "pipeline(3) P {\n in clk : clock\n in x : u32\n in hold : bool\n \
    in a : u8\n in b : u8\n in c : u8\n out y : u32\n out o : u8\n \
    stage S0 { let p : u32 = x }\n stage S1 { let q : u32 = p + 1 }\n \
    stage S2 { let w : u32 = q + 1 }\n stall S1 when hold\n \
    y = stage(S2).w\n o = a & b | c\n}\n";

/// Pipeline'ın son deyiminde W0010; pipeline desugar'ı kapanış '}'
/// okununca koşar. S0'sız S1 durdurmak önek değildir (ADR-0038 §4): E5013.
#[test]
fn w0010_in_the_last_pipeline_statement_then_desugar_error() {
    let r = p(PIPELINE_TAIL);
    assert_eq!(count(&r, "W0010"), 1, "{:?}", r.error_codes());
    assert_eq!(count(&r, "E5013"), 1, "{:?}", r.error_codes());
}

// ─── Pencere hatalar için çalışmaya devam eder ───

/// Hatadan hemen sonraki, aynı bozuk bölgeden doğan hatalar hâlâ
/// bastırılır (error-recovery.md §5): düzeltme pencereyi kapatmaz.
#[test]
fn errors_still_suppress_their_cascade() {
    let r = p("module M { in in in in in in in in : : : : }");
    assert!(
        r.diagnostics.len() <= 5,
        "kaskad bastırma çalışmıyor: {:?}",
        r.error_codes()
    );
}
