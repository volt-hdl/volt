//~ E5010
// #204: `Pair { a, b }` means `Pair { a: a, b: b }`. Under @strict_timing
// a literal that combines a 1-cycle and a 2-cycle value without
// re-alignment is a timing error (ADR-0037).
struct Pair { a : u32, b : u32 }

@strict_timing
module Misaligned {
    in  clk : clock
    in  x   : u32
    out y   : Pair

    reg a : Delayed<u32, 1> = 0
    reg b : Delayed<u32, 2> = 0

    on clk {
        a <= x
        b <= a
    }

    y = Pair { a, b }
    //~^ ERROR timing misalignment
}
