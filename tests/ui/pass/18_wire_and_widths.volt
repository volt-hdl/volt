// wire bildirimi ve geniş tipler
module WireAndWidths {
    in  clk : clock
    in  a   : u32
    in  b   : u32
    out sum : u33
    out big : i64

    wire temp : u33

    temp = a + b
    sum  = temp

    big = (a as i64) - (b as i64)
}
