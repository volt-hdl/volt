// delay<K>(x) is a claim, not a register (ADR-0037 §2, ADR-0127): the
// timing check counts x as K cycles older, but no hardware is added and
// Volt does not check the claim. The SV of 'both' is s1 + s2, a 1-cycle
// and a 2-cycle value; it is the right circuit only if s1 does not change
// during that cycle. This file pins that delay<1>(s1) removes E5010 and
// compiles; it is not the way to align two values. A real stage register
// does that: tests/ui/pass/44_delayed_aligned.volt.

@strict_timing
module DelayIsAClaim {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg s1 : Delayed<u32, 1> = 0
    reg s2 : Delayed<u32, 2> = 0

    let both = delay<1>(s1) + s2

    on clk {
        s1 <= x
        s2 <= s1
    }

    y = both
}
