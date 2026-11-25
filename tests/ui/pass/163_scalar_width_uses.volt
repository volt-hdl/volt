// #419: every scalar type has one bit width wherever it is used. The
// width of a type comes from one function (TypeArena::bit_width, with the
// scalar table volt_ast::bit_width::Scalar): bool 1, Trit 2 (ADR-0003),
// uN/iN/bits<N> N, an enum its encoding width (ADR-0074), a struct its
// layout width W (ADR-0077), an alias its target's (ADR-0070 §6). Each
// module uses one type T as a ShiftRegister element (taps is
// bits<LEN * width(T)>, ADR-0029; data_out is T), an array element
// (ADR-0056), a struct field (ADR-0077), a port and a Delayed<T, 1>
// register (ADR-0037; @strict_timing checks the declared delay). The
// widths of the taps ports below are LEN = 4 times the width of T,
// written by hand. Arrays of enums and structs have no SV mapping yet
// (E0003), so those three modules have no array port. Before #419 the
// taps of ShiftRegister<Trit, 4> was bits<4> in the type checker and 8
// bits in the SV; this file is an input of the width shadow (ADR-0136),
// which compares the two on every expression.

enum Two {
    Off,
    On,
}

enum Dir {
    Hold,
    Up,
    Down,
}

struct Pair {
    t  : Trit
    ok : bool
}

type Tr = Trit

struct HoldBool {
    f  : bool
    ok : bool
}

@strict_timing
module UseBool {
    in  clk    : clock
    in  d      : bool
    in  strobe : bool
    in  arr    : [bool; 3]
    in  h      : HoldBool
    out window : bits<4>
    out oldest : bool
    out third  : bool
    out field  : bool
    out ok     : bool
    out late   : Delayed<bool, 1>

    let sr = ShiftRegister<bool, 4> { clk: clk, data_in: d, shift_en: strobe }

    reg r : Delayed<bool, 1> = false
    on clk {
        r <= d
    }

    window = sr.taps
    oldest = sr.data_out
    third  = arr[2]
    field  = h.f
    ok     = h.ok
    late   = r
}

struct HoldTrit {
    f  : Trit
    ok : bool
}

@strict_timing
module UseTrit {
    in  clk    : clock
    in  d      : Trit
    in  strobe : bool
    in  arr    : [Trit; 3]
    in  h      : HoldTrit
    out window : bits<8>
    out oldest : Trit
    out third  : Trit
    out field  : Trit
    out ok     : bool
    out late   : Delayed<Trit, 1>

    let sr = ShiftRegister<Trit, 4> { clk: clk, data_in: d, shift_en: strobe }

    reg r : Delayed<Trit, 1> = 0
    on clk {
        r <= d
    }

    window = sr.taps
    oldest = sr.data_out
    third  = arr[2]
    field  = h.f
    ok     = h.ok
    late   = r
}

struct HoldU3 {
    f  : u3
    ok : bool
}

@strict_timing
module UseU3 {
    in  clk    : clock
    in  d      : u3
    in  strobe : bool
    in  arr    : [u3; 3]
    in  h      : HoldU3
    out window : bits<12>
    out oldest : u3
    out third  : u3
    out field  : u3
    out ok     : bool
    out late   : Delayed<u3, 1>

    let sr = ShiftRegister<u3, 4> { clk: clk, data_in: d, shift_en: strobe }

    reg r : Delayed<u3, 1> = 0
    on clk {
        r <= d
    }

    window = sr.taps
    oldest = sr.data_out
    third  = arr[2]
    field  = h.f
    ok     = h.ok
    late   = r
}

struct HoldI5 {
    f  : i5
    ok : bool
}

@strict_timing
module UseI5 {
    in  clk    : clock
    in  d      : i5
    in  strobe : bool
    in  arr    : [i5; 3]
    in  h      : HoldI5
    out window : bits<20>
    out oldest : i5
    out third  : i5
    out field  : i5
    out ok     : bool
    out late   : Delayed<i5, 1>

    let sr = ShiftRegister<i5, 4> { clk: clk, data_in: d, shift_en: strobe }

    reg r : Delayed<i5, 1> = 0
    on clk {
        r <= d
    }

    window = sr.taps
    oldest = sr.data_out
    third  = arr[2]
    field  = h.f
    ok     = h.ok
    late   = r
}

struct HoldB6 {
    f  : bits<6>
    ok : bool
}

@strict_timing
module UseB6 {
    in  clk    : clock
    in  d      : bits<6>
    in  strobe : bool
    in  arr    : [bits<6>; 3]
    in  h      : HoldB6
    out window : bits<24>
    out oldest : bits<6>
    out third  : bits<6>
    out field  : bits<6>
    out ok     : bool
    out late   : Delayed<bits<6>, 1>

    let sr = ShiftRegister<bits<6>, 4> { clk: clk, data_in: d, shift_en: strobe }

    reg r : Delayed<bits<6>, 1> = 0 as bits<6>
    on clk {
        r <= d
    }

    window = sr.taps
    oldest = sr.data_out
    third  = arr[2]
    field  = h.f
    ok     = h.ok
    late   = r
}

struct HoldTwo {
    f  : Two
    ok : bool
}

@strict_timing
module UseTwo {
    in  clk    : clock
    in  d      : Two
    in  strobe : bool
    in  h      : HoldTwo
    out window : bits<4>
    out oldest : Two
    out field  : Two
    out ok     : bool
    out late   : Delayed<Two, 1>

    let sr = ShiftRegister<Two, 4> { clk: clk, data_in: d, shift_en: strobe }

    reg r : Delayed<Two, 1> = Two::Off
    on clk {
        r <= d
    }

    window = sr.taps
    oldest = sr.data_out
    field  = h.f
    ok     = h.ok
    late   = r
}

struct HoldDir {
    f  : Dir
    ok : bool
}

@strict_timing
module UseDir {
    in  clk    : clock
    in  d      : Dir
    in  strobe : bool
    in  h      : HoldDir
    out window : bits<8>
    out oldest : Dir
    out field  : Dir
    out ok     : bool
    out late   : Delayed<Dir, 1>

    let sr = ShiftRegister<Dir, 4> { clk: clk, data_in: d, shift_en: strobe }

    reg r : Delayed<Dir, 1> = Dir::Hold
    on clk {
        r <= d
    }

    window = sr.taps
    oldest = sr.data_out
    field  = h.f
    ok     = h.ok
    late   = r
}

struct HoldPair {
    f  : Pair
    ok : bool
}

@strict_timing
module UsePair {
    in  clk    : clock
    in  d      : Pair
    in  strobe : bool
    in  h      : HoldPair
    out window : bits<12>
    out oldest : Pair
    out field  : Pair
    out ok     : bool
    out late   : Delayed<Pair, 1>

    let sr = ShiftRegister<Pair, 4> { clk: clk, data_in: d, shift_en: strobe }

    reg r : Delayed<Pair, 1> = Pair { t: 0, ok: false }
    on clk {
        r <= d
    }

    window = sr.taps
    oldest = sr.data_out
    field  = h.f
    ok     = h.ok
    late   = r
}

struct HoldTr {
    f  : Tr
    ok : bool
}

@strict_timing
module UseTr {
    in  clk    : clock
    in  d      : Tr
    in  strobe : bool
    in  arr    : [Tr; 3]
    in  h      : HoldTr
    out window : bits<8>
    out oldest : Tr
    out third  : Tr
    out field  : Tr
    out ok     : bool
    out late   : Delayed<Tr, 1>

    let sr = ShiftRegister<Tr, 4> { clk: clk, data_in: d, shift_en: strobe }

    reg r : Delayed<Tr, 1> = 0
    on clk {
        r <= d
    }

    window = sr.taps
    oldest = sr.data_out
    third  = arr[2]
    field  = h.f
    ok     = h.ok
    late   = r
}
