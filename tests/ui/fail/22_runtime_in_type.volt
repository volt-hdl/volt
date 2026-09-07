//~ E2021
// Runtime value in a type position.
// Type widths must be known at compile time.

module RuntimeInType {
    in  width_sig : u8
    in  data      : bits<width_sig>
    //~^ ERROR expected a constant expression
    out r         : u8

    r = 0
}
