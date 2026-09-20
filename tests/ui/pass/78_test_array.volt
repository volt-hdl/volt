// Test-local values (ADR-0058): `let` binds a number or an array
// literal; arrays are indexed with `name[i]` and measured with `len`.
// The expected table lives in the test instead of a generated module.

const TABLE : [u8; 4] = [0x63, 0x7c, 0x77, 0x7b]

module Sbox {
    in  clk  : clock
    in  addr : u2
    out q    : u8

    reg q_r : u8 = 0

    on clk {
        q_r <= TABLE[addr]
    }

    q = q_r
}

test "table lookup" {
    let dut = Sbox { };
    let expected = [0x63, 0x7c, 0x77, 0x7b];
    let last = len(expected) - 1;

    dut.addr = 0;
    step(1);
    assert_eq(dut.q, expected[0]);

    dut.addr = last;
    step(1);
    assert_eq(dut.q, expected[last]);
    assert_ne(dut.q, expected[last - 1]);
    assert_true((expected[1] ^ 0x7c) == 0 && !(last == 0));
}
