//~ E2003
// #356: a reset read as data through `match` reached sync(). `t` is `rst`
// itself, and the two flip-flops of sync(t, clk) are cleared by this
// module's synchronized copy of the same `rst` (sv-mapping.md §8: the
// stages are reset to 0; ADR-0065 §2): while `rst` is asserted they hold
// 0, after release `t` is 0, so `y` was 0 in every cycle. `volt check`
// reported nothing. ADR-0065 R3: a reset cannot be used as data, E2003;
// `if rst`, `!rst` and `rst as bool` already followed it, the scrutinee of
// a `match` and the operands of a comparison did not. A design that needs
// a soft reset reads a separate `bool` input instead.

module ResetReadAsData {
    in  clk : clock
    in  rst : reset(sync, active_high)
    out y   : bool

    let t = match rst { true => true, _ => false }
    y = sync(t, clk)
}
