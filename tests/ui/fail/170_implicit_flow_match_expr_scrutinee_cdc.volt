//~ E3001
// ADR-0083 Karar 8: match ifadesinin sonucu sınananın saat alanını
// taşır — yabancı alandan seçim kombinasyonel CDC'dir.

domain Fast { clock = posedge, reset = sync active_high }
domain Slow { clock = posedge, reset = sync active_high }

module ScrutineeCdc {
    in  fast_clk : clock @Fast
    in  slow_clk : clock @Slow
    in  fs       : bool  @Fast
    in  sa       : u8    @Slow
    out y        : u8    @Slow

    y = match fs { true => sa, _ => 0 }
    //~^ ERROR different clock domains cannot be combined combinationally
}
