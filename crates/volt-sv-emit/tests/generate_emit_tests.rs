//! Düzenli yapıların SV üretimi — ADR-0056: açılmış örnekler
//! (`pe_0`, `pe_1_2`), paketlenmiş dizi port/wire (`logic [N*W-1:0]`),
//! eleman erişimi part-select, bundle dizisi düz adları.

use volt_span::FileId;
use volt_sv_emit::emit;

fn compile(src: &str) -> volt_sv_emit::EmitResult {
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

fn assert_has(sv: &str, needle: &str) {
    assert!(sv.contains(needle), "'{needle}' bekleniyor:\n{sv}");
}

const PE: &str = "module Pe { in clk : clock\n in a : i8\n out c : i16\n reg r : i16 = 0\n on clk { r <= a as i16 }\n c = r }\n";

#[test]
fn unrolled_instances_are_flat_and_indexed() {
    let out = sv(&format!(
        "{PE}module Top {{ in clk : clock\n in bus : [i8; 3]\n out res : [i16; 3]\n for i in 0..3 {{ let pe = Pe {{ clk: clk, a: bus[i] }}\n res[i] = pe.c }} }}"
    ));
    for k in 0..3 {
        assert_has(&out, &format!("Pe pe_{k} ("));
        assert_has(&out, &format!("logic signed [15:0] pe_{k}_c;"));
    }
    assert!(!out.contains("genvar"), "düz açılım, generate bloğu yok");
}

#[test]
fn nested_instances_carry_both_indices() {
    let out = sv(&format!(
        "{PE}module Grid {{ in clk : clock\n in bus : [i8; 4]\n out res : [i16; 4]\n for y in 0..2 {{ for x in 0..2 {{ let pe = Pe {{ clk: clk, a: bus[y * 2 + x] }}\n res[y * 2 + x] = pe.c }} }} }}"
    ));
    assert_has(&out, "Pe pe_0_0 (");
    assert_has(&out, "Pe pe_1_1 (");
    // Katlanmış indeks: res[1*2+1] → [48 +: 16].
    assert_has(&out, "assign res[48 +: 16] = pe_1_1_c;");
}

#[test]
fn array_ports_are_packed_vectors() {
    let out =
        sv("module M { in a : [i8; 4]\n out c : [i16; 4]\n for i in 0..4 { c[i] = a[i] as i16 } }");
    assert_has(&out, "input  logic [31:0] a");
    assert_has(&out, "output logic [63:0] c");
    // İşaretli eleman okuması `$signed(...)` ile sarılır, genişletme 16'(...).
    assert_has(&out, "assign c[16 +: 16] = 16'($signed(a[8 +: 8]));");
}

#[test]
fn array_wires_are_packed_and_element_assigned() {
    let out = sv("module M { in a : [u8; 2]\n out y : u8\n wire w : [u8; 2]\n for i in 0..2 { w[i] = a[i] }\n y = w[0] | w[1] }");
    assert_has(&out, "logic [15:0] w;");
    assert_has(&out, "assign w[0 +: 8] = a[0 +: 8];");
    assert_has(&out, "assign w[8 +: 8] = a[8 +: 8];");
    assert_has(&out, "assign y = w[0 +: 8] | w[8 +: 8];");
}

#[test]
fn whole_array_assignment_copies_the_vector() {
    let out = sv("module M { in a : [u8; 4]\n out y : [u8; 4]\n y = a }");
    assert_has(&out, "assign y = a;");
}

#[test]
fn signal_indexed_array_port_uses_scaled_part_select() {
    let out = sv("module M { in a : [u8; 4]\n in sel : u2\n out y : u8\n y = a[sel] }");
    assert_has(&out, "a[8 * (sel) +: 8]");
}

#[test]
fn array_output_of_instance_is_bound_as_packed_vector() {
    let out = sv("module Src { in clk : clock\n out v : [u8; 2]\n v[0] = 1\n v[1] = 2 }\nmodule Top { in clk : clock\n out y : [u8; 2]\n let s = Src { clk: clk }\n y = s.v }");
    assert_has(&out, "logic [15:0] s_v;");
    assert_has(&out, ".v  (s_v)");
    assert_has(&out, "assign y = s_v;");
}

#[test]
fn bundle_array_ports_emit_flat_names_and_contracts() {
    let out = sv("module M { in clk : clock\n in rx : [Handshake<u8>; 2]\n out y : u8\n reg seen : bool = false\n on clk { seen <= rx[1].fired }\n rx[0].ready = true\n rx[1].ready = true\n y = rx[0].data | rx[1].data\n invariant: seen == prev(rx[1].valid && rx[1].ready) }");
    assert_has(&out, "input  logic [7:0] rx_0_data,");
    assert_has(&out, "output logic       rx_1_ready");
    assert_has(&out, "assign rx_1_ready = 1'b1;");
    assert_has(&out, "seen <= rx_1_valid && rx_1_ready;");
}

#[test]
fn for_over_array_with_let_wires_emits_suffixed_wires() {
    let out = sv("module M { in x : [u8; 2]\n out y : [u8; 2]\n for i in 0..2 { let t : u8 = x[i] + 1\n y[i] = t } }");
    assert_has(&out, "wire [7:0] t_0 = ");
    assert_has(&out, "wire [7:0] t_1 = ");
    assert_has(&out, "assign y[8 +: 8] = t_1;");
}

#[test]
fn examples_systolic_compiles_under_100_lines_of_source() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/systolic/pe_array.volt"
    );
    let src = std::fs::read_to_string(path).expect("örnek okunmalı");
    assert!(
        src.lines().count() < 100,
        "sistolik örnek < 100 satır olmalı"
    );
    let out = sv(&src);
    assert_eq!(out.matches("Pe pe_").count(), 16, "4x4 = 16 PE örneği");
    assert_has(&out, "Pe pe_3_3 (");
}
