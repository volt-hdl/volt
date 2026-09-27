//~ E1001
// ADR-0085: a misspelled constant in a pattern is E1001 — in Rust it
// would bind a new variable and swallow every value.

const LIMIT : u8 = 10

module TypoPattern {
    in  x : u8
    out y : u8

    y = match x { LIMT => 1, _ => 2 }
    //~^ ERROR undefined name: 'LIMT'
}
