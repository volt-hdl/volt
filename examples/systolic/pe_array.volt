// 4x4 output-stationary systolic array (ADR-0056: regular structures).
//
// Each processing element multiplies the operand arriving from the west
// (`a`) by the one arriving from the north (`b`), accumulates the product
// and forwards both operands one hop east / south a cycle later. Feeding
// row y of A with a skew of y cycles and column x of B with a skew of x
// cycles computes the 4x4 matrix product C = A * B in place; `clr`
// resets every accumulator for the next product.
//
// The grid is written ONCE, as two nested compile-time `for` loops: the
// compiler unrolls the loops, names the instances `pe_<y>_<x>` and wires
// the link arrays element by element. Hand-written, the same 16 PEs and
// 64 bindings would be about 400 lines.

const N : u32 = 4

/// One processing element: a_out/b_out are the delayed operands, c the
/// running sum. `clr` empties the accumulator.
module Pe {
    in  clk   : clock
    in  clr   : bool
    in  a_in  : i8
    in  b_in  : i8
    out a_out : i8
    out b_out : i8
    out c     : i16

    reg a_r : i8  = 0
    reg b_r : i8  = 0
    reg acc : i16 = 0

    on clk {
        a_r <= a_in
        b_r <= b_in
        if clr {
            acc <= 0
        } else {
            acc <= acc + (a_in as i16) * (b_in as i16)
        }
    }

    a_out = a_r
    b_out = b_r
    c     = acc

    invariant: prev(clr) -> c == 0
    invariant: prev(a_in) == a_out
    cover: c != 0
}

/// N x N grid. `a[y]` enters row y from the west, `b[x]` enters column x
/// from the north; `c[y * N + x]` is the accumulator of PE (y, x). The
/// operands leaving the east and south edges are exposed so the array
/// can be chained.
module PeArray {
    in  clk    : clock
    in  clr    : bool
    in  a      : [i8; N]
    in  b      : [i8; N]
    out c      : [i16; N * N]
    out a_east : [i8; N]
    out b_south: [i8; N]

    // Horizontal links: a_link[y * (N + 1) + x] feeds PE (y, x) from the
    // west; index x = N is the east edge. Vertical links the same way.
    wire a_link : [i8; N * (N + 1)]
    wire b_link : [i8; (N + 1) * N]

    for y in 0..N {
        a_link[y * (N + 1)] = a[y]
        a_east[y] = a_link[y * (N + 1) + N]
    }
    for x in 0..N {
        b_link[x] = b[x]
        b_south[x] = b_link[N * N + x]
    }

    for y in 0..N {
        for x in 0..N {
            let pe = Pe {
                clk: clk,
                clr: clr,
                a_in: a_link[y * (N + 1) + x],
                b_in: b_link[y * N + x],
            }
            a_link[y * (N + 1) + x + 1] = pe.a_out
            b_link[(y + 1) * N + x]     = pe.b_out
            c[y * N + x]                = pe.c
        }
    }

    // The grid inherits the PE contracts 16 times over; these state the
    // array-level view of the same facts (contracts see ports only).
    invariant: prev(clr) -> c[0] == 0 && c[N * N - 1] == 0
    cover: c[N * N - 1] != 0
}
