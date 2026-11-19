// #354: declarations of Delayed<T, N> on a let, a wire and an output port
// that match the delay of their value. The let over a mix of stages stays
// a retiming assertion and is not compared (forwarding, ADR-0037).

@strict_timing
module DelayedDeclaredCounts {
    in  clk : clock
    in  x   : u32
    in  sel : bool
    out y   : Delayed<u32, 2>
    out fwd : u32

    reg a : Delayed<u32, 1> = 0
    reg b : Delayed<u32, 2> = 0
    on clk {
        a <= x
        b <= a
    }

    let s : Delayed<u32, 2> = b
    wire w : Delayed<u32, 2>
    w = s + b
    let f : Delayed<u32, 1> = if sel { a } else { b }

    y   = w
    fwd = f
}
