//! ADR-0041 SV üretimi: bildirilen `let` genişliği, açık genişleme
//! (`W'(x)`), const diziler, `for` açma, `wire`/`comb`, kullanıcı
//! modülü örnekleme, `uint<N>`/`sint<N>`.

use volt_span::FileId;
use volt_sv_emit::{emit, emit_full_opts, ConstArrayStyle, EmitResult, SvaMode};

fn compile(src: &str) -> EmitResult {
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse hatasız olmalı: {:?}",
        parsed.error_codes()
    );
    emit(&parsed.ast, "test.volt")
}

fn sv(src: &str) -> String {
    let result = compile(src);
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

fn codes(src: &str) -> Vec<&'static str> {
    compile(src)
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect()
}

fn assert_has(sv: &str, needle: &str) {
    assert!(sv.contains(needle), "'{needle}' bekleniyor:\n{sv}");
}

const COEFFS: &str = "const COEFFS : [i16; 4] = [1, 2, 3, 4]\n";

// ═══ 1. let genişliği + açık genişleme ════════════════════════════

#[test]
fn let_annotation_drives_wire_width_and_widens_atoms() {
    let out =
        sv("module M { in a : i16\n in b : i16\n out y : i32\n let p : i32 = a * b\n y = p }");
    assert_has(&out, "wire signed [31:0] p = 32'(a) * 32'(b);");
}

#[test]
fn widening_literal_is_sized_to_context() {
    let out = sv("module M { in a : i16\n out y : i32\n let p : i32 = a * 3\n y = p }");
    assert_has(&out, "wire signed [31:0] p = 32'(a) * 32'sd3;");
}

#[test]
fn widening_const_is_sized_to_context() {
    let out = sv(
        "const C : i16 = 3\nmodule M { in a : i16\n out y : i32\n let p : i32 = a * C\n y = p }",
    );
    assert_has(&out, "wire signed [31:0] p = 32'(a) * 32'sd3;");
}

#[test]
fn assignment_widens_narrow_atom_rhs() {
    let out = sv("module M { in a : u8\n out y : u16\n y = a }");
    assert_has(&out, "assign y = 16'(a);");
}

#[test]
fn compound_expression_is_not_wrapped_only_its_atoms() {
    let out = sv("module M { in a : i16\n in b : i16\n in c : i16\n out y : i32\n y = a * b + c }");
    assert_has(&out, "assign y = 32'(a) * 32'(b) + 32'(c);");
}

#[test]
fn shift_left_operand_keeps_own_width_result_is_wrapped() {
    let out = sv("module M { in a : u8\n out y : u16\n y = a >> 2 }");
    assert_has(&out, "assign y = 16'(a >> 2);");
}

#[test]
fn same_width_operand_is_not_wrapped() {
    let out = sv("module M { in a : u8\n in b : u8\n out y : u8\n y = a + b }");
    assert_has(&out, "assign y = a + b;");
}

#[test]
fn compound_cast_uses_size_cast_instead_of_e2005() {
    let out = sv("module M { in a : i16\n in b : i16\n out y : i32\n y = (a + b) as i32 }");
    assert_has(&out, "assign y = 32'((a + b));");
}

#[test]
fn compound_narrowing_cast_uses_size_cast() {
    let out = sv("module M { in a : u16\n in b : u16\n out y : u8\n y = (a + b) as u8 }");
    assert_has(&out, "assign y = 8'((a + b));");
}

#[test]
fn simple_signal_cast_keeps_adr0036_forms() {
    let out = sv("module M { in a : i8\n out y : i16\n y = a as i16 }");
    assert_has(&out, "assign y = {{8{a[7]}}, a};");
}

// ═══ 2. const diziler ═════════════════════════════════════════════

#[test]
fn const_array_constant_index_folds_to_element_literal() {
    let out = sv(&format!(
        "{COEFFS}module M {{ in a : i16\n out y : i16\n y = a + COEFFS[2] }}"
    ));
    assert_has(&out, "assign y = a + 16'sd3;");
    assert!(
        !out.contains("COEFFS"),
        "sabit indeks tablo üretmemeli:\n{out}"
    );
}

#[test]
fn const_array_element_is_sized_to_wider_context() {
    let out = sv(&format!(
        "{COEFFS}module M {{ in a : i16\n out y : i32\n let p : i32 = a * COEFFS[1]\n y = p }}"
    ));
    assert_has(&out, "wire signed [31:0] p = 32'(a) * 32'sd2;");
}

#[test]
fn const_array_variable_index_emits_case_function_by_default() {
    // Varsayılan biçim tablo işlevi: Yosys unpacked localparam dizisini
    // reddeder (ADR-0041 ölçüm notu).
    let out = sv(&format!(
        "{COEFFS}module M {{ in idx : u2\n out y : i16\n y = COEFFS[idx] }}"
    ));
    assert_has(
        &out,
        "function automatic logic signed [15:0] COEFFS_at(input logic [1:0] i);",
    );
    assert_has(&out, "assign y = COEFFS_at(idx);");
    // Tablo gövdenin başında, kullanımından önce.
    assert!(out.find("function automatic").unwrap() < out.find("assign y").unwrap());
}

#[test]
fn const_array_localparam_style_on_request() {
    let src = format!("{COEFFS}module M {{ in idx : u2\n out y : i16\n y = COEFFS[idx] }}");
    let parsed = volt_syntax::parser::parse(FileId(0), &src);
    let out = emit_full_opts(
        &parsed.ast,
        "test.volt",
        &src,
        SvaMode::None,
        ConstArrayStyle::LocalparamArray,
    );
    assert!(out.diagnostics.is_empty(), "{:?}", out.diagnostics);
    assert_has(
        &out.sv,
        "localparam logic signed [15:0] COEFFS [0:3] = '{16'sd1, 16'sd2, 16'sd3, 16'sd4};",
    );
    assert_has(&out.sv, "assign y = COEFFS[idx];");
}

#[test]
fn const_array_case_function_style() {
    let src = format!("{COEFFS}module M {{ in idx : u2\n out y : i16\n y = COEFFS[idx] }}");
    let parsed = volt_syntax::parser::parse(FileId(0), &src);
    let out = emit_full_opts(
        &parsed.ast,
        "test.volt",
        &src,
        SvaMode::None,
        ConstArrayStyle::CaseFunction,
    );
    assert!(out.diagnostics.is_empty(), "{:?}", out.diagnostics);
    assert_has(
        &out.sv,
        "function automatic logic signed [15:0] COEFFS_at(input logic [1:0] i);",
    );
    assert_has(&out.sv, "            2: COEFFS_at = 16'sd3;");
    assert_has(&out.sv, "default: COEFFS_at = 16'sd0;");
    assert_has(&out.sv, "assign y = COEFFS_at(idx);");
}

#[test]
fn const_array_table_declared_once_per_module() {
    let out = sv(&format!(
        "{COEFFS}module M {{ in i : u2\n in j : u2\n out y : i16\n out z : i16\n y = COEFFS[i]\n z = COEFFS[j] }}"
    ));
    assert_eq!(out.matches("function automatic").count(), 1, "{out}");
}

#[test]
fn negative_const_scalar_and_array_element() {
    let out = sv("const NEG : i16 = -2\nconst K : [i16; 2] = [-1, 5]\nmodule M { in a : i16\n out y : i16\n out z : i16\n y = a * NEG\n z = a + K[0] }");
    assert_has(&out, "assign y = a * (-16'sd2);");
    assert_has(&out, "assign z = a + (-16'sd1);");
}

#[test]
fn negative_const_at_top_level_has_no_parens() {
    let out = sv("const NEG : i16 = -2\nmodule M { out y : i16\n y = NEG }");
    assert_has(&out, "assign y = -16'sd2;");
}

#[test]
fn bare_const_array_initializes_register() {
    let out = sv(&format!(
        "{COEFFS}module M {{ in clk : clock\n out y : i16\n reg r : [i16; 4] = COEFFS\n on clk {{ r[0] <= r[1] }}\n y = r[0] }}"
    ));
    assert_has(&out, "r[0] <= 16'sd1;");
    assert_has(&out, "r[3] <= 16'sd4;");
}

#[test]
fn bare_const_array_as_value_is_e2005() {
    let c = codes(&format!("{COEFFS}module M {{ out y : i16\n y = COEFFS }}"));
    assert!(c.contains(&"E2005"), "{c:?}");
}

#[test]
fn const_array_constant_index_out_of_bounds_is_e2006() {
    let c = codes(&format!(
        "{COEFFS}module M {{ out y : i16\n y = COEFFS[7] }}"
    ));
    assert!(c.contains(&"E2006"), "{c:?}");
}

#[test]
fn repeat_const_array_folds() {
    let out = sv("const Z : [u8; 3] = [7; 3]\nmodule M { out y : u8\n y = Z[2] }");
    assert_has(&out, "assign y = 8'd7;");
}

// ═══ 3. for açma ══════════════════════════════════════════════════

#[test]
fn for_in_on_block_unrolls_with_loop_var_substituted() {
    let out = sv("const N : u32 = 4\nmodule M { in clk : clock\n in s : i16\n out y : i16\n reg t : [i16; N] = [0; N]\n on clk { t[0] <= s\n for i in 1..N { t[i] <= t[i - 1] } }\n y = t[3] }");
    assert_has(&out, "t[1] <= t[0];");
    assert_has(&out, "t[2] <= t[1];");
    assert_has(&out, "t[3] <= t[2];");
    assert!(
        !out.contains("t[i]"),
        "döngü değişkeni ikame edilmeli:\n{out}"
    );
}

#[test]
fn for_at_module_level_emits_assigns() {
    let out = sv("module M { in clk : clock\n out a : bool\n out b : bool\n reg r : [bool; 2] = [false; 2]\n on clk { r[0] <= true }\n wire w : u2\n for i in 0..2 { w[i] = r[i] }\n a = w[0]\n b = w[1] }");
    assert_has(&out, "assign w[0] = r[0];");
    assert_has(&out, "assign w[1] = r[1];");
}

#[test]
fn nested_for_unrolls_both_levels() {
    let out = sv("module M { in clk : clock\n out y : bool\n reg r : [bool; 4] = [false; 4]\n on clk { for i in 0..2 { for j in 0..2 { r[i * 2 + j] <= true } } }\n y = r[3] }");
    for k in 0..4 {
        assert_has(&out, &format!("r[{k}] <= 1'b1;"));
    }
}

#[test]
fn loop_var_in_arithmetic_is_sized_by_context() {
    let out = sv("module M { in clk : clock\n out y : u8\n reg r : [u8; 2] = [0; 2]\n on clk { for i in 0..2 { r[i] <= i + 1 } }\n y = r[1] }");
    assert_has(&out, "r[0] <= 8'd0 + 8'd1;");
    assert_has(&out, "r[1] <= 8'd1 + 8'd1;");
}

#[test]
fn for_with_non_constant_bound_is_e2005() {
    let c = codes("module M { in clk : clock\n in n : u8\n out y : u8\n reg r : [u8; 4] = [0; 4]\n on clk { for i in 0..n { r[i] <= 1 } }\n y = r[0] }");
    assert!(c.contains(&"E2005"), "{c:?}");
}

#[test]
fn empty_for_range_emits_nothing() {
    let out = sv("module M { in clk : clock\n out y : bool\n reg r : [bool; 2] = [false; 2]\n on clk { r[0] <= true\n for i in 2..2 { r[i] <= false } }\n y = r[0] }");
    assert!(!out.contains("r[2]"), "{out}");
}

// ═══ 4/5. wire + comb ═════════════════════════════════════════════

#[test]
fn wire_declares_logic_and_comb_becomes_always_comb() {
    let out = sv("module M { in a : u8\n in b : u8\n out y : u8\n wire s : u8\n comb { s = a + b }\n y = s }");
    assert_has(&out, "    logic [7:0] s;");
    assert_has(&out, "    always_comb begin\n        s = a + b;\n    end");
}

#[test]
fn comb_accumulation_over_for_unrolls_with_widening() {
    let out = sv(&format!(
        "{COEFFS}module M {{ in clk : clock\n in s : i16\n out y : i32\n reg t : [i16; 4] = [0; 4]\n wire acc : i32\n on clk {{ t[0] <= s }}\n comb {{ acc = 0\n for i in 0..4 {{ acc = acc + t[i] * COEFFS[i] }} }}\n y = acc }}"
    ));
    assert_has(&out, "        acc = 32'sd0;");
    assert_has(&out, "        acc = acc + 32'(t[0]) * 32'sd1;");
    assert_has(&out, "        acc = acc + 32'(t[3]) * 32'sd4;");
}

// ═══ 6. kullanıcı modülü örnekleme ════════════════════════════════

const CHILD: &str = "module Add { in clk : clock\n in a : u8\n in b : u8\n out y : u8\n out v : bool\n reg r : u8 = 0\n on clk { r <= a + b }\n y = r\n v = true }\n";

#[test]
fn user_module_instance_emits_named_port_connections() {
    let out = sv(&format!(
        "{CHILD}module Top {{ in clk : clock\n in x : u8\n out y : u8\n let f = Add {{ clk, a: x, b: 3 }}\n y = f.y }}"
    ));
    assert_has(
        &out,
        "    Add f (\n        .clk(clk),\n        .rst(rst),\n        .a  (x),\n        .b  (8'd3),\n        .y  (f_y),\n        .v  (f_v)\n    );",
    );
}

#[test]
fn instance_output_wires_are_predeclared_before_use() {
    let out = sv(&format!(
        "{CHILD}module Top {{ in clk : clock\n in x : u8\n out y : u8\n y = f.y\n let f = Add {{ clk, a: x, b: x }} }}"
    ));
    assert_has(&out, "    logic [7:0] f_y;");
    assert_has(&out, "    logic f_v;");
    assert!(out.find("logic [7:0] f_y;").unwrap() < out.find("assign y = f_y;").unwrap());
}

#[test]
fn instance_output_read_has_port_width() {
    let out = sv(&format!(
        "{CHILD}module Top {{ in clk : clock\n in x : u8\n out y : u16\n let f = Add {{ clk, a: x, b: x }}\n y = f.y }}"
    ));
    assert_has(&out, "assign y = 16'(f_y);");
}

#[test]
fn instance_unbound_input_is_e2005() {
    let c = codes(&format!(
        "{CHILD}module Top {{ in clk : clock\n in x : u8\n out y : u8\n let f = Add {{ clk, a: x }}\n y = f.y }}"
    ));
    assert!(c.contains(&"E2005"), "{c:?}");
}

#[test]
fn instance_output_bound_in_literal_is_e2005() {
    let c = codes(&format!(
        "{CHILD}module Top {{ in clk : clock\n in x : u8\n out y : u8\n let f = Add {{ clk, a: x, b: x, y: x }}\n y = f.y }}"
    ));
    assert!(c.contains(&"E2005"), "{c:?}");
}

#[test]
fn writing_to_instance_port_is_e2005() {
    let c = codes(&format!(
        "{CHILD}module Top {{ in clk : clock\n in x : u8\n out y : u8\n let f = Add {{ clk, a: x, b: x }}\n f.y = x\n y = x }}"
    ));
    assert!(c.contains(&"E2005"), "{c:?}");
}

#[test]
fn two_instances_of_the_same_module_get_separate_wires() {
    let out = sv(&format!(
        "{CHILD}module Top {{ in clk : clock\n in x : u8\n out y : u8\n out z : u8\n let f = Add {{ clk, a: x, b: x }}\n let g = Add {{ clk, a: x, b: 1 }}\n y = f.y\n z = g.y }}"
    ));
    assert_has(&out, "    Add f (");
    assert_has(&out, "    Add g (");
    assert_has(&out, "assign z = g_y;");
}

#[test]
fn instance_of_unknown_module_is_e0003() {
    let c = codes("module Top { in clk : clock\n out y : u8\n let f = Nope { clk }\n y = 0 }");
    assert!(c.contains(&"E0003"), "{c:?}");
}

// ═══ uint<N> / sint<N> ════════════════════════════════════════════

#[test]
fn uint_n_and_sint_n_ports_have_constant_widths() {
    let out = sv(
        "const W : u32 = 12\nmodule M { in a : sint<W>\n out y : uint<W * 2>\n y = a as uint<24> }",
    );
    assert_has(&out, "input  logic signed [11:0] a");
    assert_has(&out, "output logic [23:0]        y");
}
