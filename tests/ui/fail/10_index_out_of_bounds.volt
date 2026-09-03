//~ E2006
// Bit indeksi sınır dışı.

module IndexOutOfBounds {
    in  data : u8
    out bit  : bool

    bit = data[9]
    //~^ ERROR indeks 9 sınır dışı (genişlik 8)
}
