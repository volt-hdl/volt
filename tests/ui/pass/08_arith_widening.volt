// Aritmetik taşma genişlemesi kuralları
// type-inference.md §3.3
module ArithWidening {
    in  a : u8
    in  b : u8
    out sum  : u9      // u8 + u8 → u9
    out prod : u16     // u8 * u8 → u16

    sum  = a + b
    prod = a * b
}
