// FIR low-pass filter, generic in tap count and sample width (ADR-0041):
//
//   * `FirFilter<TAPS, WIDTH>` — shift line + combinational MAC written
//     as a `for` over the taps, 1-cycle latency. Monomorphised twice
//     below: `Fir8` wraps FirFilter<8, 16>, `Fir4` wraps FirFilter<4, 16>
//     (generated SV modules FirFilter_8_16 and FirFilter_4_16).
//   * `FirFilterPipe` — the 8-tap arithmetic under `pipeline(3)`
//     (ADR-0038): Multiply → Add1 → Add2, 3-cycle latency, `valid`
//     carried as a stage-local `let`.
//
// Coefficients are the symmetric low-pass kernel [1,2,3,4,4,3,2,1]
// (sum = 20, DC gain 20): |result| <= 20 * 32768 = 655 360 → 21 bits
// signed, carried at i32. No cast anywhere: `: i32` targets push the
// width down into the products (same-sign widening, ADR-0041 §1).

const TAPS   : u32 = 8
const COEFFS : [i16; 8] = [1, 2, 3, 4, 4, 3, 2, 1]
const RESULT_MAX : i32 = 655360

module FirFilter<const TAPS: u32, const WIDTH: u32> {
    in  clk       : clock
    in  sample    : sint<WIDTH>
    in  valid_in  : bool
    out result    : i32
    out valid_out : bool

    // Valid is delayed by exactly the tap-line register (ADR-0040 prev).
    invariant: !prev(valid_in) -> !valid_out
    // No overflow: the kernel bound holds for every tap count <= 8.
    invariant: result <= RESULT_MAX && result >= -655360
    cover: sample == 32767
    cover: sample == -32768
    cover: valid_out

    reg taps_r  : [sint<WIDTH>; TAPS] = [0; TAPS]
    reg valid_r : bool = false

    on clk {
        if valid_in {
            for i in 1..TAPS { taps_r[i] <= taps_r[i - 1] }
            taps_r[0] <= sample
        }
        valid_r <= valid_in
    }

    // MAC: `acc : i32` widens each 16x16 product to 32 bits.
    wire acc : i32
    comb {
        acc = 0
        for i in 0..TAPS { acc = acc + taps_r[i] * COEFFS[i] }
    }

    result    = acc
    valid_out = valid_r
}

// Concrete tops for simulation (test blocks name a concrete module).
module Fir8 {
    in  clk       : clock
    in  sample    : i16
    in  valid_in  : bool
    out result    : i32
    out valid_out : bool

    let f = FirFilter<8, 16> { clk, sample, valid_in }
    result    = f.result
    valid_out = f.valid_out
}

module Fir4 {
    in  clk       : clock
    in  sample    : i16
    in  valid_in  : bool
    out result    : i32
    out valid_out : bool

    let f = FirFilter<4, 16> { clk, sample, valid_in }
    result    = f.result
    valid_out = f.valid_out
}

// Pipelined variant. Stage delays: Multiply 0, Add1 1, Add2 2; the tap
// line in front adds one more, so sample→result is 3 cycles.
pipeline(3) FirFilterPipe {
    in  clk       : clock
    in  sample    : i16
    in  valid_in  : bool
    out result    : i32
    out valid_out : bool

    invariant: !prev(valid_in, 3) -> !valid_out
    invariant: result <= RESULT_MAX && result >= -655360
    // Induction helpers (`--mode prove`): per-tap bound |Ck| * 32768.
    invariant: stage(Add1).p0 <= 32768  && stage(Add1).p0 >= -32768
    invariant: stage(Add1).p1 <= 65536  && stage(Add1).p1 >= -65536
    invariant: stage(Add1).p2 <= 98304  && stage(Add1).p2 >= -98304
    invariant: stage(Add1).p3 <= 131072 && stage(Add1).p3 >= -131072
    invariant: stage(Add1).p4 <= 131072 && stage(Add1).p4 >= -131072
    invariant: stage(Add1).p5 <= 98304  && stage(Add1).p5 >= -98304
    invariant: stage(Add1).p6 <= 65536  && stage(Add1).p6 >= -65536
    invariant: stage(Add1).p7 <= 32768  && stage(Add1).p7 >= -32768
    invariant: stage(Add2).s01 <= 98304  && stage(Add2).s01 >= -98304
    invariant: stage(Add2).s23 <= 229376 && stage(Add2).s23 >= -229376
    invariant: stage(Add2).s45 <= 229376 && stage(Add2).s45 >= -229376
    invariant: stage(Add2).s67 <= 98304  && stage(Add2).s67 >= -98304
    cover: valid_out
    cover: sample == 32767

    reg taps_r  : [i16; TAPS] = [0; TAPS]
    reg valid_r : bool = false

    stage Multiply {
        valid_r <= valid_in
        if valid_in {
            for i in 1..TAPS { taps_r[i] <= taps_r[i - 1] }
            taps_r[0] <= sample
        }
        // Everything read here is one register behind `sample`.
        let v  : bool = valid_r
        let p0 : i32 = taps_r[0] * COEFFS[0]
        let p1 : i32 = taps_r[1] * COEFFS[1]
        let p2 : i32 = taps_r[2] * COEFFS[2]
        let p3 : i32 = taps_r[3] * COEFFS[3]
        let p4 : i32 = taps_r[4] * COEFFS[4]
        let p5 : i32 = taps_r[5] * COEFFS[5]
        let p6 : i32 = taps_r[6] * COEFFS[6]
        let p7 : i32 = taps_r[7] * COEFFS[7]
    }
    stage Add1 {
        let s01 : i32 = p0 + p1
        let s23 : i32 = p2 + p3
        let s45 : i32 = p4 + p5
        let s67 : i32 = p6 + p7
    }
    stage Add2 {
        let sum : i32 = (s01 + s23) + (s45 + s67)
    }

    result    = stage(Add2).sum
    valid_out = stage(Add2).v
}
