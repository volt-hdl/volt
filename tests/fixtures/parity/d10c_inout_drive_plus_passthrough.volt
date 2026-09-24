// parity: E4001
// drivers: 11 9
module M {
    in  clk : clock
    in  v : bits<8>
    inout dq : bits<8>
    out o : bits<8>

    on clk { dq.drive(v) }
    o = dq.read()
    dq_out = v
}
