//! Değeri genişliğe bağlı alt ifadenin yalıtılması ve seçim biçimleri (fark
//! testi bulguları, ADR-0108).
//!
//! #117 (#106 sınıfı): Volt'ta bir alt ifadenin değeri işlenenlerinden gelir
//! (doğal genişliğinde), SV'de içinde bulunduğu ifadeden. Değeri genişliğe
//! bağlı alt ifade — işaretsiz çıkarma, doğal genişlikleri farklı
//! işlenenlerin işlemi, kaydırma, `~` — daha geniş bağlama doğal
//! genişliğinde hesaplanıp `W'($unsigned(...))` ile yalıtılarak girer;
//! öz-belirlenimli konumda (kaydırma miktarı, indeks) işlenenler doğal
//! genişliğe genişletilir. Satır içi biçim, `let`'e çıkarılmış biçimle aynı
//! değeri verir.
//!
//! #118: `-(-x)` SV'de `--` (azaltma) olmamalı.
//! #119: seçimin seçimi geçerli SV olmalı (bir boyut dönüşümü seçilemez).
//! #120: 1 bitlik değerden `[0]`/`[0:0]` seçimi değerin kendisi.

use volt_span::FileId;
use volt_sv_emit::emit;

fn sv(ports: &str, body: &str) -> String {
    let src = format!("module M {{\n{ports}\n{body}\n}}\n");
    let parsed = volt_syntax::parser::parse(FileId(0), &src);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.error_codes());
    emit(&parsed.ast, "test.volt").sv
}

const ABC: &str = "    in  a : u8\n    in  b : u8\n    in  c : u8\n    in  c9 : u9\n    in  n : u3";

#[test]
fn unsigned_subtraction_under_a_wider_sum_keeps_its_own_width() {
    let out = sv(&format!("{ABC}\n    out o : u10"), "    o = (a - b) + c9");
    assert!(
        out.contains("assign o = 10'($unsigned(9'(a) - 9'(b))) + 10'(c9);"),
        "{out}"
    );
}

#[test]
fn inline_and_let_forms_agree() {
    let out = sv(
        &format!("{ABC}\n    out o : u10\n    out p : u10"),
        "    let s = a - b\n    o = (a - b) + c9\n    p = s + c9",
    );
    // s : u8..u9 (9 bitte sarar); satır içi biçim aynı değeri verir.
    assert!(out.contains("wire [8:0] s = 9'(a) - 9'(b);"), "{out}");
    assert!(
        out.contains("assign o = 10'($unsigned(9'(a) - 9'(b))) + 10'(c9);"),
        "{out}"
    );
    assert!(out.contains("assign p = 10'(s) + 10'(c9);"), "{out}");
}

#[test]
fn unsigned_subtraction_under_a_wider_product() {
    let out = sv(
        "    in  x : u12\n    in  y : u12\n    out o : u24",
        "    o = (x - 4095) * y",
    );
    assert!(
        out.contains("assign o = 24'($unsigned(13'(x) - 13'd4095)) * 24'(y);"),
        "{out}"
    );
}

#[test]
fn widening_cast_of_a_subtraction_keeps_the_borrow() {
    let out = sv(&format!("{ABC}\n    out o : u16"), "    o = (a - b) as u16");
    assert!(
        out.contains("assign o = 16'($unsigned((9'(a) - 9'(b))));"),
        "{out}"
    );
}

#[test]
fn carry_dropped_by_a_bitwise_operation_stays_dropped() {
    let out = sv(
        &format!("{ABC}\n    out o : u9"),
        "    o = ((a + b) | c) + c",
    );
    assert!(
        out.contains("assign o = 9'($unsigned(a + b | c)) + 9'(c);"),
        "{out}"
    );
}

#[test]
fn right_shift_of_a_sum_reads_the_carry() {
    let out = sv(&format!("{ABC}\n    out o : u9"), "    o = (a + b) >> n");
    assert!(out.contains("assign o = 9'(a) + 9'(b) >> n;"), "{out}");
}

#[test]
fn bit_not_under_a_same_width_cast_is_isolated() {
    let out = sv(
        "    in  w : bits<10>\n    out o : u11",
        "    o = (~w) as u10 + 0",
    );
    assert!(
        out.contains("assign o = 11'($unsigned((~w))) + 11'd0;"),
        "{out}"
    );
}

#[test]
fn negation_inside_a_shift_keeps_its_extra_bit() {
    let out = sv(
        "    in  s : i20\n    in  n : u2\n    out o : i21",
        "    o = -s >> n",
    );
    assert!(out.contains("assign o = -(21'(s)) >>> n;"), "{out}");
}

#[test]
fn shift_amount_and_index_are_computed_at_their_natural_width() {
    let out = sv(
        "    in  p : u63\n    in  m : u2\n    in  k : u2\n    in  q : u25\n    in  i : u3\n    in  j : u3\n    out o : u63\n    out r : bits<16>",
        "    o = p << m * k\n    r = q[i + j +: 16]",
    );
    // m * k : u4 (SV kendi genişliğinde 2 bitte sarardı).
    assert!(out.contains("assign o = p << ((4'(m) * 4'(k)));"), "{out}");
    assert!(
        out.contains("assign r = q[(4'(i) + 4'(j)) +: 16];"),
        "{out}"
    );
}

#[test]
fn wide_shift_operand_does_not_widen_a_comparison() {
    let out = sv(
        "    in  p : u13\n    in  q : u13\n    out o : bool",
        "    let t = p | q * q << 4\n    o = (p | q * q << 4) == t",
    );
    assert!(
        out.contains("assign o = (p | 13'(26'(q) * 26'(q) << 4)) == t;"),
        "{out}"
    );
}

#[test]
fn narrowing_cast_is_kept_when_the_operation_width_equals_the_target() {
    let out = sv(
        "    in  x : i25\n    in  y : i25\n    out o : i26",
        "    o = -((x + y) as i25)",
    );
    assert!(out.contains("25'("), "{out}");
}

#[test]
fn double_negation_is_not_a_decrement() {
    let out = sv("    in  s : i56\n    out o : i58", "    o = -(-s)");
    assert!(out.contains("assign o = -(-(58'(s)));"), "{out}");
    assert!(!out.contains("--"), "{out}");
}

#[test]
fn select_of_a_select_of_a_computed_value_is_a_size_cast() {
    let out = sv(
        "    in  v : bits<70>\n    in  k : u6\n    in  q : u34\n    out o : bits<70>\n    out p : bool",
        "    o = (v >> k)[69:0][69:0]\n    p = (q as bits<34>)[28:0][27]",
    );
    assert!(out.contains("assign o = 70'(70'(v >> k));"), "{out}");
    assert!(out.contains("assign p = 1'((29'(q)) >> 27);"), "{out}");
}

#[test]
fn select_of_a_select_of_a_signal_is_a_size_cast() {
    // Tek boyutlu vektörün seçimi yeniden seçilemez: `q[43:0][42:15]`
    // geçersiz SV.
    let out = sv(
        "    in  q : u46\n    out o : bits<28>",
        "    o = q[43:0][42:15]",
    );
    assert!(out.contains("assign o = 28'((q[43:0]) >> 15);"), "{out}");
}

#[test]
fn bit_zero_of_a_one_bit_value_is_the_value() {
    let out = sv(
        "    in  one : u1\n    in  bit1 : bits<1>\n    out o : bool\n    out p : bits<1>",
        "    o = one[0]\n    p = bit1[0:0]",
    );
    assert!(out.contains("assign o = one;"), "{out}");
    assert!(out.contains("assign p = bit1;"), "{out}");
}

#[test]
fn bit_zero_of_a_signed_one_bit_value_is_unsigned() {
    // Seçim işaretsizdir: `s[0:0]` (s : i1) parça seçimi başlangıcında 1
    // okunmalı, -1 değil.
    let out = sv(
        "    in  s : i1
    in  q : bits<69>
    out o : bits<68>",
        "    o = q[s[0:0] as u1 +: 68]",
    );
    assert!(out.contains("assign o = q[$unsigned(s) +: 68];"), "{out}");
}
