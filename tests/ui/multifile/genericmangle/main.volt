//~ E1003
// #205 across files: the two generic modules live in two files of the
// unit, and both instances build the module name 'M_1_8'.
use liba::M;
use libb::M_1;

module Top {
    in  x : u8
    out a : u8
    out b : u8
    let p = M<1, 8> { x: x }
    let q = M_1<8> { x: x }
    a = p.y
    b = q.y
}
