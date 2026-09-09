// Arbitrary-width integer types (ADR-0031): u3/u10/u17/i5 flow end to
// end -- parse, typecheck and SV emission. The u10 counter compares
// directly against a top-level constant of the same width.

const DIVISOR : u10 = 434

module ArbitraryWidths {
    in  clk : clock
    in  a   : u3
    in  b   : u17
    in  s   : i5
    out y   : u17
    out z   : bool

    reg baud_cnt : u10 = 0
    reg acc      : u17 = 0
    reg neg      : i5  = 0

    on clk {
        if baud_cnt == DIVISOR - 1 {
            baud_cnt <= 0
        } else {
            baud_cnt <= baud_cnt + 1
        }
        acc <= acc + b
        neg <= s
    }

    y = acc
    z = a[0]
}
