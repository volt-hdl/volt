// 8x8 ternary systolic array (ADR-0056: regular structures).
//
// Activations enter row y from the west (`act[y]`) and move one PE east
// per cycle; every column runs a sum from the north edge (0) to the
// south edge, each PE adding `weight * act` on the way. `acc[x]` is the
// sum leaving column x, `act_east[y]` the activation leaving row y, so
// arrays can be chained. `weight[y * TN + x]` belongs to PE (y, x).
//
// A value held on `act[y]` reaches column x after x cycles and the
// column sum needs TN more hops: with constant inputs `acc[x]` settles
// after x + TN cycles to sum over y of weight[y][x] * act[y].

package hybrid_accel::ternary_array;

use hybrid_accel::ternary_pe::{TernaryPe, T_ACC_MAX, T_ACC_MIN};

pub const TN : u32 = 8

pub module TernaryArray {
    in  clk      : clock
    in  weight   : [i2; TN * TN]     // Trit encoding, see ternary_pe.volt
    in  act      : [i8; TN]
    out acc      : [i32; TN]
    out act_east : [i8; TN]

    // act_link[y * (TN + 1) + x] feeds PE (y, x) from the west, x = TN is
    // the east edge; acc_link[y * TN + x] feeds it from the north, row
    // TN is the south edge.
    wire act_link : [i8; TN * (TN + 1)]
    wire acc_link : [i32; (TN + 1) * TN]

    for y in 0..TN {
        act_link[y * (TN + 1)] = act[y]
        act_east[y] = act_link[y * (TN + 1) + TN]
    }
    for x in 0..TN {
        acc_link[x] = 0
        acc[x] = acc_link[TN * TN + x]
    }

    for y in 0..TN {
        for x in 0..TN {
            let pe = TernaryPe {
                clk: clk,
                weight: weight[y * TN + x],
                act: act_link[y * (TN + 1) + x],
                acc_in: acc_link[y * TN + x],
            }
            act_link[y * (TN + 1) + x + 1] = pe.act_out
            acc_link[(y + 1) * TN + x]   = pe.acc_out
        }
    }

    // Array-level view of the PE bound (contracts see ports only).
    invariant: acc[0] >= T_ACC_MIN && acc[0] <= T_ACC_MAX
    invariant: acc[TN - 1] >= T_ACC_MIN && acc[TN - 1] <= T_ACC_MAX
    cover: acc[TN - 1] != 0
}
