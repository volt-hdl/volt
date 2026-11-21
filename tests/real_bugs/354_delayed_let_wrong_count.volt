//~ E5010
// #354: a Delayed<T, N> declaration on a let was never compared with the
// delay of its initializer, so a wrong N silenced E5010 one line later.
// `b` is exactly 2 cycles old (a register is its source's delay + 1,
// type-inference.md §12); the declaration says 1, so `s + a` looked like
// two 1-cycle values. In simulation y was x(t-1) + x(t-2) under
// @strict_timing, whose promise is that this mix is rejected (ADR-0037).
// A declared delay is checked when the value's delay is exactly known
// (ADR-0126); the mismatch is E5010 at the declaration.
@strict_timing
module P {
    in  clk : clock
    in  x   : u32
    out y   : u32
    reg a : Delayed<u32, 1> = 0
    reg b : Delayed<u32, 2> = 0
    on clk { a <= x  b <= a }
    let s : Delayed<u32, 1> = b
    y = s + a
}
