// Nested conditionals, mandatory else
module NestedCond {
    in  a : bool
    in  b : bool
    in  c : bool
    in  x : u8
    in  y : u8
    out r : u8

    // else is MANDATORY in an if expression (prevents E0008)
    r = if a {
            if b { x } else { y }
        } else {
            if c { y } else { 0 }
        }
}
