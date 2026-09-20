//! SV operatör önceliği regresyon testleri (ADR-0057).
//!
//! Volt'ta `&` `^` `|` karşılaştırmadan SIKI bağlanır (ADR-0013 §2.2);
//! SystemVerilog'da (IEEE 1800-2017 Tablo 11-2) GEVŞEK. Üretici parantez
//! kararını SV tablosuyla vermelidir — aksi hâlde `(a & b) == 0` sessizce
//! `a & (b == 0)` olur.
//!
//! Son bölümdeki gidiş-dönüş testi üretilen metni bağımsız bir IEEE
//! öncelik ayrıştırıcısıyla geri okur: üreticinin tablosu ile bu dosyadaki
//! tablo ayrı yazıldığından biri kayarsa test kırılır.

use volt_span::FileId;
use volt_sv_emit::{emit, emit_full, SvaMode};

fn sv(src: &str) -> String {
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    // W0010 (karışık öncelik uyarısı) bu testlerde beklenen bir uyarıdır.
    assert!(
        !parsed
            .diagnostics
            .iter()
            .any(|d| d.severity == volt_diagnostics::Severity::Error),
        "parse hatasız olmalı: {:?}\n{src}",
        parsed.error_codes()
    );
    let result = emit(&parsed.ast, "test.volt");
    assert!(
        !result.has_errors(),
        "emit hatasız olmalı: {:?}\n{src}",
        result
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>()
    );
    result.sv
}

/// `y = <expr>` atamasının üretilen sağ tarafı.
fn rhs_of(decls: &str, y_ty: &str, expr: &str) -> String {
    let out = sv(&format!("module M {{ {decls} out y : {y_ty} y = {expr} }}"));
    let line = out
        .lines()
        .find(|l| l.trim_start().starts_with("assign y = "))
        .unwrap_or_else(|| panic!("'assign y' yok:\n{out}"));
    line.trim()
        .trim_start_matches("assign y = ")
        .trim_end_matches(';')
        .to_string()
}

const U8_ABCD: &str = "in a : u8 in b : u8 in c : u8 in d : u8";

// ═══ Bit düzeyi sol operand, karşılaştırma üstte ═════════════════

#[test]
fn bitand_under_eq_keeps_parens() {
    let rhs = rhs_of(U8_ABCD, "bool", "(a & b) == 0");
    assert_eq!(rhs, "(a & b) == 8'd0");
}

#[test]
fn bitand_under_eq_without_source_parens_gets_parens() {
    // Volt'ta parantezsiz yazım da aynı ağaçtır (ADR-0013 §2.2).
    let rhs = rhs_of(U8_ABCD, "bool", "a & b == 0");
    assert_eq!(rhs, "(a & b) == 8'd0");
}

#[test]
fn bitor_under_ne_keeps_parens() {
    let rhs = rhs_of(U8_ABCD, "bool", "(a | b) != c");
    assert_eq!(rhs, "(a | b) != c");
}

#[test]
fn bitxor_under_lt_keeps_parens() {
    let rhs = rhs_of(U8_ABCD, "bool", "(a ^ b) < d");
    assert_eq!(rhs, "(a ^ b) < d");
}

#[test]
fn bitwise_on_right_of_comparison_keeps_parens() {
    let rhs = rhs_of(U8_ABCD, "bool", "a >= (b & c)");
    assert_eq!(rhs, "a >= (b & c)");
}

#[test]
fn bitwise_on_both_sides_of_eq() {
    let rhs = rhs_of(U8_ABCD, "bool", "a & b == c | d");
    assert_eq!(rhs, "(a & b) == (c | d)");
}

#[test]
fn rv32im_mepc_alignment_pattern() {
    // Hatanın bulunduğu kalıp: SV `mepc & (3 == 0)` okuyup sabite indirgiyordu.
    let rhs = rhs_of("in mepc : u32", "bool", "(mepc & 3) == 0");
    assert_eq!(rhs, "(mepc & 32'd3) == 32'd0");
}

// ═══ Karşılaştırma operand, bit düzeyi üstte ═════════════════════

#[test]
fn eq_under_bitand_keeps_parens() {
    // SV tablosu parantezsiz de doğru okur; okunabilirlik için korunur.
    let rhs = rhs_of("in a : bool in b : u8", "bool", "a & (b == 0)");
    assert_eq!(rhs, "a & (b == 8'd0)");
}

#[test]
fn comparisons_under_bitor_keep_parens() {
    let rhs = rhs_of(U8_ABCD, "bool", "(a < b) | (c != d)");
    assert_eq!(rhs, "(a < b) | (c != d)");
}

// ═══ İki tabloda aynı olan düzeyler — davranış değişmemeli ═══════

#[test]
fn shift_amount_sum_unchanged() {
    // `a << 2 + 1` → `a << (2 + 1)`: miktar sabite katlanır.
    let rhs = rhs_of(U8_ABCD, "u8", "a << 2 + 1");
    assert_eq!(rhs, "a << (3)");
}

#[test]
fn shift_under_bitor_has_no_parens() {
    let rhs = rhs_of(U8_ABCD, "u8", "(a << 2) | (b >> 1)");
    assert_eq!(rhs, "a << 2 | b >> 1");
}

#[test]
fn bitwise_chain_has_no_extra_parens() {
    let rhs = rhs_of(U8_ABCD, "u8", "a & b | c ^ d");
    assert_eq!(rhs, "a & b | c ^ d");
    let rhs = rhs_of(U8_ABCD, "u8", "a & (b | c)");
    assert_eq!(rhs, "a & (b | c)");
}

#[test]
fn logical_over_comparison_has_no_parens() {
    let rhs = rhs_of(U8_ABCD, "bool", "a == b && c < d || a != d");
    assert_eq!(rhs, "a == b && c < d || a != d");
}

#[test]
fn arithmetic_under_comparison_has_no_parens() {
    let rhs = rhs_of(U8_ABCD, "bool", "a + b == c * d");
    assert_eq!(rhs, "a + b == c * d");
}

#[test]
fn explicit_comparison_nesting_keeps_tree() {
    let rhs = rhs_of(U8_ABCD, "bool", "(a == b) < (c == d)");
    assert_eq!(rhs, "(a == b) < (c == d)");
    let rhs = rhs_of(U8_ABCD, "bool", "(a < b) == (c < d)");
    assert_eq!(rhs, "a < b == c < d");
}

// ═══ Diğer ifade konumları ═══════════════════════════════════════

#[test]
fn ui_pass_precedence_fixture_masks_before_comparing() {
    let src = include_str!("../../../tests/ui/pass/04_operator_precedence.volt");
    let out = sv(src);
    assert!(
        out.contains("assign r2 = (a & 8'hF) == 8'd0;"),
        "çıktı:\n{out}"
    );
}

#[test]
fn if_condition_in_sequential_block() {
    let out = sv("module M { in clk : clock in a : u8 out q : bool \
         reg q_r : bool = false \
         on clk { if a & 3 == 0 { q_r <= true } } q = q_r }");
    assert!(out.contains("if ((a & 8'd3) == 8'd0)"), "çıktı:\n{out}");
}

#[test]
fn if_expression_condition() {
    let rhs = rhs_of(U8_ABCD, "u8", "if a & b != 0 { c } else { d }");
    assert_eq!(rhs, "((a & b) != 8'd0) ? c : d");
}

#[test]
fn implication_operands() {
    let rhs = rhs_of(U8_ABCD, "bool", "a & 1 == 1 -> b | c != 0");
    assert_eq!(rhs, "!((a & 8'd1) == 8'd1) || (b | c) != 8'd0");
}

// ═══ SVA (ADR-0057 §formal): kontratlar aynı üreticiyi kullanır ══

fn sva(contract: &str) -> String {
    let src = format!(
        "module M {{\n    in clk : clock\n    in a : u8\n    in b : u8\n    \
         out q : u8\n\n    {contract}\n\n    reg q_r : u8 = 0\n\n    \
         on clk {{\n        q_r <= a\n    }}\n\n    q = q_r\n}}\n"
    );
    let parsed = volt_syntax::parser::parse(FileId(0), &src);
    let out = emit_full(&parsed.ast, "test.volt", &src, SvaMode::Separate);
    assert_eq!(out.sva_files.len(), 1);
    out.sva_files[0].content.clone()
}

#[test]
fn sva_invariant_masks_before_comparing() {
    let text = sva("invariant: (q & 3) == 0");
    assert!(text.contains("(q & 8'd3) == 8'd0;"), "{text}");
    assert!(!text.contains("q & 8'd3 == 8'd0"), "{text}");
}

#[test]
fn sva_implication_sides_mask_before_comparing() {
    let text = sva("invariant: a & 1 == 0 -> q | b != 0");
    assert!(
        text.contains("(a & 8'd1) == 8'd0 |-> (q | b) != 8'd0"),
        "{text}"
    );
}

#[test]
fn sva_assume_masks_before_comparing() {
    let text = sva("assume: a ^ b < 4");
    assert!(text.contains("(a ^ b) < 8'd4;"), "{text}");
}

// ═══ Gidiş-dönüş: tüm operatör çiftleri × iki ağaç biçimi ════════

/// Volt ikili operatörleri (kaynak yazımı).
const OPS: [&str; 19] = [
    "->", "||", "&&", "==", "!=", "<", ">", "<=", ">=", "|", "^", "&", "<<", ">>", "+", "-", "*",
    "/", "%",
];

/// IEEE 1800-2017 Tablo 11-2 — yüksek sayı sıkı bağlanır. Üreticideki
/// tablodan BAĞIMSIZ yazılmıştır.
fn ieee_prec(op: &str) -> u8 {
    match op {
        "||" => 1,
        "&&" => 2,
        "|" => 3,
        "^" => 4,
        "&" => 5,
        "==" | "!=" => 6,
        "<" | ">" | "<=" | ">=" => 7,
        "<<" | ">>" => 8,
        "+" | "-" => 9,
        "*" | "/" | "%" => 10,
        other => panic!("bilinmeyen SV operatörü: {other}"),
    }
}

fn tokenize(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_alphanumeric() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            tokens.push(chars[start..i].iter().collect());
        } else {
            let two: String = chars[i..chars.len().min(i + 2)].iter().collect();
            if ["||", "&&", "==", "!=", "<=", ">=", "<<", ">>"].contains(&two.as_str()) {
                tokens.push(two);
                i += 2;
            } else {
                tokens.push(c.to_string());
                i += 1;
            }
        }
    }
    tokens
}

/// Öncelik tırmanışlı SV ifade ayrıştırıcısı → S-ifadesi. İkili
/// operatörlerin tümü sol birleşmelidir (IEEE 1800-2017 §11.3.2).
struct SvParser {
    tokens: Vec<String>,
    pos: usize,
}

impl SvParser {
    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.pos).map(String::as_str)
    }

    fn next(&mut self) -> String {
        let t = self.tokens[self.pos].clone();
        self.pos += 1;
        t
    }

    fn primary(&mut self) -> String {
        let t = self.next();
        match t.as_str() {
            "(" => {
                let inner = self.expr(0);
                assert_eq!(self.next(), ")");
                inner
            }
            "!" => format!("(! {})", self.primary()),
            _ => t,
        }
    }

    fn expr(&mut self, min_prec: u8) -> String {
        let mut lhs = self.primary();
        while let Some(op) = self.peek() {
            if op == ")" {
                break;
            }
            let prec = ieee_prec(op);
            if prec < min_prec {
                break;
            }
            let op = self.next();
            let rhs = self.expr(prec + 1);
            lhs = format!("({op} {lhs} {rhs})");
        }
        lhs
    }
}

fn parse_sv(text: &str) -> String {
    let mut p = SvParser {
        tokens: tokenize(text),
        pos: 0,
    };
    let tree = p.expr(0);
    assert_eq!(p.pos, p.tokens.len(), "artık belirteç: {text}");
    tree
}

/// Beklenen ağaç; implikasyon `!a || b` açılımıyla (ADR-0034).
fn node(op: &str, l: &str, r: &str) -> String {
    if op == "->" {
        format!("(|| (! {l}) {r})")
    } else {
        format!("({op} {l} {r})")
    }
}

#[test]
fn every_operator_pair_round_trips_through_ieee_precedence() {
    let mut checked = 0;
    let mut failures = Vec::new();
    for outer in OPS {
        for inner in OPS {
            let cases = [
                (
                    format!("(a {inner} b) {outer} c"),
                    node(outer, &node(inner, "a", "b"), "c"),
                ),
                (
                    format!("a {outer} (b {inner} c)"),
                    node(outer, "a", &node(inner, "b", "c")),
                ),
            ];
            for (src, expected) in cases {
                let rhs = rhs_of("in a : u8 in b : u8 in c : u8", "bool", &src);
                if parse_sv(&rhs) != expected {
                    failures.push(format!("Volt `{src}` → SV `{rhs}`"));
                }
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 19 * 19 * 2);
    assert!(
        failures.is_empty(),
        "{} ifade SV'de farklı ağaca ayrışıyor:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn ieee_oracle_reads_unparenthesized_mask_compare_the_c_way() {
    // Kâhinin kendisi: hatalı eski çıktı gerçekten yanlış ağaçtır.
    assert_eq!(parse_sv("a & b == c"), "(& a (== b c))");
    assert_eq!(parse_sv("(a & b) == c"), "(== (& a b) c)");
}
