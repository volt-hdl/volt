// ADR-0083 Karar 5: fn gövdesinde match ifadesi (ADR-0081'de ertelenen).
// Modül let'inden çağrı tel kipidir (sonuç teli kök → case); comb
// bloğundan çağrı ikame kipidir (iç → üçlü).

fn alu(op: u3, a: u8, b: u8) -> u8 {
    match op {
        0 => a + b,
        1 => a - b,
        2 => a & b,
        3 => a | b,
        4 => a ^ b,
        _ => 0
    }
}

fn taken(f3: u3, eq: bool, lt: bool) -> bool {
    match f3 {
        0 => eq,
        1 => !eq,
        4 => lt,
        5 => !lt,
        _ => false
    }
}

module MatchExprFn {
    in  op : u3
    in  a  : u8
    in  b  : u8
    out y  : u8
    out z  : u8
    out t  : bool

    let r = alu(op, a, b)
    y = r
    comb {
        z = alu(op, b, a)
    }
    t = taken(op, a == b, a < b)
}
