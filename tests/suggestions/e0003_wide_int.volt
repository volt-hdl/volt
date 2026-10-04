// suggestion: E0003
// uN and iN go up to 64 bits; a wider raw value is bits<N>.
module Wide {
    in  a : u128
    out y : bits<128>

    y = a as bits<128>
}
