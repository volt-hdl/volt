// Minimal module: just a single port connection
module Passthrough {
    in  a : u8
    out b : u8

    b = a
}
