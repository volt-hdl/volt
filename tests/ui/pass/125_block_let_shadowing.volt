// ADR-0083 Karar 8/11: blok let'i iç kapsamda modül adını ve dış blok
// let'ini gölgeleyebilir (W1002, name-resolution.md §6); SV'de ad
// yakalamasını önlemek için yerel yeniden adlandırılır (a_2, t_2).

module BlockLetShadowing {
    in  clk : clock
    in  a   : u8
    in  b   : u8
    in  c   : bool
    out q   : u8

    reg r : u8 = 0
    on clk {
        let t = a
        if c {
            let t = b
            let a = t + 1
            r <= a
        } else {
            r <= t
        }
    }
    q = r
}

// Struct tipli blok let'i yapraklara indirgenir; her yaprak ayrı yerel
// (yapraklar aynı bildirimden gelir, adları farklıdır).
struct Pair {
    x : u8
    f : bool
}

module BlockLetStruct {
    in  clk : clock
    in  a   : u8
    in  c   : bool
    out q   : u8

    reg r : u8 = 0
    on clk {
        let p : Pair = Pair { x: a, f: c }
        if p.f {
            r <= p.x
        }
    }
    q = r
}
