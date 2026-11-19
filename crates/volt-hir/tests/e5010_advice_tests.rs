//! E5010'un tavsiyesi (ADR-0127, #232): ne oldu (hangi değer kaç
//! çevrim yaşında), neden (farklı çevrimlerin değerleri birleşiyor), yol
//! (genç değer için gerçek bir aşama register'ı, kodu ve bedeliyle) ve en
//! son `delay<K>`'nın register değil, denetlenmeyen bir iddia olduğu.
//!
//! Beklenen metinler ADR-0127'nin kararından elle yazıldı. Help'teki kodun
//! tetikleyiciye uygulanınca E5010'u giderdiği ve değerleri gerçekten
//! hizaladığı `volt-driver/tests/e5010_advice_tests.rs`'te sınanır
//! (Verilator ile).

use volt_diagnostics::Diagnostic;
use volt_hir::analyze;
use volt_syntax::{parse, FileId};

/// Kaynaktaki E5010 tanıları (varsayılan dil EN).
fn e5010(src: &str) -> Vec<Diagnostic> {
    let parsed = parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse hataları: {:?}",
        parsed.error_codes()
    );
    analyze(&parsed.ast)
        .diagnostics
        .into_iter()
        .filter(|d| d.code.as_str() == "E5010")
        .collect()
}

/// Tek E5010'un help metni.
fn help(src: &str) -> String {
    let d = e5010(src);
    assert_eq!(d.len(), 1, "tek E5010 beklenir: {d:?}");
    d[0].help.clone().expect("help")
}

fn assert_contains(text: &str, parts: &[&str]) {
    for p in parts {
        assert!(text.contains(p), "eksik: {p:?}\n--- metin:\n{text}");
    }
}

/// `x` → `a` (1) → `b` (2) → `c` (3) → `d` (4) → `e` (5): `ty` tipinde
/// aşama register'ları; `expr`, `y`'yi süren ifade.
fn chain(ty: &str, expr: &str) -> String {
    format!(
        "@strict_timing
module M {{
    in  clk : clock
    in  en  : bool
    in  x   : {ty}
    out y   : {ty}
    reg a : Delayed<{ty}, 1> = {init}
    reg b : Delayed<{ty}, 2> = {init}
    reg c : Delayed<{ty}, 3> = {init}
    reg d : Delayed<{ty}, 4> = {init}
    reg e : Delayed<{ty}, 5> = {init}
    on clk {{ a <= x  b <= a  c <= b  d <= c  e <= d }}
    y = {expr}
}}
",
        init = if ty == "bool" { "false" } else { "0" }
    )
}

#[test]
fn labels_say_how_old_each_value_is_and_the_reason_says_why() {
    let d = e5010(&chain("u32", "a + b"));
    assert_eq!(d.len(), 1);
    let labels: Vec<&str> = d[0].spans.iter().map(|s| s.label.as_str()).collect();
    assert_eq!(
        labels,
        ["this value is 1 cycle old", "this value is 2 cycles old"]
    );
    let reason = &d[0].notes[0].text;
    assert_eq!(
        reason,
        "the result combines values from different cycles: the 1-cycle value belongs to an item that entered 1 cycle later than the 2-cycle value"
    );
    let d = e5010(&chain("u32", "a + d"));
    assert!(
        d[0].notes[0].text.contains("entered 3 cycles later"),
        "{d:?}"
    );
}

#[test]
fn help_gives_the_stage_register_with_its_code_and_cost_then_the_claim() {
    let h = help(&chain("u32", "a + b"));
    assert_eq!(
        h,
        "add a stage register so that the 1-cycle value is 2 cycles old too: \
         `reg a_d2 : Delayed<u32, 2> = 0`, write `a_d2 <= a` in the `on clk` block, \
         and use `a_d2` here (cost: 1 register of 32 bits); \
         `delay<1>(…)` is not such a register: it is a claim that adds no hardware \
         and that Volt does not check, right only if the value really is 2 cycles old \
         already (it does not change during that cycle)"
    );
}

#[test]
fn the_younger_value_is_named_on_either_side() {
    // Genç değer sağda da olsa adı ve register'ı onundur.
    let h = help(&chain("u32", "b + a"));
    assert_contains(&h, &["`reg a_d2 : Delayed<u32, 2> = 0`", "`a_d2 <= a`"]);
    // if ifadesinin kolları da doğrudan işlenendir.
    let h = help(&chain("u32", "if en { b } else { a }"));
    assert_contains(&h, &["`reg a_d2 : Delayed<u32, 2> = 0`", "use `a_d2` here"]);
    let h = help(&chain("u32", "if en { a } else { b }"));
    assert_contains(&h, &["`reg a_d2 : Delayed<u32, 2> = 0`", "use `a_d2` here"]);
}

#[test]
fn a_difference_of_two_cycles_is_a_chain_of_two_registers() {
    let h = help(&chain("u8", "a + c"));
    assert_contains(
        &h,
        &[
            "add 2 stage registers so that the 1-cycle value is 3 cycles old too",
            "`reg a_d2 : Delayed<u8, 2> = 0`, `reg a_d3 : Delayed<u8, 3> = 0`",
            "write `a_d2 <= a  a_d3 <= a_d2` in the `on clk` block",
            "use `a_d3` here (cost: 2 registers of 8 bits)",
            "`delay<2>(…)` is not such a register",
            "(it does not change during those 2 cycles)",
        ],
    );
    // Üç register hâlâ tek tek yazılır.
    let h = help(&chain("u8", "a + d"));
    assert_contains(
        &h,
        &[
            "`reg a_d2 : Delayed<u8, 2> = 0`, `reg a_d3 : Delayed<u8, 3> = 0`, `reg a_d4 : Delayed<u8, 4> = 0`",
            "`a_d2 <= a  a_d3 <= a_d2  a_d4 <= a_d3`",
            "(cost: 3 registers of 8 bits)",
        ],
    );
}

#[test]
fn a_long_chain_is_written_by_its_first_and_last_register() {
    let h = help(&chain("u8", "a + e"));
    assert_contains(
        &h,
        &[
            "add 4 stage registers",
            "`reg a_d2 : Delayed<u8, 2> = 0`, …, `reg a_d5 : Delayed<u8, 5> = 0`",
            "`a_d2 <= a  …  a_d5 <= a_d4`",
            "use `a_d5` here (cost: 4 registers of 8 bits)",
        ],
    );
    assert!(!h.contains("a_d3"), "{h}");
}

#[test]
fn the_register_takes_the_type_and_reset_value_of_the_value() {
    for (ty, decl, cost) in [
        (
            "i8",
            "`reg a_d2 : Delayed<i8, 2> = 0`",
            "1 register of 8 bits",
        ),
        (
            "bits<4>",
            "`reg a_d2 : Delayed<bits<4>, 2> = 0`",
            "1 register of 4 bits",
        ),
        (
            "u16",
            "`reg a_d2 : Delayed<u16, 2> = 0`",
            "1 register of 16 bits",
        ),
    ] {
        let h = help(&chain(ty, "a ^ b"));
        assert_contains(&h, &[decl, cost]);
    }
    let h = help(&chain("bool", "a && b"));
    assert_contains(
        &h,
        &[
            "`reg a_d2 : Delayed<bool, 2> = false`",
            "1 register of 1 bit)",
        ],
    );
}

#[test]
fn a_compound_younger_value_gets_the_general_form() {
    let src = "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32
    reg a : Delayed<u32, 1> = 0
    reg c : Delayed<u32, 1> = 0
    reg b : Delayed<u32, 2> = 0
    on clk { a <= x  c <= x  b <= a }
    y = (a ^ c) + b
}
";
    let h = help(src);
    assert_contains(
        &h,
        &[
            "add a stage register so that the 1-cycle value is 2 cycles old too: \
             declare each as `reg … : Delayed<T, N> = 0`, load the first from the value \
             and each next one from the one before in the `on clk` block, and use the \
             last one here (cost: 1 register of 32 bits)",
            "`delay<1>(…)` is not such a register",
        ],
    );
    assert!(!h.contains("a_d2"), "{h}");
}

#[test]
fn a_value_without_a_zero_reset_gets_the_general_form() {
    let src = "struct P { lo : u8 }
@strict_timing
module M {
    in  clk : clock
    in  en  : bool
    in  x   : P
    out y   : P
    reg a : Delayed<P, 1> = P { lo: 0 }
    reg b : Delayed<P, 2> = P { lo: 0 }
    on clk { a <= x  b <= a }
    y = if en { a } else { b }
}
";
    let h = help(src);
    assert_contains(&h, &["declare each as `reg … : Delayed<T, N> = 0`"]);
    assert!(!h.contains("a_d2"), "{h}");
}

#[test]
fn a_name_already_in_use_is_not_offered() {
    let src = "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32
    out z   : u32
    reg a    : Delayed<u32, 1> = 0
    reg b    : Delayed<u32, 2> = 0
    reg a_d2 : u32 = 0
    on clk { a <= x  b <= a  a_d2 <= x }
    y = a + b
    z = a_d2
}
";
    let h = help(src);
    assert_contains(&h, &["declare each as `reg … : Delayed<T, N> = 0`"]);
    assert!(!h.contains("`reg a_d2"), "{h}");
}

#[test]
fn with_two_clocks_the_help_names_the_value_s_clock_block() {
    let src = "@strict_timing
module M {
    in  clk  : clock
    in  clk2 : clock
    in  x    : u32
    out y    : u32
    out z    : u32
    reg a : Delayed<u32, 1> = 0
    reg b : Delayed<u32, 2> = 0
    reg q : u32 = 0
    on clk { a <= x  b <= a }
    on clk2 { q <= x }
    y = a + b
    z = q
}
";
    let h = help(src);
    assert_contains(&h, &["write `a_d2 <= a` in the `on` block of its clock"]);
    assert!(!h.contains("`on clk`"), "{h}");
}

#[test]
fn a_register_written_from_a_younger_value_gets_the_stage_register_first() {
    let src = "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32
    reg c : Delayed<u32, 2> = 0
    on clk { c <= x }
    y = c
}
";
    let h = help(src);
    assert_eq!(
        h,
        "write the register from a value 1 cycle old: add a stage register so that \
         the 0-cycle value is 1 cycle old too: `reg x_d1 : Delayed<u32, 1> = 0`, write \
         `x_d1 <= x` in the `on clk` block, and use `x_d1` here (cost: 1 register of 32 \
         bits); or, if it really holds a 1-cycle value, declare it as `Delayed<T, 1>`; \
         `delay<1>(…)` is not such a register: it is a claim that adds no hardware and \
         that Volt does not check, right only if the value really is 1 cycle old \
         already (it does not change during that cycle)"
    );
}

#[test]
fn a_register_written_from_an_older_value_gets_the_declaration_or_an_earlier_stage() {
    let src = "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32
    reg a : Delayed<u32, 1> = 0
    reg b : Delayed<u32, 2> = 0
    reg c : Delayed<u32, 1> = 0
    on clk { a <= x  b <= a  c <= b }
    y = c
}
";
    let h = help(src);
    assert_eq!(
        h,
        "declare the register as `Delayed<T, 3>`, or write it from the stage 2 cycles \
         earlier, where the value is 0 cycles old"
    );
}

#[test]
fn a_register_declared_zero_cycles_old_cannot_exist() {
    let src = "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32
    reg c : Delayed<u32, 0> = 0
    on clk { c <= x }
    y = c
}
";
    let h = help(src);
    assert_eq!(
        h,
        "a register is at least 1 cycle old: declare it as `Delayed<T, 1>`"
    );
}

#[test]
fn no_help_presents_delay_as_alignment() {
    // Eski tavsiye: "align the younger value with delay<K>(...)" (#232).
    for src in [
        chain("u32", "a + b"),
        chain("u8", "a + e"),
        "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32
    reg c : Delayed<u32, 2> = 0
    on clk { c <= x }
    y = c
}
"
        .to_string(),
    ] {
        let h = help(&src);
        assert!(!h.contains("align"), "{h}");
        assert!(h.contains("is not such a register"), "{h}");
    }
}
