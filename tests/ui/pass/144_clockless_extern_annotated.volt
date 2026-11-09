// ADR-0114 K4: the worst case for a clockless extern applies to
// unannotated outputs only. An extern whose paths are independent keeps
// them apart with @Domain annotations on its ports (K8a).
domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

extern module Bank {
    in  a1 : bool @Fast
    in  a2 : bool @Slow
    out y1 : bool @Fast
    out y2 : bool @Slow
}

module Bridge {
    in  _fast_clk : clock @Fast
    in  slow_clk  : clock @Slow
    in  fast_flag : bool  @Fast
    in  slow_in   : bool  @Slow
    out slow_flag : bool  @Slow
    out slow_echo : bool  @Slow

    let bk = Bank { a1: fast_flag, a2: slow_in }
    let by1 = bk.y1
    slow_flag = sync(by1, slow_clk)
    slow_echo = bk.y2
}

// Output net (ADR-0079):
//~ NET-SKIP: the extern has no @source body on purpose (the fixture shows the domain rule); the design is incomplete without external SV
