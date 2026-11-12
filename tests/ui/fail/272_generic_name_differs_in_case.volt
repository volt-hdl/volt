//~ E1003
// #250 with a generic instance: M<8> builds the module name 'M_8'
// (sv-mapping.md §9), which differs from the module 'm_8' only in letter
// case. Before the fix, on Windows 'rtl/M_8.sv' held 'module m_8' and no
// 'module M_8' was written; on every operating system one 'formal/m_8.sva'
// was written, and the invariant of M_8 was lost. E1003 (ADR-0090).
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
    in  clk : clock
    in  x   : u8
    out y   : bool
    reg r : bool = false
    on clk { r <= x != 8 }
    invariant: !r || true
    y = r
}

module Top {
    in  clk : clock
    in  x   : u8
    out a   : bool
    out b   : bool
    let p = M<8> { clk: clk, x: x }
    //~^ ERROR module names 'm_8' and 'M_8' differ only in letter case
    let q = m_8 { clk: clk, x: x }
    a = p.y
    b = q.y
}
