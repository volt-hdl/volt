// Ternary processing element: weight in {-1, 0, +1}, 8-bit activation.
//
// The weight is a real `Trit` and the product uses the ternary MAC rule
// (type-inference.md sec. 3.3, `Trit * iN -> iN`). A Trit is stored as
// its two's-complement 2-bit code (ADR-0003: +1 = 01, 0 = 00, -1 = 11),
// and the emitter lowers `weight * act_w` to a select between `act_w`,
// `-act_w` and 0: the generated SV holds NO multiplier. The unused code
// 10 (-2) cannot come from Volt source (E2011 / E2009), but a formal
// tool drives the port freely, so an `assume` excludes it.
//
// Systolic flow: the activation moves one hop east per cycle, the
// running sum moves one hop south per cycle with this PE's product
// added on the way.

package hybrid_accel::ternary_pe;

/// |product| <= 128, so a column of 8 PEs stays inside +-1024.
pub const T_ACC_MAX : i32 = 1024
pub const T_ACC_MIN : i32 = -1024
const T_ACC_IN_MAX : i32 = 896      // 7 PEs above, 128 each
const T_ACC_IN_MIN : i32 = -896

pub module TernaryPe {
    in  clk     : clock
    in  weight  : Trit
    in  act     : i8
    in  acc_in  : i32
    out acc_out : i32
    out act_out : i8

    // 9 bits wide so that -(-128) = 128 does not wrap.
    let act_w   : i9 = act as i9
    let product : i9 = weight * act_w   // Trit * i9: a select, no multiplier

    reg acc   : i32 = 0
    reg act_r : i8  = 0

    on clk {
        acc   <= acc_in + (product as i32)
        act_r <= act
    }

    acc_out = acc
    act_out = act_r

    assume: (weight as i2) != -2
    // Overflow bound: the PEs above deliver at most seven products.
    assume: acc_in >= T_ACC_IN_MIN && acc_in <= T_ACC_IN_MAX
    invariant: acc_out >= T_ACC_MIN && acc_out <= T_ACC_MAX
    invariant: prev(act) == act_out
    cover: weight == 0
    cover: acc_out != 0
}
