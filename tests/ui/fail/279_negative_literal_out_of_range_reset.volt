//~ E2010
// #213: a reset value of -200 does not fit in i8. It was written 8'sh38 (+56).
module M {
    in  clk : clock
    out o : i8
    reg r : i8 = -200
    on clk { r <= r }
    o = r
}
