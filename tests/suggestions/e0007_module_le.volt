// suggestion: E0007
// '<=' outside an 'on' block on a wire target: the fix writes '='.
module Le {
    in  a : u8
    out y : u8

    y <= a
}
