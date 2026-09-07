//~ E2007
// The range is written in reverse: hi must not be below lo.

module ReversedRange {
    in  data  : u8
    out slice : bits<4>

    slice = data[3:7]
    //~^ ERROR range is reversed (hi < lo)
}
