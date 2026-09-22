// Hybrid accelerator: an 8x8 ternary layer at 400 MHz feeding a 4x4
// binary layer at 200 MHz.
//
//   HybridTop
//    ├── t_clk (TernaryCore)  TernaryArray 8x8, column sums -> t_sum,
//    │       │                job counter, push when settled
//    │       │  AsyncFifo<i32, 16>   (the only data crossing)
//    └── b_clk (BinaryCore)   pop, saturate to i8, 4-sample window,
//                             BinaryArray 4x4, column sums -> result
//
// Clock-domain crossings (all explicit):
//   ternary -> binary  layer output (32 bits)  AsyncFifo<i32, 16>
//   binary -> ternary  b_busy (1 bit, level)   sync()
//
// The per-domain control lives in two single-clock modules, TernaryCtl
// and BinaryFront, so their register invariants are proven by the
// single-clock formal flow (user invariants on registers of a two-clock
// module hit the wrapper's reset-ordering gap, see vga/README.md).
//
// A ternary job: the host holds `t_act` / `t_weight`, pulses `t_start`;
// T_SETTLE cycles later (7 hops east + 8 south + the sum register) the
// layer output is pushed. A full FIFO stalls the job in its last cycle
// instead of dropping the word, so the FIFO cannot overflow.

package hybrid_accel::hybrid_top;

use hybrid_accel::ternary_array::{TernaryArray, TN};
use hybrid_accel::binary_array::{BinaryArray, BN};

pub domain TernaryCore {
    clock = posedge,
    reset = sync active_high,
    frequency = 400.mhz,
}

pub domain BinaryCore {
    clock = posedge,
    reset = sync active_high,
    frequency = 200.mhz,
}

const T_SETTLE : u5 = 16    // 7 + 8 + 1
const B_SETTLE : u4 = 8     // 3 + 4 + 1

/// Ternary job control: `start` opens a job, `push` fires in its last
/// cycle once the FIFO has room.
module TernaryCtl {
    in  clk   : clock
    in  start : bool
    in  full  : bool
    out busy  : bool
    out push  : bool

    reg cnt : u5 = 0
    let last : bool = cnt == 1

    on clk {
        if cnt == 0 {
            if start { cnt <= T_SETTLE }
        } else {
            if !last || !full { cnt <= cnt - 1 }
        }
    }

    busy = cnt != 0
    push = last && !full

    invariant: cnt <= T_SETTLE
    // The FIFO never overflows: a push only happens with room ...
    invariant: push -> !full
    // ... and a full FIFO stalls the job instead of dropping the word.
    invariant: (prev(cnt) == 1 && prev(full)) -> cnt == 1
    cover: push
    cover: busy && full
}

/// Binary front end: a popped word is saturated to i8 and shifted into
/// the BN-sample window that feeds the binary array rows.
module BinaryFront {
    in  clk   : clock
    in  empty : bool
    in  word  : i32                 // FIFO rd_data: valid the cycle after a pop
    out act   : [i8; BN]
    out busy  : bool

    reg pop_r  : bool = false
    reg win    : [i8; BN] = [0; BN]
    reg cnt    : u4   = 0
    reg busy_r : bool = false

    let sample : i8 = if word > 127 { 127 } else { if word < -128 { -128 } else { word[7:0] as i8 } }

    on clk {
        pop_r <= !empty
        if pop_r {
            for i in 1..BN { win[i] <= win[i - 1] }
            win[0] <= sample
            cnt    <= B_SETTLE
            busy_r <= true
        } else {
            if cnt != 0 {
                cnt <= cnt - 1
                if cnt == 1 { busy_r <= false }
            }
        }
    }

    // A `reg` array is an unpacked SV array while array ports are packed
    // vectors (ADR-0056), so the window leaves through element copies.
    for i in 0..BN { act[i] = win[i] }
    busy = busy_r

    invariant: cnt <= B_SETTLE
    invariant: busy_r == (cnt != 0)
    invariant: prev(pop_r) -> busy
    cover: busy
    cover: prev(busy) && !busy
}

pub module HybridTop {
    // Reset, taken raw (ADR-0065): released synchronously to t_clk and
    // to b_clk by a two-stage chain each; every single-clock child gets
    // the chain of the clock it is bound to.
    in  rst        : reset(sync, active_high)

    // ── Ternary domain ────────────────────────────────────────────
    in  t_clk      : clock @TernaryCore
    in  t_weight   : [Trit; TN * TN] @TernaryCore
    in  t_act      : [i8; TN]      @TernaryCore
    in  t_start    : bool          @TernaryCore
    out t_busy     : bool          @TernaryCore
    out t_push     : bool          @TernaryCore   // a word enters the FIFO
    out t_full     : bool          @TernaryCore
    out t_act_east : [i8; TN]      @TernaryCore   // for chaining
    out both_busy  : bool          @TernaryCore

    // ── Binary domain ─────────────────────────────────────────────
    in  b_clk      : clock @BinaryCore
    in  b_weight   : [i8; BN * BN] @BinaryCore
    out b_busy     : bool          @BinaryCore
    out b_act_east : [i8; BN]      @BinaryCore
    out result     : i32           @BinaryCore

    // The FIFO never overflows. The inductive proof of this is
    // TernaryCtl's (`push -> !full`, single clock); here it is restated
    // on the ports and the AsyncFifo occupancy invariant rides along.
    // Two-clock modules only get `bmc`: under `multiclock on` the wrapper
    // drops rst before the first clock edge (vga/README.md), and even
    // the tautology `both_busy -> t_busy` "fails at cycle 2" from that
    // random state, the two sides being sampled around the edge.
    invariant: t_push -> !t_full
    cover: t_push
    cover: both_busy

    // ── Ternary layer ─────────────────────────────────────────────
    let ta = TernaryArray { clk: t_clk, weight: t_weight, act: t_act }
    t_act_east = ta.act_east

    // Indexing an instance's array output directly (`ta.acc[x]`) emits a
    // 1-bit select `ta_acc[x]` instead of `ta_acc[32*x +: 32]` (caught
    // by Verilator UNUSEDSIGNAL); a wire array in between is correct.
    wire t_acc : [i32; TN]
    t_acc = ta.acc

    wire t_total : i32
    comb {
        t_total = 0
        for x in 0..TN { t_total = t_total + t_acc[x] }
    }

    reg t_sum : i32 = 0
    on t_clk { t_sum <= t_total }

    wire fifo_full : bool
    let ctl = TernaryCtl { clk: t_clk, start: t_start, full: fifo_full }

    // ── The crossing ──────────────────────────────────────────────
    // Binding `word: t_sum` on the binary side instead is E3001 (tried:
    // the checker names both domains and suggests AsyncFifo).
    let fifo = AsyncFifo<i32, 16> {
        wr_clk:  t_clk,
        wr_data: t_sum,
        wr_en:   ctl.push,
        rd_clk:  b_clk,
        rd_en:   true,              // ignored while empty
    }
    fifo_full = fifo.wr_full

    t_busy = ctl.busy
    t_push = ctl.push
    t_full = fifo.wr_full

    // ── Binary layer ──────────────────────────────────────────────
    let bf = BinaryFront { clk: b_clk, empty: fifo.rd_empty, word: fifo.rd_data }
    let ba = BinaryArray { clk: b_clk, weight: b_weight, act: bf.act }
    b_act_east = ba.act_east

    wire b_acc : [i32; BN]
    b_acc = ba.acc

    wire b_total : i32
    comb {
        b_total = 0
        for x in 0..BN { b_total = b_total + b_acc[x] }
    }

    reg b_sum : i32 = 0
    on b_clk { b_sum <= b_total }

    b_busy = bf.busy
    result = b_sum

    // binary -> ternary: BinaryFront.busy is a registered level, 1 bit.
    wire b_busy_s : bool
    b_busy_s  = sync(b_busy, t_clk)
    both_busy = t_busy && b_busy_s
}
