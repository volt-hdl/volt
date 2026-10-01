//~ E0019
// A register assigned with '=' outside an 'on' block. `r = r + 1` used to
// become `assign r = r + 8'd1;`: a combinational loop with the reset value
// lost, reported only as the warning W3001.

module RegisterAssignOutsideOn {
    in  clk : clock
    out q   : u8

    reg r : u8 = 3
    r = r + 1
    //~^ ERROR register 'r' cannot be assigned with '=' outside an 'on' block
    q = r
}
