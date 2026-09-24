//~ E4001
// ADR-0073: a 'let' is driven by its initializer. Assigning it again is a
// second driver — before, this compiled silently to
// `wire v = a; assign v = b;` (two drivers on one net).

module LetDoubleDriver {
    in  a   : u8
    in  b   : u8
    out y   : u8

    let v = a
    v = b
//~^ ERROR 'v' is already driven
    y = v
}
