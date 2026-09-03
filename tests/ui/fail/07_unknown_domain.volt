//~ E3002
// Tanımlanmamış domain'e referans.

module UnknownDomain {
    in  clk  : clock @Nonexistent
    //~^ ERROR tanımsız saat alanı: 'Nonexistent'
    out y    : u8

    y = 0
}
