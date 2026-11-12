// #250, the safe twin of fail/270-273: every module name differs from the
// others by more than letter case, so each module writes its own RTL,
// SVA and constraints files. M<8> builds 'M_8', which is not 'm_9'. The
// generic module 'bar' is instantiated, so it leaves the module list and
// writes no file: 'Bar' and 'bar_1' are the module names.
//
// The clock domain gives each module's constraints file a clock period.
domain Sys { clock = posedge, frequency = 100.mhz }

module Foo {
    in  clk : clock @Sys
    in  x   : u8
    out y   : bool
    reg r : bool = false
    on clk { r <= x == 1 }
    invariant: !r || true
    y = r
}

module Foo_2 {
    in  clk : clock @Sys
    in  x   : u8
    out y   : bool
    reg r : bool = false
    on clk { r <= x == 2 }
    invariant: !r || true
    y = r
}

module M<const A: u32> {
    in  clk : clock @Sys
    in  x   : u8
    out y   : bool
    reg r : bool = false
    on clk { r <= x == A }
    invariant: !r || true
    y = r
}

module m_9 {
    in  clk : clock @Sys
    in  x   : u8
    out y   : bool
    reg r : bool = false
    on clk { r <= x != 9 }
    invariant: !r || true
    y = r
}

module bar<const B: u32> {
    in  clk : clock @Sys
    in  x   : u8
    out y   : bool
    reg r : bool = false
    on clk { r <= x == B }
    invariant: !r || true
    y = r
}

module Bar {
    in  clk : clock @Sys
    in  x   : u8
    out y   : bool
    reg r : bool = false
    on clk { r <= x == 3 }
    invariant: !r || true
    y = r
}

module Top {
    in  clk : clock @Sys
    in  x   : u8
    out a   : bool
    out b   : bool
    out c   : bool
    out d   : bool
    out e   : bool
    out f   : bool
    let p = Foo { clk: clk, x: x }
    let q = Foo_2 { clk: clk, x: x }
    let s = M<8> { clk: clk, x: x }
    let t = m_9 { clk: clk, x: x }
    let u = bar<1> { clk: clk, x: x }
    let v = Bar { clk: clk, x: x }
    a = p.y
    b = q.y
    c = s.y
    d = t.y
    e = u.y
    f = v.y
}
