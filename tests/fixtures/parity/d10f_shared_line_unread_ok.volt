// parity: ok
// drivers: ok
module Pad {
    in  clk : clock
    in  en : bool
    inout dq : bits<8>
    out r : bits<8>

    reg rr : bits<8> = 0 as bits<8>
    on clk {
        if en { dq.drive(0 as bits<8>) } else { dq.release()
            rr <= dq.read() }
    }
    r = rr
}

module M {
    in  clk : clock
    in  en : bool
    out r : bits<8>

    wire bus : bits<8>
    let p = Pad { clk, en, dq: bus }
    r = p.r
}
