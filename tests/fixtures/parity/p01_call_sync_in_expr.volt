// parity: E0003
domain A { clock = posedge
 reset = sync active_high }
domain B { clock = posedge
 reset = sync active_high }
module M {
    in  ca : clock @A
    in  cb : clock @B
    in  x  : bool  @A
    out y  : bool  @B
    y = !sync(x, cb)
}
