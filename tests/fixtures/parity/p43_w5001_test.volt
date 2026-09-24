// parity: ok
module M {
    in  clk : clock
    in  x   : u2
    out y   : u2
    reg r : u2 = 0
    on clk { r <= x }
    y = r
    invariant: match r { 0 => true, _ => true }
}
test "t" {
    let dut = M { };
    dut.x = 1;
    step(1);
    assert_eq(dut.y, 1);
}
