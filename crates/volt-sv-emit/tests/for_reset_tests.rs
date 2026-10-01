//! Reset dalı, `on` bloğunda yazılan HER reg'i kapsar (sv-mapping.md §7)
//! — yazma hangi denetim yapısının içinde olursa olsun. Önceden yalnız bir
//! `for` gövdesinde yazılan reg'in reset satırı sessizce düşüyordu
//! (`collect_written` `BlockStmt::For`'u gezmiyordu): `[7; 2]` başlangıç
//! değeri kayboluyor, reset dalı boş kalıyordu.

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
        "emit hatasız olmalı: {:?}",
        result
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>()
    );
    result.sv
}

/// `if (rst) begin` ile `end else begin` arasındaki reset dalı.
fn reset_branch(out: &str) -> String {
    let start = out.find("if (rst) begin").expect("reset dalı yok");
    let rest = &out[start..];
    let end = rest.find("end else begin").expect("reset dalı kapanmıyor");
    rest[..end].to_string()
}

#[test]
fn register_written_only_in_a_for_loop_keeps_its_reset() {
    let out = sv("module M {
        in clk : clock
        in en : bool
        out a : u8
        reg r : [u8; 2] = [7; 2]
        on clk {
            for i in 0..2 {
                if en { r[i] <= r[i] + 1 }
            }
        }
        a = r[0]
    }");
    let reset = reset_branch(&out);
    assert!(
        reset.contains("r[volt_i] <= 8'd7;"),
        "for içinde yazılan dizi reg reset almalı:\n{out}"
    );
}

#[test]
fn scalar_register_written_only_in_a_for_loop_keeps_its_reset() {
    let out = sv("module M {
        in clk : clock
        out a : u8
        reg s : u8 = 5
        on clk {
            for i in 0..1 { s <= s + 1 }
        }
        a = s
    }");
    assert!(
        reset_branch(&out).contains("s <= 8'd5;"),
        "for içinde yazılan reg reset almalı:\n{out}"
    );
}

#[test]
fn register_written_in_a_nested_for_loop_keeps_its_reset() {
    let out = sv("module M {
        in clk : clock
        out a : u8
        reg r : [u8; 4] = [1, 2, 3, 4]
        on clk {
            for i in 0..2 {
                for j in 0..2 { r[2 * i + j] <= 0 }
            }
        }
        a = r[0]
    }");
    let reset = reset_branch(&out);
    for line in [
        "r[0] <= 8'd1;",
        "r[1] <= 8'd2;",
        "r[2] <= 8'd3;",
        "r[3] <= 8'd4;",
    ] {
        assert!(reset.contains(line), "{line} eksik:\n{out}");
    }
}

#[test]
fn register_written_in_a_for_inside_a_match_arm_keeps_its_reset() {
    let out = sv("module M {
        in clk : clock
        in sel : u2
        out a : u8
        reg r : [u8; 2] = [9; 2]
        on clk {
            match sel {
                0 => { for i in 0..2 { r[i] <= 1 } }
                _ => {}
            }
        }
        a = r[0]
    }");
    assert!(
        reset_branch(&out).contains("r[volt_i] <= 8'd9;"),
        "match kolundaki for içinde yazılan reg reset almalı:\n{out}"
    );
}

#[test]
fn register_written_in_a_match_inside_a_for_keeps_its_reset() {
    let out = sv("module M {
        in clk : clock
        in sel : u2
        out a : u8
        reg r : [u8; 2] = [3; 2]
        on clk {
            for i in 0..2 {
                match sel {
                    1 => { r[i] <= 1 }
                    _ => {}
                }
            }
        }
        a = r[0]
    }");
    assert!(
        reset_branch(&out).contains("r[volt_i] <= 8'd3;"),
        "for içindeki match kolunda yazılan reg reset almalı:\n{out}"
    );
}

/// Kıyas: if/else-if zinciri ve match kolları zaten gezilirdi — bu
/// düzeltmeyle değişmeyen davranışı sabitler.
#[test]
fn registers_written_in_else_if_chains_and_match_arms_keep_their_reset() {
    let out = sv("module M {
        in clk : clock
        in sel : u2
        out a : u8
        reg p : u8 = 1
        reg q : u8 = 2
        reg t : u8 = 4
        on clk {
            if sel == 0 { } else if sel == 1 { } else { p <= 0 }
            match sel {
                2 => { if sel == 2 { q <= 0 } }
                _ => { t <= 0 }
            }
        }
        a = p + q + t
    }");
    let reset = reset_branch(&out);
    for line in ["p <= 8'd1;", "q <= 8'd2;", "t <= 8'd4;"] {
        assert!(reset.contains(line), "{line} eksik:\n{out}");
    }
}
