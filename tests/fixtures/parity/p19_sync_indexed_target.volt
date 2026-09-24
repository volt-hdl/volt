// parity: E0003
domain A { clock = posedge
 reset = sync active_high }
domain B { clock = posedge
 reset = sync active_high }
module M {
    in  ca : clock @A
    in  cb : clock @B
    in  x  : bool  @A
    out y  : bits<2> @B
    wire w : bits<2> @B
    w[0] = sync(x, cb)
    w[1] = false
    y = w
}
