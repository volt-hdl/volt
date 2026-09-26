// parity: E0003
fn low(v: u8) -> u4 { v[3:0] as u4 }
module M {
    in  xs : [u8; 2]
    out ys : [u4; 2]
    comb {
        for i in 0..2 {
            ys[i] = low(xs[i] + 1)
        }
    }
}
