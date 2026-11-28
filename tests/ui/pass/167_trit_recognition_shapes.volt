// #463 (ADR-0145): the SV emitter recognizes a Trit by syntax (`is_trit`)
// and reads the width of a Trit product and of `-t` from the type checker.
// This file is input to the width shadow (ADR-0136): it puts a Trit in the
// places the recognizer has to know, each with a Trit product and a `-t`
// kept in an untyped let: a struct field, an array element, a fn parameter,
// a generic const parameter (+1 and -1), a `comb` local and an `on` local.
// type-inference.md sec. 3.3: `Trit * iN` is iN; sec. 3.4: `-t` is a Trit,
// `-z` (z : iN) is i(N+1).

struct Weight {
    t  : Trit
    ok : bool
}

fn scale(t : Trit, x : i8) -> i8 {
    let p = t * x
    p
}

fn flip(t : Trit, x : i8) -> i9 {
    let m = -(t * x)
    m
}

fn negt(t : Trit) -> Trit {
    let n = -t
    n
}

// The parser turns a generic const parameter into its literal (ADR-0041);
// the literal of a `const S : Trit` keeps the Trit type (#468), so `S * x`
// is a Trit product (i8), `-S` is a Trit and `-(S * x)` is i9, as with a
// Trit signal. `Scaled<-1>` is written with a minus (#471).
module Scaled<const S: Trit> {
    in  x : i8
    in  c : bool
    out y : i8
    out z : Trit
    out m : i9
    out v : i8
    out w : Trit

    let p = S * x
    y = p
    let n = -S
    z = n
    let q = -(S * x)
    m = q
    let r = -S * x
    v = r
    let s = if c { S } else { -S }
    w = s
}

module TritShapes {
    in  clk     : clock
    in  w       : Weight
    in  ws      : [Trit; 2]
    in  x       : i8
    in  tp      : bool
    in  tn      : bool
    out o_field : i8
    out n_field : Trit
    out o_elem  : i8
    out n_elem  : Trit
    out o_fn    : i8
    out m_fn    : i9
    out n_fn    : Trit
    out o_gen   : i8
    out n_gen   : Trit
    out m_gen   : i9
    out v_gen   : i8
    out w_gen   : Trit
    out o_comb  : i8
    out m_comb  : i9
    out n_comb  : Trit
    out o_on    : i8

    // A struct field.
    let pf = w.t * x
    o_field = pf
    let nf = -w.t
    n_field = nf

    // An array element.
    let pe = ws[1] * x
    o_elem = pe
    let ne = -ws[0]
    n_elem = ne

    // A fn parameter.
    o_fn = scale(w.t, x)
    m_fn = flip(ws[0], x)
    n_fn = negt(ws[1])

    // A generic const parameter, +1 and -1.
    let g = Scaled<1> { x: x, c: tp }
    let h = Scaled<-1> { x: x, c: tn }
    o_gen = g.y
    n_gen = h.z
    m_gen = h.m
    v_gen = g.v
    w_gen = h.w

    // A comb local.
    comb {
        let tc : Trit = if tp { 1 } else if tn { -1 } else { 0 }
        let pc = tc * x
        o_comb = pc
        let mc = -(tc * x)
        m_comb = mc
        let nc = -tc
        n_comb = nc
    }

    // An on local.
    reg r : i8 = 0
    on clk {
        let to : Trit = if tp { 1 } else if tn { -1 } else { 0 }
        let po = to * x
        r <= po
    }
    o_on = r
}
