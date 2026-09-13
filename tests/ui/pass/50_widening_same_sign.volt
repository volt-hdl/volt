// Same-sign implicit widening under an explicit target type (ADR-0041).
// type-inference.md sec. 5: the expected type is pushed down into the
// arithmetic operands, so `a + b` below is added at i19 (no carry loss).

module WideningSameSign {
    in  clk : clock
    in  a   : i18
    in  b   : i19
    in  t   : i16
    in  c   : i16
    in  u   : u8
    out acc : i20
    out prod : i32
    out wide : i32
    out cnt  : u16

    // Operands of different widths are fine once the target is written.
    let sum : i20 = a + b
    // 16 x 16 product carried at 32 bits — no `as i32` needed.
    let p : i32 = t * c
    // Plain widening assignment.
    let w : i32 = t

    reg count : u16 = 0
    on clk {
        // u8 widened into a u16 register through an explicit target.
        count <= count + u
    }

    acc  = sum
    prod = p
    wide = w
    cnt  = count
}
