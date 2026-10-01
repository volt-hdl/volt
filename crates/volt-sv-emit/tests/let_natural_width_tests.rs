//! Tipsiz `let`'in tel genişliği: işlemin DOĞAL genişliği
//! (type-inference.md §3.3, ADR-0025). `u8 + u8` sonucu `u9`'dur; tel
//! 8 bit basılırsa `sum : u9 = s` taşma bitini sessizce kaybeder.

use volt_span::FileId;
use volt_sv_emit::emit;

fn sv(src: &str) -> String {
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse hatasız olmalı: {:?}",
        parsed.error_codes()
    );
    let result = emit(&parsed.ast, "test.volt");
    assert!(
        !result.has_errors(),
        "emit hatasız olmalı: {:?}\n{}",
        result
            .diagnostics
            .iter()
            .map(|d| format!("{} {}", d.code.as_str(), d.message))
            .collect::<Vec<_>>(),
        result.sv
    );
    result.sv
}

fn assert_has(sv: &str, needle: &str) {
    assert!(sv.contains(needle), "'{needle}' bekleniyor:\n{sv}");
}

fn module(body: &str) -> String {
    format!("module M {{\n in a : u8\n in b : u8\n in c : u8\n in x : i8\n in y : i8\n{body}\n}}\n")
}

// ═══ Genişleyen işleçler ══════════════════════════════════════════

#[test]
fn untyped_add_keeps_the_carry_bit() {
    let out = sv(&module(" out sum : u9\n let s = a + b\n sum = s"));
    assert_has(&out, "wire [8:0] s = 9'(a) + 9'(b);");
    assert_has(&out, "assign sum = s;");
}

#[test]
fn untyped_add_matches_the_annotated_let() {
    let typed = sv(&module(" out sum : u9\n let s : u9 = a + b\n sum = s"));
    let untyped = sv(&module(" out sum : u9\n let s = a + b\n sum = s"));
    assert_eq!(typed, untyped);
}

#[test]
fn untyped_sub_keeps_the_borrow_bit() {
    let out = sv(&module(" out d : u9\n let ds = a - b\n d = ds"));
    assert_has(&out, "wire [8:0] ds = 9'(a) - 9'(b);");
}

#[test]
fn untyped_mul_doubles_the_width() {
    let out = sv(&module(" out p : u16\n let ps = a * b\n p = ps"));
    assert_has(&out, "wire [15:0] ps = 16'(a) * 16'(b);");
    assert_has(&out, "assign p = ps;");
}

#[test]
fn untyped_add_with_a_literal_widens_too() {
    let out = sv(&module(" out sum : u9\n let s = a + 1\n sum = s"));
    assert_has(&out, "wire [8:0] s = 9'(a) + 9'd1;");
}

#[test]
fn untyped_signed_add_and_negation_widen() {
    let out = sv(&module(
        " out t : i9\n out n : i9\n let ts = x + y\n t = ts\n let ns = -x\n n = ns",
    ));
    assert_has(&out, "wire signed [8:0] ts = 9'(x) + 9'(y);");
    assert_has(&out, "wire signed [8:0] ns = -(9'(x));");
}

#[test]
fn chained_untyped_lets_follow_the_type_checker() {
    // `s` taşır u8..u9; `s + c` ortak genişlik 8, doğal 9 (ADR-0025).
    let out = sv(&module(
        " out t : u9\n let s = a + b\n let ts = s + c\n t = ts",
    ));
    assert_has(&out, "wire [8:0] ts = s + 9'(c);");
}

// ═══ Genişlemeyen işleçler ════════════════════════════════════════

#[test]
fn shift_division_and_bitwise_keep_the_operand_width() {
    let out = sv(&module(
        " out o1 : u8\n out o2 : u8\n out o3 : u8\n let sh = a << 1\n o1 = sh\n let q = a / b\n o2 = q\n let m = a & b\n o3 = m",
    ));
    assert_has(&out, "wire [7:0] sh = a << 1;");
    assert_has(&out, "wire [7:0] q = a / b;");
    assert_has(&out, "wire [7:0] m = a & b;");
}

// ═══ Dar hedef: taşma bitini atmak açık ═══════════════════════════

#[test]
fn narrow_target_truncates_explicitly() {
    // `sum8 : u8 = s` esnek aralığın alt ucu (ADR-0025): taşma bitini
    // bilerek atar; SV'de kesme açık yazılır (Verilator WIDTHTRUNC yok).
    let out = sv(&module(" out sum8 : u8\n let s = a + b\n sum8 = s"));
    assert_has(&out, "wire [8:0] s = 9'(a) + 9'(b);");
    assert_has(&out, "assign sum8 = 8'(s);");
}

#[test]
fn counter_pattern_is_unchanged() {
    let out = sv(
        "module M {\n in clk : clock\n out q : u8\n reg r : u8 = 0\n on clk { r <= r + 1 }\n q = r\n}\n",
    );
    assert_has(&out, "r <= r + 8'd1;");
}
