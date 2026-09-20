// Ternary processing element: weight in {-1, 0, +1}, 8-bit activation.
//
// The intended source is the ternary MAC rule (type-inference.md
// sec. 3.3): `in weight : Trit` and `let product = weight * act`. That
// type-checks (tests/ui/pass/16_trit_ternary_mac.volt) but the SV
// emitter has no mapping for `Trit` yet (E0003), so the trit is carried
// as its two's-complement `i2` encoding (the 2-bit storage the spec
// gives Trit) and the product is spelled out: a select between `act`,
// `-act` and 0. There is NO multiplier, here or in the generated SV.
// The fourth code (-2) is excluded by an `assume`.
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
    in  weight  : i2                // Trit encoding: -1, 0, +1
    in  act     : i8
    in  acc_in  : i32
    out acc_out : i32
    out act_out : i8

    // The negation is 9 bits wide: -(-128) = 128 does not wrap. Written
    // as `0 - x` because unary `-act` is emitted as `-9'(act)`, which
    // Yosys parses as a cast to size -9 ("Static cast with zero or
    // negative size"); Verilator accepts it.
    let act_w   : i9 = act as i9
    let act_neg : i9 = 0 - act_w
    let product : i9 = if weight == 1 { act_w } else { if weight == -1 { act_neg } else { 0 } }

    reg acc   : i32 = 0
    reg act_r : i8  = 0

    on clk {
        acc   <= acc_in + (product as i32)
        act_r <= act
    }

    acc_out = acc
    act_out = act_r

    assume: weight != -2
    // Overflow bound: the PEs above deliver at most seven products.
    assume: acc_in >= T_ACC_IN_MIN && acc_in <= T_ACC_IN_MAX
    invariant: acc_out >= T_ACC_MIN && acc_out <= T_ACC_MAX
    invariant: prev(act) == act_out
    cover: weight == 0
    cover: acc_out != 0
}
