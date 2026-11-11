//~ E1003
// #250, fail/272 in the other order: Top comes first and instantiates
// m_8 before M<8>, and the module m_8 is declared after the generic
// module M. E1003 at the later of the two names, 'm_8'.
module Top {
    in  clk : clock
    in  x   : u8
    out a   : bool
    out b   : bool
    let q = m_8 { clk: clk, x: x }
    let p = M<8> { clk: clk, x: x }
    a = p.y
    b = q.y
}

module M<const A: u32> {
    in  clk : clock
    in  x   : u8
    out y   : bool
    reg r : bool = false
    on clk { r <= x == A }
    invariant: !r || true
    y = r
}

module m_8 {
//~^ ERROR module names 'M_8' and 'm_8' differ only in letter case
    in  clk : clock
    in  x   : u8
    out y   : bool
    reg r : bool = false
    on clk { r <= x != 8 }
    invariant: !r || true
    y = r
}
