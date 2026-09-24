// parity: ok
// EMIT: --emit=sva
module M {
    in  clk : clock
    in  x   : u2
    out y   : u2
    reg r : u2 = 0
    on clk { r <= x }
    y = r
    invariant: match r { 0 => true, _ => true }
}
