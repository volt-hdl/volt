// Test block (ADR-0033): contextual 'test' keyword, script statements
// end with ';', all six builtins are accepted.

module Counter {
    in  clk    : clock
    in  enable : bool
    out count  : u8

    reg count_r : u8 = 0

    on clk {
        if enable {
            count_r <= count_r + 1
        }
    }

    count = count_r
}

test "counter increments" {
    let dut = Counter { };
    dut.enable = true;
    step(1);
    assert_eq(dut.count, 1);
    step(3);
    assert_eq(dut.count, 4);
}

test "reset clears the count" {
    let dut = Counter { };
    dut.enable = true;
    step(2);
    reset();
    assert_false(dut.count);
    assert_ne(dut.count, 2);
}

test "holds value while disabled" {
    let dut = Counter { };
    dut.enable = true;
    step(1);
    dut.enable = false;
    step(5);
    assert_true(dut.count);
    assert_eq(dut.count, 1);
}
