//~ E5013
// #206: the pipeline desugar runs when the closing '}' is read. A W0010
// warning in the last statement of the pipeline used to open the
// cascade-suppression window there, so the E5013 of the desugar was
// swallowed: the invalid stall produced no stall logic at all and only
// W1001 for the unused 'hold'. Stalling S1 without S0 is not a prefix of
// the stages (ADR-0038 §4): E5013.
pipeline(3) P {
    in  clk : clock
    in  x : u32
    in  hold : bool
    in  a : u8
    in  b : u8
    in  c : u8
    out y : u32
    out o : u8

    stage S0 {
        let p : u32 = x
    }
    stage S1 {
        let q : u32 = p + 1
    }
    stage S2 {
        let w : u32 = q + 1
    }

    stall S1 when hold
    //~^ ERROR stalled stages must form a contiguous prefix starting at the first stage

    y = stage(S2).w
    o = a & b | c
}
