// Run-time `for` loop in a test (ADR-0058): the same check runs for
// every input vector. The loop is a loop in the generated testbench,
// not an unrolling, so large ranges stay cheap to compile.

module Adder {
    in  clk : clock
    in  a   : u8
    in  b   : u8
    out sum : u8

    reg sum_r : u8 = 0

    on clk {
        sum_r <= a + b
    }

    sum = sum_r
}

test "all inputs" {
    let dut = Adder { };
    for i in 0..16 {
        dut.a = i;
        dut.b = i;
        step(1);
        assert_eq(dut.sum, i * 2);
    }
}

test "nested loops wrap at eight bits" {
    let dut = Adder { };
    let vectors = [0, 1, 0x7F, 0x80, 0xFF];
    for i in 0..len(vectors) {
        for j in 0..len(vectors) {
            dut.a = vectors[i];
            dut.b = vectors[j];
            step(1);
            assert_eq(dut.sum, (vectors[i] + vectors[j]) & 0xFF);
        }
    }
}
