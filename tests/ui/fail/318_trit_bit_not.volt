//~ E2003
// #434: '~' is not defined for a Trit (ADR-0142). A Trit is -1, 0 or +1
// (ADR-0003) and '~' inverts its two-bit code (ADR-0062: +1 = 01), so
// ~(+1) would be the code 10, outside the Trit values. Negate a Trit
// with '-' (type-inference.md sec. 3.4: -Trit is a Trit).

module TritBitNot {
    in  w : Trit
    in  x : i8
    out y : i8

    y = (~w) * x
    //~^ ERROR operator '~' is not defined for 'Trit'
}
