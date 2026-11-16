//~ PASS
// #132: an untyped `let` in a fn body lost its flexible width when the fn
// was inlined in a comb block: the substituted value kept its natural width
// (`9'(9'(a) + 9'(b)) | c`), so `x` differed from the same call at module
// level (`y`). type-inference.md §3.3 and ADR-0025: `let s = a + b` (a, b u8)
// has the range u8..u9 and carries its natural-width value; `s | c` (c u8)
// is done at the intersection of the ranges, 8 bits, and a bitwise operator
// does not widen; `(s | c) + c` is u8 + u8, natural width u9. ADR-0081
// Karar 12.2 ("Uygulamada netleşenler"): the substitution mode keeps the
// width of the wire mode, so x == y. Fixed by #168.
fn g(a: u8, b: u8, c: u8) -> u9 {
    let s = a + b;
    (s | c) + c
}

module M {
    in  a : u8
    in  b : u8
    in  c : u8
    out x : u9
    out y : u9
    comb { x = g(a, b, c) }
    y = g(a, b, c)
}

test "all ones" {
    let dut = M { };
    dut.a = 255;
    dut.b = 255;
    dut.c = 255;
    step(1);
    // s = 510; s | c at 8 bits: 510 mod 256 = 254, 254 | 255 = 255;
    // 255 + 255 = 510 (u9). The bug gave x = (510 | 255) + 255 = 766,
    // 9 bits: 254.
    assert_eq(dut.x, 510);
    assert_eq(dut.y, 510);
}

test "carry only" {
    let dut = M { };
    dut.a = 128;
    dut.b = 128;
    dut.c = 128;
    step(1);
    // s = 256; at 8 bits 0; 0 | 128 = 128; 128 + 128 = 256 (u9). The bug
    // gave x = (256 | 128) + 128 = 512, 9 bits: 0.
    assert_eq(dut.x, 256);
    assert_eq(dut.y, 256);
}
