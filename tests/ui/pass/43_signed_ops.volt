// Signed operations (ADR-0036): arithmetic right shift on signed
// values, signed comparison, and same-width sign reinterpretation
// casts. The emitter maps these to `>>>`, `$signed()`/`$unsigned()`.
module SignedOps {
    in  a   : i32
    in  b   : i32
    in  u   : u32
    in  sh  : u8
    out asr : i32
    out lt  : bool
    out clt : bool
    out bck : u32

    // i32 >> n keeps the sign (arithmetic shift).
    asr = a >> (sh & 31)
    // Signed ports compare signed natively.
    lt  = a < b
    // Reinterpreting an unsigned value makes the compare signed.
    clt = (u as i32) < a
    // Round-trip: arithmetic shift inside, unsigned result outside.
    bck = ((u as i32) >> 2) as u32
}
