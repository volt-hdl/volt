// ADR-0090 sec. 3: 'let s = sync(x, clk)' is the same bridge as 'wire s'
// + 's = sync(x, clk)' — byte-identical SystemVerilog and the same SDC/XDC
// bridge constraint. The width comes from the type, else from the source.

domain Fast {
    clock = posedge
    reset = sync active_high
}
domain Slow {
    clock = posedge
    reset = sync active_high
}

module LetSync {
    in  fclk  : clock @Fast
    in  sclk  : clock @Slow
    in  flag  : bool  @Fast
    in  level : u4    @Fast
    out seen  : bool  @Slow
    out lvl   : u4    @Slow
    out slow3 : bool  @Slow

    reg f : bool = false
    reg l : u4 = 0
    on fclk {
        f <= flag
        l <= level
    }

    let s = sync(f, sclk)
    let v : u4 = sync(l, sclk)
    let t = sync3(flag, sclk)
    seen  = s
    lvl   = v
    slow3 = t
}
