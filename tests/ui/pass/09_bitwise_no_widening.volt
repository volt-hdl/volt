// Bit düzeyi operatörler genişlemez
module BitwiseOps {
    in  a : u8
    in  b : u8
    out and_r : u8
    out or_r  : u8
    out xor_r : u8
    out not_r : u8

    and_r = a & b
    or_r  = a | b
    xor_r = a ^ b
    not_r = ~a
}
