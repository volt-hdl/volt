//! W2016'nın susturma kuralları sınırında (ADR-0116 Karar 2): her test bir
//! girdinin uyarı alıp almadığını, değerin elle hesaplanan aralığıyla
//! söyler. `arith_cut_property_tests` susturmanın sağlam olduğunu (hiçbir
//! susturulan kesme değeri değiştirmez) sınar; bu dosya kuralın gereksiz
//! uyarı vermediğini ve Karar 2'nin saydığı biçimlerin dışına çıkmadığını.
//!
//! İşlem genişliği (type-inference.md §3.3, ADR-0025): `x * i` (x : u4,
//! `i` döngü değişkeni) u4..u8, `+ y` (y : u4) 4'te buluşur, `C` = 5;
//! çarpım (doğal genişliği 8) 5 bite kesilir. Karşılaştırma değerin
//! tamamını okur (Karar 2.1); susturmayı yalnız değer aralığı (Karar 2.2)
//! verebilir.

use volt_hir::analyze;
use volt_syntax::{parse, FileId};

/// W2016 sayısı; kaynak hatasız olmalı.
fn w2016(src: &str) -> usize {
    let parsed = parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "{src}\n{:?}",
        parsed.error_codes()
    );
    let result = analyze(&parsed.ast);
    assert!(!result.has_errors(), "{src}\n{:?}", result.error_codes());
    result
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == "W2016")
        .count()
}

/// Blok içi `for` (değişkeni `i`, sınırları `range`) içinde `cond < 3`.
fn in_loop(ports: &str, range: &str, cond: &str) -> String {
    format!(
        "module M {{\n{ports}    out q : bool\n    comb {{\n        q = false\n        for i in {range} {{\n            if {cond} < 3 {{\n                q = true\n            }}\n        }}\n    }}\n}}\n"
    )
}

const U4: &str = "    in  x : u4\n    in  y : u4\n";
const I4: &str = "    in  x : i4\n    in  y : i4\n";

// ═══ Karar 2.2: blok içi `for` değişkeninin sınırları ════════════════════

/// `for i in 0..2`: i 0..1, `x * i` 0..15, toplam 0..30, 5 bite (0..31)
/// sığar: uyarı yok. `0..3`: i 0..2, `x * i` 0..30, toplam 0..45 sığmaz:
/// uyarı (x = 15, i = 2, y = 15'te bugün 45 - 32 = 13).
#[test]
fn loop_variable_bounds_decide_at_the_unsigned_edge() {
    assert_eq!(w2016(&in_loop(U4, "0..2", "x * i + y")), 0);
    assert_eq!(w2016(&in_loop(U4, "0..3", "x * i + y")), 1);
}

/// İşaretli ikiz: x, y : i4 (-8..7), i 0..1: `x * i` -8..7, toplam -16..14,
/// işaretli 5 bite (-16..15) sığar: uyarı yok. i 0..2: `x * i` -16..14,
/// toplam -24..21 sığmaz: uyarı.
#[test]
fn loop_variable_bounds_decide_at_the_signed_edge() {
    assert_eq!(w2016(&in_loop(I4, "0..2", "x * i + y")), 0);
    assert_eq!(w2016(&in_loop(I4, "0..3", "x * i + y")), 1);
}

/// Boş döngünün (`1..1`) değişkeninin değer aralığı yoktur: kural tip
/// aralığına düşer, uyarı kalır. (Ters aralık `[1, 0]` kullanılsaydı
/// `x * i` 0..15, toplam 0..30 sığardı.)
#[test]
fn an_empty_loop_gives_no_value_range() {
    assert_eq!(w2016(&in_loop(U4, "1..1", "x * i + y")), 1);
}

/// En geniş aralık: x, y : u63, `x * i` u63..u126, `+ y` 63'te buluşur,
/// `C` = 64; çarpım 126 bitten kesilir. i yalnız 0: çarpım 0, toplam
/// 0..2^63-1, 64 bite sığar: uyarı yok. i 0..2: çarpım 0..2^64-2, toplam
/// 2^64'ü aşar: uyarı. 126 bitlik düğümün tip aralığı `i128` ile
/// kurulabilir (en geniş durum).
#[test]
fn a_126_bit_product_still_has_a_value_range() {
    let ports = "    in  x : u63\n    in  y : u63\n";
    assert_eq!(w2016(&in_loop(ports, "0..1", "x * i + y")), 0);
    assert_eq!(w2016(&in_loop(ports, "0..3", "x * i + y")), 1);
}

/// Aralık işlenenini kesmeyen düğümden geçer: `x * i + y * i + w * i`
/// (u4) 9 bitte: sol işlenen `x * i + y * i` 9 bitlik, sağ `w * i` u4..u8,
/// 8'de buluşur, `C` = 9; doğal genişliği tam `C` olan işlenen kesilmez.
/// i yalnız 0, değeri 0. `+ z` (z : u4) 5 bitte, 9 bitlik toplam kesilir;
/// sonuç 0..15, sığar: uyarı yok.
#[test]
fn a_node_whose_operand_is_exactly_as_wide_as_the_operation_keeps_its_range() {
    let ports = "    in  x : u4\n    in  y : u4\n    in  w : u4\n    in  z : u4\n";
    assert_eq!(
        w2016(&in_loop(ports, "0..1", "x * i + y * i + w * i + z")),
        0
    );
}

/// İşlenenini kesen düğümün aralığı yoktur (Karar 2.2): `x * i + y` (u4)
/// 5 bitte, çarpım kesilir; i yalnız 0, toplam 0..15, sığar: bu kesme
/// uyarı almaz. Ama kesen düğümün ebeveyne verdiği aralık yoktur (ne
/// türetilmiş ne tip aralığı): `w * i + (x * i + y)` 6 bitte, `w * i`
/// (8 bit) kesilir ve toplamın aralığı kurulamadığından uyarı alır. Bu
/// tutucudur: değer bugün de 0..15'tir; kararın gevşetilmesi #266'nın
/// işidir.
#[test]
fn a_node_that_cuts_an_operand_gives_its_parent_no_value_range() {
    let ports = "    in  x : u4\n    in  y : u4\n    in  w : u4\n";
    assert_eq!(w2016(&in_loop(ports, "0..1", "x * i + y")), 0);
    assert_eq!(w2016(&in_loop(ports, "0..1", "w * i + (x * i + y)")), 1);
}

/// Bit işleminin aralığı izlenmez (Karar 2.2: yapraklar ve `+ - *`):
/// `y | 0` tipinin aralığıdır (0..15). i 0..2: toplam 0..45, sığmaz:
/// uyarı; değeri gerçekten değişir (x = 15, i = 2, y = 15: 45 ile 13).
#[test]
fn a_bitwise_operand_counts_with_its_type_range() {
    assert_eq!(w2016(&in_loop(U4, "0..3", "x * i + (y | 0)")), 1);
}

// ═══ Karar 2.1: okunan genişlik yalnız `+ - *` zincirinden geçer ═════════

/// Bölme işleneninin tamamını okur (Karar 2.1): hedef u9, bölme 9 bitte;
/// bölünen `a * b + c` (u8) 9 bitlik, bölme onu kesmez (W2015 yok),
/// çarpımın 9 bite kesilmesi uyarı alır. Kalan da öyle.
#[test]
fn division_in_a_typed_target_reads_the_whole_dividend() {
    let src = |op: &str| {
        format!(
            "module M {{\n    in  a : u8\n    in  b : u8\n    in  c : u8\n    in  d : u8\n    out o : u9\n    o = (a * b + c) {op} d\n}}\n"
        )
    };
    assert_eq!(w2016(&src("/")), 1);
    assert_eq!(w2016(&src("%")), 1);
}

// ═══ Karar 2.3: daraltan `as` ════════════════════════════════════════════

/// Daraltan `as` alt bitleri okur, W2016 geri alınır; aynı genişliğe
/// dönüşüm daraltmaz (Karar 2.3: "daraltan `as`"), uyarı kalır.
/// `a * b - a` (i32) 33 bitlik.
#[test]
fn only_a_narrowing_cast_takes_the_warning_away() {
    let src = |ty: &str| {
        format!(
            "module M {{\n    in  a : i32\n    in  b : i32\n    out q : bool\n    q = (a * b - a) as {ty} == 3\n}}\n"
        )
    };
    assert_eq!(w2016(&src("i32")), 0);
    assert_eq!(w2016(&src("i33")), 1);
    assert_eq!(w2016(&src("i34")), 1);
}
