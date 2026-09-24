// Disjoint partial drivers (ADR-0073): the double-driver check works on
// bit ranges. Different blocks may drive different bits, part selects
// or array elements of one signal; only an overlap is E4001. A 'comb'
// block's 'for' loop drives exactly the elements of its range, and a
// wire shared by two inout ports is a tri-state bus, not a double driver.
module Pad {
    in  clk : clock
    in  en  : bool
    inout dq : bits<4>
    out r   : bits<4>

    reg rr : bits<4> = 0 as bits<4>
    on clk {
        if en {
            dq.drive(0 as bits<4>)
        } else {
            dq.release()
            rr <= dq.read()
        }
    }
    r = rr
}

module Split {
    in  clk  : clock
    in  en   : bool
    in  a    : u8
    in  b    : u8
    out y    : u8
    out z    : u8
    out lanes : [u8; 4]
    out r0   : bits<4>
    out r1   : bits<4>

    // Bits: two ranges, then two part selects.
    y[7:4] = a[7:4]
    y[3:0] = b[3:0]
    z[0 +: 4] = a[3:0]
    z[7 -: 4] = b[7:4]

    // Elements 0..2 from one block, 2 and 3 from continuous assignments.
    comb {
        for i in 0..2 {
            lanes[i] = a
        }
    }
    lanes[2] = b
    lanes[3] = a

    // One bus, two tri-state pads.
    wire bus : bits<4>
    let p0 = Pad { clk, en, dq: bus }
    let p1 = Pad { clk, en: !en, dq: bus }
    r0 = p0.r
    r1 = p1.r
}
