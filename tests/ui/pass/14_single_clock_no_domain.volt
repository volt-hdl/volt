// Tek saat kuralı: domain hiç yazılmıyor
// domain-inference.md K2 — UX Anayasası ilkesi
module SingleClock {
    in  clk    : clock
    in  enable : bool
    in  data   : u8
    out result : u8

    reg buffer : u8 = 0

    // Hiçbir yerde @Domain yok — çıkarım yapılıyor
    on clk {
        if enable {
            buffer <= data
        }
    }

    result = buffer
}
