//~ E4010
// Bundle flattening has a budget (ADR-0067): a module may end up with at
// most 4096 flat ports and a bundle may nest at most 8 levels deep. This
// array expands to 256 x 17 = 4352 ports. The limit exists so that an
// exponential (diamond-shaped) or accidentally huge bundle graph is a
// diagnostic instead of a memory blow-up; a real interface never needs it.
struct port Wide {
    out f0  : u8
    out f1  : u8
    out f2  : u8
    out f3  : u8
    out f4  : u8
    out f5  : u8
    out f6  : u8
    out f7  : u8
    out f8  : u8
    out f9  : u8
    out f10 : u8
    out f11 : u8
    out f12 : u8
    out f13 : u8
    out f14 : u8
    out f15 : u8
    out f16 : u8
}

module Sink {
    in ch : [Wide; 256]
    //~^ ERROR E4010
}
