/// 8-bit yukarı sayaç
/// enable yüksekken her saat kenarında artar
module Counter {
    in  clk    : clock
    in  enable : bool
    out count  : u8

    reg count_r : u8 = 0

    on clk {
        if enable {
            count_r <= count_r + 1
        }
    }

    count = count_r
}
