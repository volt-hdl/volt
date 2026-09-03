// Temel register: her saat kenarında artıyor
module Counter {
    in  clk : clock
    out result : u8

    reg r : u8 = 0

    on clk {
        r <= r + 1
    }

    result = r
}
