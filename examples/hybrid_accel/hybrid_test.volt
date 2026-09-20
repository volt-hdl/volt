// Simulation tests for the hybrid ternary/binary accelerator (ADR-0033).
//
// There is no sibling hybrid.volt: the designs come in through `use`
// (ADR-0042). Test scripts can only drive scalar `in` ports with
// non-negative literals, so three small benches live in this file:
//   TernaryPeTb     weight from two bools (minus / nonzero), act = -128
//                   from a bool
//   TernaryArrayTb  one weight and one activation broadcast to the grid
//   BinaryArrayTb   the same for the 4x4 binary grid
//   HybridTb        both broadcasts on HybridTop
//
// The harness drives every clock port from one generator, so t_clk and
// b_clk run lock-step 1:1 here (the 400/200 MHz ratio and real
// asynchrony are NOT exercised; the FIFO therefore never fills).
//
// Timing: a value driven before edge k is in a PE register after edge
// k. With constant inputs column x of an N-row array settles after
// x + N edges.

package hybrid_accel::hybrid_test;

use hybrid_accel::ternary_pe::TernaryPe;
use hybrid_accel::ternary_array::{TernaryArray, TN};
use hybrid_accel::binary_array::{BinaryArray, BN};
use hybrid_accel::hybrid_top::{HybridTop, TernaryCore, BinaryCore};

module TernaryPeTb {
    in  clk     : clock
    in  nonzero : bool
    in  minus   : bool
    in  act     : i8
    in  act_min : bool              // overrides act with -128
    in  acc_in  : i32
    out acc_out : i32
    out act_out : i8

    let weight : i2 = if nonzero { if minus { -1 } else { 1 } } else { 0 }
    let act_v  : i8 = if act_min { -128 } else { act }
    let pe = TernaryPe { clk: clk, weight: weight, act: act_v, acc_in: acc_in }
    acc_out = pe.acc_out
    act_out = pe.act_out
}

module TernaryArrayTb {
    in  clk    : clock
    in  minus  : bool
    in  act    : i8
    out col0   : i32
    out col7   : i32
    out east7  : i8

    wire w : [i2; TN * TN]
    wire a : [i8; TN]
    for i in 0..TN * TN { w[i] = if minus { -1 } else { 1 } }
    for i in 0..TN { a[i] = act }

    let arr = TernaryArray { clk: clk, weight: w, act: a }
    wire acc  : [i32; TN]
    wire east : [i8; TN]
    acc  = arr.acc
    east = arr.act_east
    col0  = acc[0]
    col7  = acc[TN - 1]
    east7 = east[TN - 1]
}

module BinaryArrayTb {
    in  clk    : clock
    in  weight : i8
    in  act    : i8
    out col0   : i32
    out col3   : i32
    out east3  : i8

    wire w : [i8; BN * BN]
    wire a : [i8; BN]
    for i in 0..BN * BN { w[i] = weight }
    for i in 0..BN { a[i] = act }

    let arr = BinaryArray { clk: clk, weight: w, act: a }
    wire acc  : [i32; BN]
    wire east : [i8; BN]
    acc  = arr.acc
    east = arr.act_east
    col0  = acc[0]
    col3  = acc[BN - 1]
    east3 = east[BN - 1]
}

module HybridTb {
    in  t_clk    : clock @TernaryCore
    in  t_act    : i8    @TernaryCore
    in  t_start  : bool  @TernaryCore
    out t_busy   : bool  @TernaryCore
    out t_full   : bool  @TernaryCore
    out both_busy: bool  @TernaryCore
    in  b_clk    : clock @BinaryCore
    in  b_weight : i8    @BinaryCore
    out b_busy   : bool  @BinaryCore
    out result   : i32   @BinaryCore

    wire tw : [i2; TN * TN]
    wire ta : [i8; TN]
    wire bw : [i8; BN * BN]
    for i in 0..TN * TN { tw[i] = 1 }
    for i in 0..TN { ta[i] = t_act }
    for i in 0..BN * BN { bw[i] = b_weight }

    let top = HybridTop {
        t_clk: t_clk, t_weight: tw, t_act: ta, t_start: t_start,
        b_clk: b_clk, b_weight: bw,
    }
    t_busy    = top.t_busy
    t_full    = top.t_full
    both_busy = top.both_busy
    b_busy    = top.b_busy
    result    = top.result
}

// ── TernaryPe ────────────────────────────────────────────────────

test "ternary pe plus one" {
    let dut = TernaryPeTb { };
    dut.nonzero = true;
    dut.act = 5;
    dut.acc_in = 100;
    step(1);
    assert_eq(dut.acc_out, 105);
    assert_eq(dut.act_out, 5);      // forwarded east one cycle later
}

test "ternary pe minus one" {
    let dut = TernaryPeTb { };
    dut.nonzero = true;
    dut.minus = true;
    dut.act = 5;
    dut.acc_in = 100;
    step(1);
    assert_eq(dut.acc_out, 95);
    dut.act_min = true;             // -(-128) = +128: the i9 product does not wrap
    step(1);
    assert_eq(dut.acc_out, 228);
}

test "ternary pe zero" {
    let dut = TernaryPeTb { };
    dut.act = 77;
    dut.acc_in = 100;
    step(1);
    assert_eq(dut.acc_out, 100);    // weight 0: the sum passes through
    dut.minus = true;               // minus without nonzero is still 0
    step(1);
    assert_eq(dut.acc_out, 100);
}

// ── Arrays ───────────────────────────────────────────────────────

test "ternary array accumulates" {
    let dut = TernaryArrayTb { };
    dut.act = 3;
    step(8);                        // column 0: 8 hops south
    assert_eq(dut.col0, 24);        // 8 rows * (+1 * 3)
    assert_eq(dut.col7, 3);         // only PE (7, 7) has seen its operand so far
    assert_eq(dut.east7, 3);
    step(7);                        // column 7: 7 hops east + 8 south
    assert_eq(dut.col7, 24);
    dut.act = 0;
    step(15);
    assert_eq(dut.col0, 0);
    assert_eq(dut.col7, 0);
}

test "binary array accumulates" {
    let dut = BinaryArrayTb { };
    dut.weight = 7;
    dut.act = 3;
    step(4);
    assert_eq(dut.col0, 84);        // 4 rows * 7 * 3
    assert_eq(dut.east3, 3);
    step(3);
    assert_eq(dut.col3, 84);
    dut.weight = 100;
    dut.act = 100;
    step(7);
    assert_eq(dut.col3, 40000);     // needs the full 16-bit product
}

// ── HybridTop (both clocks, lock-step) ───────────────────────────

test "cdc bridge transfers" {
    let dut = HybridTb { };
    dut.t_act = 1;
    dut.b_weight = 2;
    assert_false(dut.t_busy);
    dut.t_start = true;
    step(1);
    dut.t_start = false;
    assert_true(dut.t_busy);
    assert_false(dut.b_busy);
    step(16);                       // settle, then the push edge
    assert_false(dut.t_busy);
    assert_false(dut.t_full);
    step(5);                        // gray pointer sync + pop + window
    assert_true(dut.b_busy);
    assert_false(dut.both_busy);
    dut.t_start = true;             // second job while the binary layer works
    step(1);
    dut.t_start = false;
    step(2);                        // b_busy through the two-flop sync
    assert_true(dut.both_busy);
    // Layer output: 8 columns * 8 rows * (+1 * 1) = 64 -> window [64,0,0,0].
    // Binary layer: every column sums 2 * 64 = 128, four columns = 512.
    step(9);
    assert_false(dut.b_busy);
    assert_eq(dut.result, 512);
    step(30);                       // second word: window [64,64,0,0]
    assert_false(dut.t_busy);
    assert_false(dut.b_busy);
    assert_eq(dut.result, 1024);
}
