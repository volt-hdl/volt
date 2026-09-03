//~ E2007
// Aralık ters yazılmış: hi < lo olmalı değil.

module ReversedRange {
    in  data  : u8
    out slice : bits<4>

    slice = data[3:7]
    //~^ ERROR aralık ters (hi < lo)
}
