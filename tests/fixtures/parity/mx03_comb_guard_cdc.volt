// parity: E3001
// ADR-0083 Karar 8: comb'da muhafız alanı.
domain Fast { clock = posedge, reset = sync active_high }
domain Slow { clock = posedge, reset = sync active_high }
pub module M {
    in fclk : clock @Fast
    in sclk : clock @Slow
    in fs : bool @Fast
    in fv : u2 @Fast
    in sa : u8 @Slow
    in sb : bool @Slow
    out y : u8 @Slow
    comb { match sa { 0 if fs => { y = 1 }, _ => { y = 0 } } }
}
