//! fn açılımı (ADR-0081 Karar 12) — emitter'ın doğrudan kullanımı: HIR
//! tanıları olmadan da açılım sınırlıdır (bütçe, döngü), fn tanımı
//! çağrılmadıkça çıktıyı değiştirmez, kiplerin biçimi.

use volt_span::FileId;
use volt_sv_emit::{emit_full, EmitOutput, SvaMode};

fn emit_src(src: &str) -> EmitOutput {
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse hatasız olmalı: {:?}",
        parsed.error_codes()
    );
    emit_full(&parsed.ast, "t.volt", src, SvaMode::None)
}

fn codes(out: &EmitOutput) -> Vec<&'static str> {
    out.diagnostics.iter().map(|d| d.code.as_str()).collect()
}

const MODULE: &str = "module M {\n    in  a : u8\n    out y : u8\n    y = a + 1\n}\n";

/// Karar 12.1: çağrılmayan fn açılım geçidini tetiklemez — çıktı fn'siz
/// birimle byte-aynı (golden ilkesi).
#[test]
fn an_uncalled_function_does_not_change_the_output() {
    let plain = emit_src(MODULE);
    let with_fn = emit_src(&format!("fn inc(a: u8) -> u8 {{ a + 1 }}\n\n{MODULE}"));
    assert!(codes(&with_fn).is_empty(), "{:?}", codes(&with_fn));
    assert_eq!(
        plain.sv.replace("t.volt", ""),
        with_fn.sv.replace("t.volt", "")
    );
}

/// Savunma bütçesi (Karar 11): HIR'ı atlayan emitter kullanımı da üstel
/// çağrı ağacını açmaya kalkmaz; E2027 bir kez.
#[test]
fn the_emitter_stops_an_exponential_call_tree_with_e2027() {
    let mut src = String::from("fn f0(a: u8) -> u8 { a + 1 }\n");
    for k in 1..24 {
        src.push_str(&format!(
            "fn f{k}(a: u8) -> u8 {{ f{}(a) ^ f{}(~a) }}\n",
            k - 1,
            k - 1
        ));
    }
    src.push_str("module M {\n    in  a : u8\n    out y : u8\n    y = f23(a)\n}\n");
    let start = std::time::Instant::now();
    let out = emit_src(&src);
    assert!(start.elapsed().as_secs() < 20, "bütçe açılımı sınırlamalı");
    assert_eq!(
        codes(&out).iter().filter(|c| **c == "E2027").count(),
        1,
        "{:?}",
        codes(&out)
    );
}

/// Döngü savunması: özyineli fn (HIR'da E4013) emitter'ı sonsuz açılıma
/// sokmaz.
#[test]
fn a_recursive_call_is_not_expanded_forever() {
    let src = "fn f(a: u8) -> u8 { g(a) + 1 }\nfn g(a: u8) -> u8 { f(a) }\nmodule M {\n    in  a : u8\n    out y : u8\n    y = f(a)\n}\n";
    let out = emit_src(src);
    assert!(!out.sv.contains("function"), "{}", out.sv);
}

/// Tel kipi: argümanı modül düzeyi olan `on` çağrısı always_ff'ten önce
/// koşulsuz bir tel alır.
#[test]
fn an_on_block_call_gets_a_wire_before_always_ff() {
    let src = "fn inc(a: u8) -> u8 { a + 1 }\nmodule M {\n    in  clk : clock\n    in  en  : bool\n    out y   : u8\n    reg r : u8 = 0\n    on clk {\n        if en { r <= inc(r) }\n    }\n    y = r\n}\n";
    let out = emit_src(src);
    assert!(codes(&out).is_empty(), "{:?}", codes(&out));
    let wire = out.sv.find("wire [7:0] inc_0 = r + 8'd1;").expect("tel");
    let ff = out.sv.find("always_ff").expect("always_ff");
    assert!(wire < ff, "{}", out.sv);
    assert!(out.sv.contains("r <= inc_0;"), "{}", out.sv);
}

/// İkame kipi: tipi bilinmeyen argüman parametre genişliğinde (`W'(e)`),
/// bağlama duyarlı sonuç dönüş genişliğinde hesaplanır.
#[test]
fn substitution_keeps_parameter_and_result_widths() {
    let src = "fn add(a: u8, b: u8) -> u8 { a + b }\nmodule M {\n    in  x : u8\n    in  z : u8\n    out y : u16\n    comb {\n        y = add(x + 1, z) as u16\n    }\n}\n";
    let out = emit_src(src);
    assert!(codes(&out).is_empty(), "{:?}", codes(&out));
    assert!(out.sv.contains("8'(8'(x + 8'd1) + z)"), "{}", out.sv);
}
