//! Üreticinin ad çözümü (#128), emitter doğrudan çağrılarak (HIR'sız).
//! Ad çözümünün anlamını volt-driver'ın `name_scope_sim_tests`'i Verilator
//! simülasyonuyla, elle hesaplanmış değerlerle sınar; burada yalnız HIR'ın
//! emitter'a ulaştırmadığı girdiler kalır:
//! - HIR'ın E2031 ile reddettiği aynı adlı esnek `let` (metin testi, ADR-0025
//!   eki: tipsiz `let`'in doğal genişlikte bildirimi);
//! - HIR'ın E2020 ile reddettiği döngüsel const zinciri ve zincirin derinlik
//!   sınırı (tanı testleri: SV metni karşılaştırılmaz).

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

/// Metin testi, dayanağı ADR-0025'in "Tipsiz `let` telinin SV genişliği"
/// eki: tipsiz `let` aralığın üst ucunda, doğal genişlikte bildirilir
/// (`wire [N-1:0]`). Davranış testine çevrilemez: HIR bu girdiyi E2031 ile
/// reddeder, simüle edilecek SV oluşmaz; çözüm emitter doğrudan
/// çağrıldığında da aynı kalmalı.
///
/// Esnek aralık (ADR-0025): gövdedeki `K` const'tur (u16, aralık 16..16);
/// çağıranın aynı adlı esnek `let`'inin (8..9) alt ucu ona sızmaz. `t = K + K`
/// 16..17, `u = t ^ x` (x : u12) kesişmez → doğal genişlik 17. Sızsaydı
/// t 8..17 olurdu ve u 12 bit.
#[test]
fn a_function_const_range_ignores_a_same_named_flexible_let() {
    let src = "const K : u16 = 1000
fn f(x: u12) -> u17 {
    let t = K + K
    let u = t ^ x
    u
}
module Top {
    in  a : u8
    in  b : u8
    in  x : u12
    out y : u17
    out z : u9
    let K = a + b
    y = f(x)
    z = K
}
";
    let out = emit_src(src);
    assert!(out.sv.contains("wire [16:0] f_0_t = "), "{}", out.sv);
    assert!(out.sv.contains("wire [16:0] f_0_u = "), "{}", out.sv);
}

/// Tanı kodları (HIR'sız emitter; döngüsel const'u HIR E2020 ile reddeder).
fn emit_codes(src: &str) -> (Vec<String>, String) {
    let out = emit_src(src);
    let codes = out
        .diagnostics
        .iter()
        .map(|d| d.code.as_str().to_string())
        .collect();
    (codes, out.sv)
}

/// Döngüsel const zinciri (`A → B → A`) emitter'da sonsuz özyinelemeye
/// girmez: derinlik sınırında değerlendirme biter, katlanamayan const
/// E0003 olur. (HIR bu girdiyi E2020 ile emitter'a varmadan reddeder;
/// emitter'ın kendi sınırı yine de yığın taşmasını önler.)
#[test]
fn a_cyclic_const_chain_stops_at_the_depth_limit() {
    let (codes, sv) = emit_codes(
        "const A : u32 = B
const B : u32 = A
module M {
    out o : u32
    o = A
}
",
    );
    assert_eq!(codes, ["E0003"], "{sv}");
}

/// Enum tipli döngüsel const zinciri match'in sınananında: `const_match_key`
/// kendini çağırır; derinlik sınırı orada da denetlenir. Düzeltmeden önce
/// yığın taşıyordu.
#[test]
fn a_cyclic_enum_const_chain_in_a_match_stops_at_the_depth_limit() {
    let (codes, sv) = emit_codes(
        "enum E { X, Y }
const A : E = B
const B : E = A
const N : u8 = match A { E::X => 1, E::Y => 2 }
module M {
    out o : u8
    o = N
}
",
    );
    assert_eq!(codes, ["E0003"], "{sv}");
}

/// `n` halkalı enum const zinciri: `A0 = A1`, …, `A{n-1} = E::Y`.
fn enum_chain(n: usize) -> String {
    let mut src = String::from("enum E { X, Y }\n");
    for j in 0..n {
        let value = if j + 1 == n {
            "E::Y".to_string()
        } else {
            format!("A{}", j + 1)
        };
        src.push_str(&format!("const A{j} : E = {value}\n"));
    }
    src.push_str(
        "const N : u8 = match A0 { E::X => 1, E::Y => 2 }
module M {
    out o : u8
    o = N
}
",
    );
    src
}

/// Derinlik sınırı 64'tür (u32 zinciriyle aynı sınır). Derinlik 0'dan
/// sayılır: N'nin match'i 0, sınanan `A0` 1, `A{j}` j + 1, son halkanın
/// değeri `E::Y` n + 1; 64'ü aşan derinlik değerlendirilmez. n = 63'te
/// `E::Y` 64. derinlikte, tanı yok (değerini volt-driver'ın
/// `name_scope_sim_tests`'i simüle eder: N = 2); n = 64'te 65. derinlikte
/// N katlanamaz: E0003.
#[test]
fn an_enum_const_chain_stops_folding_past_the_depth_limit() {
    let (codes, sv) = emit_codes(&enum_chain(63));
    assert!(codes.is_empty(), "{codes:?}\n{sv}");

    let (codes, sv) = emit_codes(&enum_chain(64));
    assert_eq!(codes, ["E0003"], "{sv}");
}
