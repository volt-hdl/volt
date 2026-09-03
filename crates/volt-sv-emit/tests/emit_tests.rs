//! volt-sv-emit birim testleri.
//!
//! Kural referansları: docs/spec/sv-mapping.md.
//! KRİTİK: counter.volt çıktısı counter.expected.sv ile BİREBİR eşleşmeli.

use volt_span::FileId;
use volt_sv_emit::{emit, EmitResult};

fn compile(src: &str, name: &str) -> EmitResult {
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse hatasız olmalı: {:?}",
        parsed.error_codes()
    );
    emit(&parsed.ast, name)
}

fn sv(src: &str) -> String {
    let result = compile(src, "test.volt");
    assert!(
        !result.has_errors(),
        "emit hatasız olmalı: {:?}",
        result
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>()
    );
    result.sv
}

fn emit_codes(src: &str) -> Vec<&'static str> {
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    let result = emit(&parsed.ast, "test.volt");
    result.diagnostics.iter().map(|d| d.code.as_str()).collect()
}

/// sv-mapping.md §11 yasak yapı denetimi.
fn assert_no_forbidden(sv: &str) {
    for line in sv.lines() {
        let code = line.split("//").next().unwrap_or("");
        assert!(
            !code.contains("always ") && !code.trim_end().ends_with("always"),
            "çıplak 'always' yasak: {line}"
        );
        assert!(
            !code.contains("reg ") && !code.contains(" reg;"),
            "'reg' yasak (logic kullanılmalı): {line}"
        );
        assert!(!code.contains("initial"), "'initial' yasak: {line}");
        assert!(!code.contains('#'), "'#' gecikme yasak: {line}");
        assert!(
            !code.contains("casex") && !code.contains("casez"),
            "casex/casez yasak: {line}"
        );
    }
}

// ═══ KRİTİK: birebir snapshot ═════════════════════════════════════

#[test]
fn counter_matches_expected_sv_byte_for_byte() {
    let src = include_str!("../../../tests/fixtures/counter.volt");
    let expected = include_str!("../../../tests/fixtures/counter.expected.sv");
    let result = compile(src, "counter.volt");
    assert!(!result.has_errors());
    assert_eq!(
        result.sv, expected,
        "üretilen SV beklenen dosyayla birebir eşleşmeli"
    );
}

#[test]
fn counter_output_has_no_forbidden_constructs() {
    let src = include_str!("../../../tests/fixtures/counter.volt");
    assert_no_forbidden(&compile(src, "counter.volt").sv);
}

// ═══ Başlık ve altbilgi (§12) ═════════════════════════════════════

#[test]
fn header_contains_source_and_version() {
    let out = sv("module M { in a : u8 out b : u8 b = a }");
    assert!(out.starts_with("// Bu dosya Volt tarafından otomatik üretilmiştir.\n"));
    assert!(out.contains("// Kaynak: test.volt\n"));
    assert!(out.contains("// Volt sürümü: 0.1.0\n"));
    assert!(out.contains("`default_nettype none\n"));
}

#[test]
fn header_is_deterministic_no_date() {
    let out = sv("module M { in a : u8 out b : u8 b = a }");
    assert!(
        !out.contains("tarih"),
        "deterministik build'de tarih olmamalı"
    );
    assert!(!out.contains("Üretim"));
}

#[test]
fn footer_restores_default_nettype() {
    let out = sv("module M { in a : u8 out b : u8 b = a }");
    assert!(out.ends_with("`default_nettype wire\n"));
}

// ═══ Tip eşleme (§2) ══════════════════════════════════════════════

#[test]
fn type_map_unsigned_widths() {
    let out = sv("module M { in a : u8 in b : u16 in c : u32 in d : u64 out y : bool y = a[0] }");
    assert!(out.contains("input  logic [7:0]  a,"));
    assert!(out.contains("input  logic [15:0] b,"));
    assert!(out.contains("input  logic [31:0] c,"));
    assert!(out.contains("input  logic [63:0] d,"));
}

#[test]
fn type_map_bool_is_bare_logic() {
    let out = sv("module M { in f : bool out y : bool y = f }");
    assert!(out.contains("input  logic f,"), "çıktı:\n{out}");
}

#[test]
fn type_map_signed() {
    let out = sv("module M { in a : i8 in b : i16 out y : bool y = a[0] }");
    assert!(out.contains("input  logic signed [7:0]  a,"));
    assert!(out.contains("input  logic signed [15:0] b,"));
}

#[test]
fn type_map_bits_const_expr() {
    let out = sv("module M { in a : bits<4> in b : bits<4 + 4> out y : bool y = a[0] }");
    assert!(out.contains("input  logic [3:0] a,"));
    assert!(out.contains("input  logic [7:0] b,"));
}

// ═══ Port sırası (§1) ═════════════════════════════════════════════

#[test]
fn port_order_clock_reset_in_out() {
    // Kaynakta out önce yazılsa bile sıra: clock → rst → in → out
    let out = sv(
        "module M { out y : u8 in clk : clock in a : u8 reg r : u8 = 0 on clk { r <= a } y = r }",
    );
    let clk_pos = out.find(" clk,").unwrap();
    let rst_pos = out.find(" rst,").unwrap();
    let a_pos = out.find(" a,").unwrap();
    let y_pos = out.find(" y\n").unwrap();
    assert!(
        clk_pos < rst_pos && rst_pos < a_pos && a_pos < y_pos,
        "sıra bozuk:\n{out}"
    );
}

#[test]
fn no_clock_means_no_reset_port() {
    let out = sv("module M { in a : u8 out b : u8 b = a }");
    assert!(
        !out.contains("rst"),
        "saatsiz modülde reset olmamalı:\n{out}"
    );
}

// ═══ Reset varyantları (§7) ═══════════════════════════════════════

#[test]
fn reset_default_sync_active_high() {
    let out = sv("module M { in clk : clock reg r : u8 = 0 on clk { r <= r } }");
    assert!(out.contains("input  logic rst\n"), "çıktı:\n{out}");
    assert!(out.contains("always_ff @(posedge clk) begin"));
    assert!(out.contains("        if (rst) begin"));
}

#[test]
fn reset_sync_active_low() {
    let out = sv("domain D { clock = posedge, reset = sync active_low }\n\
         module M { in clk : clock @D reg r : u8 = 0 on clk { r <= r } }");
    assert!(out.contains("input  logic rst_n\n"), "çıktı:\n{out}");
    assert!(out.contains("always_ff @(posedge clk) begin"));
    assert!(out.contains("if (!rst_n) begin"));
}

#[test]
fn reset_async_active_high() {
    let out = sv("domain D { clock = posedge, reset = async active_high }\n\
         module M { in clk : clock @D reg r : u8 = 0 on clk { r <= r } }");
    assert!(out.contains("always_ff @(posedge clk or posedge rst) begin"));
    assert!(out.contains("if (rst) begin"));
}

#[test]
fn reset_async_active_low_negedge_clock() {
    // sv-mapping.md §4 asenkron varyant + negedge saat (ui/pass/19 deseni)
    let out = sv(
        "domain UsbDomain { clock = negedge, reset = async active_low }\n\
         module M { in usb_clk : clock @UsbDomain in data : u8 out q : u8 \
         reg buffer : u8 = 0 on usb_clk { buffer <= data } q = buffer }",
    );
    assert!(
        out.contains("always_ff @(negedge usb_clk or negedge rst_n) begin"),
        "çıktı:\n{out}"
    );
    assert!(out.contains("if (!rst_n) begin"));
    assert!(out.contains("buffer <= 8'd0;"));
}

#[test]
fn reset_none_no_port_no_reset_block() {
    let out = sv("domain Free { clock = posedge, reset = none }\n\
         module M { in clk : clock @Free reg r : u8 = 0 on clk { r <= r } }");
    assert!(!out.contains("rst"), "reset=none → port yok:\n{out}");
    assert!(!out.contains("if ("), "reset bloğu olmamalı:\n{out}");
    assert!(out.contains("always_ff @(posedge clk) begin\n        r <= r;\n    end"));
}

// ═══ reg ve on bloğu (§3, §4) ═════════════════════════════════════

#[test]
fn reg_becomes_logic_without_initializer() {
    let out = sv("module M { in clk : clock reg r : u16 = 0 on clk { r <= r } }");
    assert!(out.contains("\n    logic [15:0] r;\n"));
    assert!(
        !out.contains("logic [15:0] r = "),
        "başlangıç değeri bildirimde olmamalı"
    );
}

#[test]
fn reg_init_lands_in_reset_block_with_width() {
    let out = sv("module M { in clk : clock reg r : u16 = 0 on clk { r <= r } }");
    assert!(out.contains("            r <= 16'd0;"));
}

#[test]
fn reg_missing_type_is_e2012() {
    assert!(
        emit_codes("module M { in clk : clock reg r = 0 on clk { r <= r } }").contains(&"E2012")
    );
}

#[test]
fn multiple_regs_reset_in_declaration_order() {
    let out = sv(
        "module M { in clk : clock reg a : u8 = 1 reg b : bool = false \
         on clk { b <= a[0] a <= a } }",
    );
    let a_pos = out.find("a <= 8'd1;").expect("a reset");
    let b_pos = out.find("b <= 1'b0;").expect("b reset");
    assert!(a_pos < b_pos, "bildirim sırası korunmalı:\n{out}");
}

#[test]
fn else_if_chain_in_always_ff() {
    let out = sv(
        "module M { in clk : clock in clear : bool in enable : bool \
         reg acc : u16 = 0 on clk { if clear { acc <= 0 } else if enable { acc <= acc + 1 } } }",
    );
    assert!(out.contains("            if (clear) begin"));
    assert!(out.contains("            end else if (enable) begin"));
    assert!(out.contains("acc <= acc + 16'd1;"));
}

// ═══ Kombinasyonel (§5) ═══════════════════════════════════════════

#[test]
fn simple_connection_is_assign() {
    let out = sv("module M { in a : u8 out b : u8 b = a }");
    assert!(out.contains("\n    assign b = a;\n"));
}

#[test]
fn let_becomes_wire_with_width() {
    let out = sv("module M { in a : u8 in b : u8 out y : u8 let s = a + b y = s }");
    assert!(
        out.contains("\n    wire [7:0] s = a + b;\n"),
        "çıktı:\n{out}"
    );
}

#[test]
fn if_expr_becomes_ternary() {
    let out =
        sv("module M { in sel : bool in a : u8 in b : u8 out y : u8 y = if sel { a } else { b } }");
    assert!(out.contains("    assign y = sel ? a : b;"), "çıktı:\n{out}");
}

#[test]
fn nested_if_expr_parenthesized_ternary() {
    // §5.4: a ? 8'd1 : (b ? 8'd2 : 8'd3)
    let out = sv(
        "module M { in a : bool in b : bool out y : u8 y = if a { 1 } else if b { 2 } else { 3 } }",
    );
    assert!(
        out.contains("assign y = a ? 8'd1 : (b ? 8'd2 : 8'd3);"),
        "çıktı:\n{out}"
    );
}

// ═══ Operatörler ve dönüşümler (§6) ═══════════════════════════════

#[test]
fn cast_zero_extend() {
    let out = sv("module M { in a : u8 out y : u16 y = a as u16 }");
    assert!(out.contains("assign y = {{8{1'b0}}, a};"), "çıktı:\n{out}");
}

#[test]
fn cast_sign_extend() {
    let out = sv("module M { in a : i8 out y : bool let w = a as i16 y = w[0] }");
    assert!(out.contains("{{8{a[7]}}, a}"), "çıktı:\n{out}");
}

#[test]
fn bit_and_range_select() {
    let out = sv(
        "module M { in data : u8 out msb : bool out upper : bits<4> \
         msb = data[7] upper = data[7:4] }",
    );
    assert!(out.contains("assign msb = data[7];"));
    assert!(out.contains("assign upper = data[7:4];"));
}

#[test]
fn bitwise_and_unary_ops() {
    let out = sv("module M { in a : u8 in b : u8 out x : u8 out n : u8 x = a & b n = ~a }");
    assert!(out.contains("assign x = a & b;"));
    assert!(out.contains("assign n = ~a;"));
}

#[test]
fn precedence_parens_preserved_in_output() {
    let out = sv("module M { in a : u8 in b : u8 in c : u8 out y : u8 y = (a + b) * c }");
    assert!(out.contains("assign y = (a + b) * c;"), "çıktı:\n{out}");
}

// ═══ Literaller (§10) ═════════════════════════════════════════════

#[test]
fn literal_sizing_dec_hex_bin() {
    let out = sv(
        "module M { in a : u8 out x : u8 out h : u8 out b : bits<4> \
         x = a + 1 h = 0xFF b = 0b1010 }",
    );
    assert!(out.contains("a + 8'd1"), "dec: {out}");
    assert!(out.contains("assign h = 8'hFF;"), "hex: {out}");
    assert!(out.contains("assign b = 4'b1010;"), "bin: {out}");
}

#[test]
fn bool_literals() {
    let out = sv("module M { out t : bool out f : bool t = true f = false }");
    assert!(out.contains("assign t = 1'b1;"));
    assert!(out.contains("assign f = 1'b0;"));
}

#[test]
fn ambiguous_literal_is_e2005() {
    // let genişliği yalnız literalden çıkarılamaz → E2005, tahmin yok
    assert!(emit_codes("module M { let x = 0 }").contains(&"E2005"));
}

// ═══ Doc yorumları (İ5) ═══════════════════════════════════════════

#[test]
fn doc_comments_transferred_as_line_comments() {
    let out = sv("/// deneme modülü\n/// ikinci satır\nmodule M { in a : u8 out b : u8 b = a }");
    assert!(
        out.contains("// deneme modülü\n// ikinci satır\nmodule M ("),
        "çıktı:\n{out}"
    );
}

// ═══ Kapsam dışı (E0003) ══════════════════════════════════════════

#[test]
fn trit_port_is_e0003() {
    assert!(emit_codes("module M { in t : Trit out y : bool y = true }").contains(&"E0003"));
}

#[test]
fn call_expr_is_e0003() {
    // NOT: 'sync' aktif anahtar kelime olduğundan sync() çağrısı parser'da
    // E0001 verir; Call düğümü ancak sıradan bir isimle oluşur. sync()'in
    // CDC çağrısı olarak özel ele alınması F1 parser işi.
    assert!(emit_codes("module M { in a : u8 out y : u8 y = stretch(a) }").contains(&"E0003"));
}

// ═══ ui/pass taraması ═════════════════════════════════════════════

#[test]
fn ui_pass_sweep_no_panics_and_f0_files_emit_clean_sv() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/ui/pass");
    // F0 kapsamında temiz SV üretmesi ZORUNLU dosyalar
    let must_emit = [
        "01_minimal_module.volt",
        "03_conditional.volt",
        "04_operator_precedence.volt",
        "05_multi_stmt_sequential.volt",
        "06_let_binding.volt",
        "09_bitwise_no_widening.volt",
        "11_bit_and_range_select.volt",
        "14_single_clock_no_domain.volt",
        "15_nested_conditionals.volt",
    ];

    let mut clean = 0;
    let mut skipped = 0;
    for entry in std::fs::read_dir(dir).expect("ui/pass okunmalı") {
        let path = entry.expect("girdi").path();
        if path.extension().and_then(|e| e.to_str()) != Some("volt") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let src = std::fs::read_to_string(&path).expect("dosya okunmalı");

        // Panik yok garantisi: parse + emit her dosyada çalışır
        let parsed = volt_syntax::parser::parse(FileId(0), &src);
        let result = emit(&parsed.ast, &name);

        let all_clean = parsed.diagnostics.is_empty() && !result.has_errors();
        if all_clean {
            assert!(result.sv.contains("module"), "{name}: SV üretilmeli");
            assert_no_forbidden(&result.sv);
            clean += 1;
        } else {
            skipped += 1;
        }
        if must_emit.contains(&name.as_str()) {
            assert!(
                all_clean,
                "{name} F0 kapsamında, temiz SV üretmeli; parse: {:?}, emit: {:?}",
                parsed.error_codes(),
                result
                    .diagnostics
                    .iter()
                    .map(|d| d.code.as_str())
                    .collect::<Vec<_>>()
            );
        }
    }
    assert!(
        clean >= must_emit.len(),
        "temiz: {clean}, atlanan: {skipped}"
    );
}

#[test]
fn multiple_modules_in_one_file() {
    let out =
        sv("module A { in a : u8 out b : u8 b = a }\nmodule B { in x : bool out y : bool y = x }");
    assert!(out.contains("module A ("));
    assert!(out.contains("module B ("));
    let a_end = out.find("endmodule").unwrap();
    assert!(
        out[a_end..].contains("module B ("),
        "modüller sıralı olmalı"
    );
}
