//~ PASS
// #413: in `volt test` and `volt verify` the helper register of prev(x)
// took the SV emitter's own width of x and was always declared unsigned
// (`logic [W-1:0]`). prev(x) has the type of x (ADR-0040: prev(x: T) -> T):
// a signed x (iN, a signed struct field, a Trit: ADR-0003, ADR-0062) keeps
// its sign, and an arithmetic x has its natural width (ADR-0025, u8 + u8 is
// nine bits). The unsigned register compared -1 as 255; the eight-bit
// register of a + b kept 300 as 44, and `--emit=sva` wrote
// `$past(a + b) != 8'h2C`. The contracts below hold in Volt; before the fix
// each test failed with a contract violation and no diagnostic. Fixed by
// the pull request that closes #413: the width and the sign of prev(x) come
// from the type checker (ADR-0137, ADR-0141).

struct P {
    x : u7
    y : i5
}

module SignedPrev {
    in  clk : clock
    in  chk : bool
    in  sa  : i8
    in  sb  : i8
    out y   : i8

    reg r : i8 = 0
    on clk { r <= sa }
    y = r

    invariant: !chk || prev(sa) < sb
    invariant: !chk || prev(sa, 2) < sb
}

module FieldPrev {
    in  clk : clock
    in  chk : bool
    in  xin : u7
    in  yin : i5
    in  zin : i5
    out y   : i5

    reg p : P = P { x: 0, y: 0 }
    on clk { p <= P { x: xin, y: yin } }
    y = p.y

    invariant: !chk || prev(p.y) < zin
    invariant: !chk || prev(p).y < zin
}

module TritPrev {
    in  clk  : clock
    in  chk  : bool
    in  up   : bool
    in  down : bool
    out o    : Trit

    reg t : Trit = 0
    on clk { t <= if up { 1 } else if down { -1 } else { 0 } }
    o = t

    invariant: !chk || prev(t) < t
}

module SumPrev {
    in  clk : clock
    in  chk : bool
    in  a   : u8
    in  b   : u8
    in  c   : u9
    in  sa  : i8
    in  sb  : i8
    in  d   : i9
    out y   : u8

    reg r : u8 = 0
    on clk { r <= a }
    y = r

    invariant: !chk || prev(a + b) == c
    invariant: !chk || prev(a + b, 2) == c
    invariant: !chk || prev(sa + sb) == d
}

// A contract is checked at each clock edge on the values before the edge.
// Each test holds its inputs for three cycles, so every helper register of
// depth 1 and 2 holds the same value, and only then turns the check on.
// The test language writes a signed value as its bit pattern (ADR-0059) and
// an `out` port reads back its bit pattern: i8 -1 is 255, i8 -100 is 156,
// i5 -3 is 29, i9 -200 is 312.
test "prev of a signed input keeps its sign" {
    let dut = SignedPrev { };
    dut.sa = 255; // -1
    dut.sb = 0;
    step(3);
    // prev(sa) = prev(sa, 2) = -1, and -1 < 0. The unsigned register
    // compared 255 < 0.
    dut.chk = true;
    step(2);
    assert_eq(dut.y, 255);
}

test "prev of a signed struct field keeps its sign" {
    let dut = FieldPrev { };
    dut.xin = 5;
    dut.yin = 29; // -3
    dut.zin = 0;
    step(3);
    // p.y = -3 for two cycles, so prev(p.y) = prev(p).y = -3 < 0. The
    // unsigned register compared 29 < 0.
    dut.chk = true;
    step(2);
    assert_eq(dut.y, 29);
}

test "prev of a Trit keeps its sign" {
    let dut = TritPrev { };
    dut.down = true;
    step(3);
    // t = -1 (11). The edge after `down` falls loads t = 0; at the next
    // edge t = 0 and prev(t) = -1, and -1 < 0. The unsigned register
    // compared 3 < 0.
    dut.down = false;
    step(1);
    dut.chk = true;
    step(1);
    assert_eq(dut.o, 0);
}

test "prev of a sum has its natural width" {
    let dut = SumPrev { };
    dut.a = 200;
    dut.b = 100;
    dut.c = 300;
    dut.sa = 156; // -100
    dut.sb = 156; // -100
    dut.d = 312; // -200
    step(3);
    // a + b = 300 in nine bits, so prev(a + b) = prev(a + b, 2) = 300 = c;
    // sa + sb = -200 in nine signed bits = d. The eight-bit registers held
    // 44 and 56.
    dut.chk = true;
    step(2);
    assert_eq(dut.y, 200);
}
