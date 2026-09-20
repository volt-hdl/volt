// 4x4 binary systolic array (ADR-0056: regular structures).
//
// Activations enter row y from the west (`act[y]`) and move one PE east
// per cycle; every column runs a sum from the north edge (0) to the
// south edge, each PE adding `weight * act` on the way. `acc[x]` is the
// sum leaving column x, `act_east[y]` the activation leaving row y, so
// arrays can be chained. `weight[y * BN + x]` belongs to PE (y, x).
//
// A value held on `act[y]` reaches column x after x cycles and the
// column sum needs BN more hops: with constant inputs `acc[x]` settles
// after x + BN cycles to sum over y of weight[y][x] * act[y].

package hybrid_accel::binary_array;

use hybrid_accel::binary_pe::{BinaryPe, B_ACC_MAX, B_ACC_MIN};

pub const BN : u32 = 4

pub module BinaryArray {
    in  clk      : clock
    in  weight   : [i8; BN * BN]
    in  act      : [i8; BN]
    out acc      : [i32; BN]
    out act_east : [i8; BN]

    // act_link[y * (BN + 1) + x] feeds PE (y, x) from the west, x = BN is
    // the east edge; acc_link[y * BN + x] feeds it from the north, row
    // BN is the south edge.
    wire act_link : [i8; BN * (BN + 1)]
    wire acc_link : [i32; (BN + 1) * BN]

    for y in 0..BN {
        act_link[y * (BN + 1)] = act[y]
        act_east[y] = act_link[y * (BN + 1) + BN]
    }
    for x in 0..BN {
        acc_link[x] = 0
        acc[x] = acc_link[BN * BN + x]
    }

    for y in 0..BN {
        for x in 0..BN {
            let pe = BinaryPe {
                clk: clk,
                weight: weight[y * BN + x],
                act: act_link[y * (BN + 1) + x],
                acc_in: acc_link[y * BN + x],
            }
            act_link[y * (BN + 1) + x + 1] = pe.act_out
            acc_link[(y + 1) * BN + x]   = pe.acc_out
        }
    }

    // Array-level view of the PE bound (contracts see ports only).
    invariant: acc[0] >= B_ACC_MIN && acc[0] <= B_ACC_MAX
    invariant: acc[BN - 1] >= B_ACC_MIN && acc[BN - 1] <= B_ACC_MAX
    cover: acc[BN - 1] != 0
}
