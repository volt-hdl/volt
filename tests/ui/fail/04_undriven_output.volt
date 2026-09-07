//~ E4002
// The output port is never driven.

module UndrivenOutput {
    in  a : u8
    out y : u8
    out z : u8
    //~^ ERROR output port 'z' is not driven

    y = a
    // z is never assigned
}
