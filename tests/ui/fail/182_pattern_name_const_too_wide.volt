//~ E2010
// ADR-0085: a const pattern is compared by VALUE with the scrutinee; a
// value that does not fit the scrutinee type can never match (SV would
// compare 16'd300 with an 8-bit value).

const BIG : u16 = 300
const SMALL : u16 = 7

module WideConstPattern {
    in  x : u8
    out y : u8

    y = match x { SMALL => 3, BIG => 1, _ => 2 }
    //~^ ERROR constant 'BIG' = 300 does not fit in type u8 of the matched value
}
