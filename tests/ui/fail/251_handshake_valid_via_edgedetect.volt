//~ E4007
// ADR-0114 K7: EdgeDetect's rising is combinational in signal (the
// history register holds only the previous value), so valid follows
// ready in the same cycle.
module Producer {
    in  clk  : clock
    in  have : bool
    out tx   : Handshake<u8>

    reg count : u8 = 0
    on clk {
        if tx.fired {
            count <= count + 1
        }
    }
    let ed = EdgeDetect { clk: clk, signal: tx.ready }
    tx.valid = have && ed.rising
    //~^ ERROR handshake 'tx': valid depends combinationally on ready
    tx.data  = count
}
