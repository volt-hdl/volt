// ADR-0081: struct ve enum parametreli fn, struct literal argümanı (tel
// kipinde <fn>_<k>_<param> teli, alan başına sinyal) ve struct dönüşü.

enum Op { Add, Sub, Xor }

struct Pair {
    a : u8
    b : u8
}

fn alu(op: Op, p: Pair) -> u8 {
    if op == Op::Add {
        p.a + p.b
    } else if op == Op::Sub {
        p.a - p.b
    } else {
        p.a ^ p.b
    }
}

fn swap(p: Pair) -> Pair {
    Pair { a: p.b, b: p.a }
}

module FnStructEnum {
    in  op : Op
    in  x  : u8
    in  y  : u8
    out r  : u8
    out s  : Pair

    r = alu(op, Pair { a: x, b: y })
    s = swap(Pair { a: x, b: y })
}
