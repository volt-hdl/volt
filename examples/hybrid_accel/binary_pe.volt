// Binary processing element: the same systolic cell as TernaryPe with
// a full 8-bit weight. The generated SV holds a real 8x8 multiplier --
// the cost the ternary cell avoids.
//
// The `: i16` target matters: an untyped `let product = weight * act`
// is i8 * i8 -> i8 and silently drops the high byte (ADR-0041 widens
// towards the declared target only). The "binary array accumulates"
// test pins this with 100 * 100.

package hybrid_accel::binary_pe;

/// |product| <= 128 * 128, so a column of 4 PEs stays inside +-65536.
pub const B_ACC_MAX : i32 = 65536
pub const B_ACC_MIN : i32 = -65536
const B_ACC_IN_MAX : i32 = 49152    // 3 PEs above, 16384 each
const B_ACC_IN_MIN : i32 = -49152

pub module BinaryPe {
    in  clk     : clock
    in  weight  : i8
    in  act     : i8
    in  acc_in  : i32
    out acc_out : i32
    out act_out : i8

    let product : i16 = weight * act    // 8x8 multiplier, full product

    reg acc   : i32 = 0
    reg act_r : i8  = 0

    on clk {
        acc   <= acc_in + (product as i32)
        act_r <= act
    }

    acc_out = acc
    act_out = act_r

    assume: acc_in >= B_ACC_IN_MIN && acc_in <= B_ACC_IN_MAX
    invariant: acc_out >= B_ACC_MIN && acc_out <= B_ACC_MAX
    invariant: prev(act) == act_out
    cover: acc_out != 0
}
