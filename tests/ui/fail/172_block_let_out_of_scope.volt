//~ E1001
// ADR-0083 Karar 8: blok let'inin kapsamı içinde bulunduğu { } —
// if dalında bildirilen ad dal dışında görünmez.

module LetOutOfScope {
    in  clk : clock
    in  c   : bool
    in  a   : u8
    out q   : u8

    reg r : u8 = 0
    on clk {
        if c {
            let t = a + 1
            r <= t
        }
        r <= t
        //~^ ERROR undefined name: 't'
    }
    q = r
}
