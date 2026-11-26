// #413: the value of prev(x) has the type of x (ADR-0040: prev(x: T) -> T),
// so its helper register in `volt test` and `volt verify` has the width
// and the sign of x's type: a type alias its target's (ADR-0070 §6), a
// generic parameter and a constant expression their value, a struct field
// its declared type (ADR-0077), an enum its encoding width (ADR-0074), an
// array element its element type (ADR-0056), a Trit two signed bits
// (ADR-0003, ADR-0062), an arithmetic result its natural width (ADR-0025).
// Each module uses one kind of x with prev(x) and prev(x, 2); every input
// also reaches an output, so the SV has no signal only a contract reads.
// This file is an input of the width shadow (ADR-0136), which compares
// every helper register with the type checker.

const K : u32 = 3

type Word = u12

struct P {
    x : u7
    y : i5
}

struct Outer {
    ok : bool
    p  : P
}

enum St {
    Idle,
    Run,
    Done,
}

module Gen<const W: u32> {
    in  clk : clock
    in  g   : uint<W>
    out o   : uint<W>
    reg r : uint<W> = 0
    on clk { r <= g }
    o = r
    invariant: prev(g) != 0 || prev(g, 2) == g
}

module UseAlias {
    in  clk : clock
    in  w   : Word
    out o   : Word
    reg r : Word = 0
    on clk { r <= w }
    o = r
    invariant: prev(w) != 4095 || prev(w, 2) == w
}

module UseGeneric {
    in  clk : clock
    in  g   : u10
    out o   : u10
    let inst = Gen<10> { clk: clk, g: g }
    o = inst.o
}

module UseConstWidth {
    in  clk : clock
    in  c   : uint<K * 2>
    out o   : uint<K * 2>
    reg r : uint<K * 2> = 0
    on clk { r <= c }
    o = r
    invariant: prev(c) != 63 || prev(c, 2) == c
}

module UseFields {
    in  clk : clock
    in  p   : P
    in  q   : Outer
    out x   : u7
    out y   : i5
    out ok  : bool
    out qy  : i5
    reg r : P = P { x: 0, y: 0 }
    on clk { r <= p }
    x = r.x
    y = r.y
    ok = q.ok
    qy = q.p.y
    invariant: prev(p.x) != 127 || prev(p.x, 2) == p.x
    invariant: prev(p.y) < 0 || prev(p.y, 2) >= p.y
    invariant: prev(q.p.y) > -16 || prev(q.p.y, 2) == q.p.y
    invariant: prev(p).y < 0 || prev(p, 2).y == p.y
    invariant: prev(q).p.y >= 0 || prev(q).ok == q.ok
}

module UseEnum {
    in  clk : clock
    in  s   : St
    out o   : St
    reg r : St = St::Idle
    on clk { r <= s }
    o = r
    invariant: prev(s) != St::Done || prev(s, 2) == s
}

module UseArray {
    in  clk : clock
    in  arr : [u6; 4]
    out all : [u6; 4]
    out one : u6
    reg r : u6 = 0
    on clk { r <= arr[1] }
    all = arr
    one = r
    invariant: prev(arr[1]) != 63 || prev(arr[1], 2) == arr[1]
    invariant: prev(arr) == arr || prev(arr, 2) == arr
}

module UseTrit {
    in  clk : clock
    in  t   : Trit
    out o   : Trit
    reg r : Trit = 0
    on clk { r <= t }
    o = r
    invariant: prev(t) < t || prev(t, 2) >= t
}

module UseSigned {
    in  clk : clock
    in  sa  : i8
    in  sb  : i8
    out o   : i8
    out p   : i8
    reg r : i8 = 0
    on clk { r <= sa }
    o = r
    p = sb
    invariant: prev(sa) < sb || prev(sa, 2) >= sb
    invariant: prev(sa + sb) < 0 || prev(sa + sb, 2) > -256
}

module UseSum {
    in  clk : clock
    in  a   : u8
    in  b   : u8
    out o   : u8
    out p   : u8
    reg r : u8 = 0
    on clk { r <= a }
    o = r
    p = b
    invariant: prev(a + b) > 255 || prev(a + b, 2) <= 510
    invariant: prev(prev(a)) == prev(a, 2)
}
