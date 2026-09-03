//~ E0010
// Karşılaştırma operatörleri zincirlenemez.
// Matematiksel yanılgıyı önlemek için yasak.

module ComparisonChain {
    in  a : u8
    in  b : u8
    in  c : u8
    out r : bool

    r = a < b < c
    //~^ ERROR karşılaştırma operatörleri zincirlenemez
}
