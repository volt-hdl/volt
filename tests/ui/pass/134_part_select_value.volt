// Indexed part-select of a computed value (#112): SystemVerilog cannot
// select from an expression, so the value is computed at its natural
// width (a + b is 9 bits), shifted and cut with a size cast. The carry
// (bit 8) can be selected.
module PartSelectValue {
    in  a  : u8
    in  b  : u8
    in  i  : u4
    out y1 : bits<4>
    out y2 : bits<4>
    out y3 : bits<4>
    out y4 : bits<4>

    y1 = (a + b)[4 +: 4]
    y2 = (a + b)[i +: 4]
    y3 = (a + b)[8 -: 4]
    y4 = (a + b)[i -: 4]
}
