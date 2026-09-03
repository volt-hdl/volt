//~ E4001
// Aynı sinyale iki farklı yerden atama yapılıyor.

module DoubleDriver {
    in  a : u8
    in  b : u8
    out y : u8

    y = a
    y = b
    //~^ ERROR 'y' zaten sürülüyor (satır 8)
}
