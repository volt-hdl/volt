//~ E2010
// #213: -1000 does not fit in i8. It was written 8'sh18 (+24).
module M {
    in  clk : clock
    out o : i8
    reg r : i8 = 0
    on clk { r <= -1000 }
    o = r
}
