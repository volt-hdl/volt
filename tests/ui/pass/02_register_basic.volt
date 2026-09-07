// Basic register: increments on every clock edge
module Counter {
    in  clk : clock
    out result : u8

    reg r : u8 = 0

    on clk {
        r <= r + 1
    }

    result = r
}
