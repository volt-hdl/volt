//~ W1002
// #128: a fn body that indexes with a root const `I`, inlined inside a comb
// `for I` loop, read the caller's loop variable instead of the const: the SV
// was `ys[k] = xs[k]` for every k. A fn body sees its parameters, its own
// lets and the unit's items; its parent scope is the root, not the caller
// (name-resolution.md §2 "Fonksiyon kapsamı", ADR-0081 Karar 12.1), so `I`
// in `pick` is the const 1 and `ys[k] = xs[1]` for every k. The loop
// variable `I` shadows the root const `I` inside the module: W1002
// (name-resolution.md §6). Fixed by #131 (one scope chain in the emitter).
const I : u32 = 1

fn pick(x: [u8; 4]) -> u8 { x[I] }

module M {
    in  x0 : u8
    in  x1 : u8
    in  x2 : u8
    in  x3 : u8
    out y0 : u8
    out y1 : u8
    out y2 : u8
    out y3 : u8
    wire xs : [u8; 4]
    wire ys : [u8; 4]
    comb {
        xs[0] = x0
        xs[1] = x1
        xs[2] = x2
        xs[3] = x3
        for I in 0..4 { ys[I] = pick(xs) }
    }
    y0 = ys[0]
    y1 = ys[1]
    y2 = ys[2]
    y3 = ys[3]
}

test "the fn body reads the const, not the loop variable" {
    let dut = M { };
    dut.x0 = 10;
    dut.x1 = 20;
    dut.x2 = 30;
    dut.x3 = 40;
    step(1);
    // pick(xs) = xs[I] with I = 1 (the root const) = x1 = 20, for every
    // iteration. The bug gave xs[k]: 10, 20, 30, 40.
    assert_eq(dut.y0, 20);
    assert_eq(dut.y1, 20);
    assert_eq(dut.y2, 20);
    assert_eq(dut.y3, 20);
}
