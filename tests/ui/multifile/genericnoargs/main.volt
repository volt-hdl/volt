//~ E2003
// #249 across files: the two generic modules live in two files of the
// unit. The instance 'r' has no generic arguments: E2003, and it is not
// bound to the module 'M_1' that M<1> builds.
use liba::M;
use libb::M_1;

module Top {
    in  x : u8
    out a : u8
    out b : u8
    out c : u8
    let p = M<1> { x: x }
    let q = M_1<5> { x: x }
    let r = M_1 { x: x }
    a = p.y
    b = q.y
    c = r.y
}
