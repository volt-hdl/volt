// ADR-0085: a bare name in a pattern is a VALUE — a 'const' or a generic
// parameter — compared with the scrutinee, never a binding. Volt has no
// binding patterns, so 'LIMIT' cannot turn into a catch-all arm.

const LIMIT : u8 = 10
const LOW   : u8 = 1
const HIGH  : u8 = 2
// Declared wider than the scrutinee: compared by value, emitted at the
// scrutinee's width (8'd7, not 16'd7).
const SEVEN : u16 = 7

enum Mode { Off, Slow, Fast }
const DEFAULT_MODE : Mode = Mode::Slow

fn classify(v: u8) -> u8 {
    match v { LIMIT => 3, LOW | HIGH => 1, SEVEN => 5, _ => 0 }
}

module Threshold<const N: u8> {
    in  x   : u8
    out hit : bool

    hit = match x { N => true, _ => false }
}

module PatternNames {
    in  clk  : clock
    in  x    : u8
    in  mode : Mode
    out a    : u8
    out b    : u8
    out c    : u8
    out d    : bool
    out e    : bool

    reg a_r : u8 = 0
    on clk {
        match x {
            LIMIT => { a_r <= 1 }
            _ => { a_r <= 0 }
        }
    }
    a = a_r

    b = classify(x)
    c = match mode { DEFAULT_MODE => 7, Mode::Off => 0, Mode::Fast => 9 }

    let t = Threshold<4> { x }
    d = t.hit
    e = mode == Mode::Fast
}
