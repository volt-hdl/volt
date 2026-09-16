// Nested module-level `for` loops (ADR-0056): a 2x2 grid of cells.
//
// Instances are named `cell_<y>_<x>` (outer index first); link wires
// are indexed with constant arithmetic on the loop variables, which the
// unroller folds (`(0 + 1) * 2 + 1` -> `3`). A `for` in an `on` block
// is still unrolled by the SV emitter, so both forms coexist.

const W : u32 = 2
const H : u32 = 2

module Cell {
    in  clk : clock
    in  x_in : u4
    in  y_in : u4
    out x_out : u4
    out y_out : u4

    reg xr : u4 = 0
    reg yr : u4 = 0
    on clk {
        xr <= x_in
        yr <= y_in
    }
    x_out = xr
    y_out = yr
}

module Grid {
    in  clk   : clock
    in  left  : [u4; H]
    in  top   : [u4; W]
    out right : [u4; H]
    out bottom: [u4; W]

    wire hlink : [u4; H * (W + 1)]
    wire vlink : [u4; (H + 1) * W]

    for y in 0..H {
        hlink[y * (W + 1)] = left[y]
        right[y] = hlink[y * (W + 1) + W]
    }
    for x in 0..W {
        vlink[x] = top[x]
        bottom[x] = vlink[H * W + x]
    }
    for y in 0..H {
        for x in 0..W {
            let cell = Cell {
                clk: clk,
                x_in: hlink[y * (W + 1) + x],
                y_in: vlink[y * W + x],
            }
            hlink[y * (W + 1) + x + 1] = cell.x_out
            vlink[(y + 1) * W + x]     = cell.y_out
        }
    }
}
