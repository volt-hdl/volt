// let binding: intermediate values, type inference
// let → emitted as a wire in SV
module LetBinding {
    in  a : u8
    in  b : u8
    in  c : u8
    out y : u16

    // Type inference: u8 + u8 → u9
    let sum = a + b

    // After widening
    let widened = sum as u16

    // Chained usage
    let scaled = widened * (c as u16)

    y = scaled
}
