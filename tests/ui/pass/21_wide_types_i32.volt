// Geniş işaretli ve işaretsiz tipler
module WideTypes {
    in  clk : clock
    in  a   : i32
    in  b   : u64
    out sum : i33
    out big : u64

    reg acc : u64 = 0

    sum = a + a

    on clk {
        acc <= acc | b
    }

    big = acc
}
