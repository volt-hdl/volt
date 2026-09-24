// parity: ok
type W = u8
type V = W
type C = clock
type A = [u4; 3]

module M {
    in  clk : C
    in  p : V
    in  q : A
    out y : W
    reg r : W = 0
    on clk { r <= p }
    y = r ^ q[1] as u8
}
