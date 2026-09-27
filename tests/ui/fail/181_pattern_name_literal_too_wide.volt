//~ E2010
// ADR-0085: a pattern value is typed like 'x == value'. 300 does not fit
// in u8 — SystemVerilog would compare with 8'd300, which is 44.

module WideLiteralPattern {
    in  x : u8
    out y : u8

    y = match x { 300 => 1, _ => 2 }
    //~^ ERROR literal 300 does not fit in type u8 (maximum 255)
}
