//~ E3001
// İki farklı saat alanı arasında doğrudan bağlantı.
// sync() köprüsü olmadan geçiş yapılamaz.

domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

module CdcViolation {
    in  fast_clk  : clock @Fast
    in  slow_clk  : clock @Slow
    in  fast_data : u8    @Fast
    out slow_data : u8    @Slow

    slow_data = fast_data
    //~^ ERROR iki farklı saat alanı doğrudan bağlanamaz
}
