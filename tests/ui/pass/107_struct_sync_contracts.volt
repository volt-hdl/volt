// ADR-0077 Karar 6: a struct crosses a clock domain through sync() one
// field at a time (a 1-bit struct gets no W3003, like any 1-bit signal);
// contracts read fields, compare whole values and use prev().
domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

struct Flags {
    ok : bool
}

struct Pair {
    lo : u2
    hi : u2
}

module Cross {
    in  fast : clock @Fast
    in  slow : clock @Slow
    in  x    : u2 @Fast
    out ok   : bool @Slow

    reg f : Flags = Flags { ok: false }
    on fast { f.ok <= x != 0 }

    wire s : Flags
    s = sync(f, slow)
    ok = s.ok
}

module Keep {
    in  clk : clock
    in  lo  : u2
    out y   : u2

    reg p : Pair = Pair { lo: 0, hi: 0 }
    on clk {
        p.lo <= lo
        p.hi <= p.lo
    }

    invariant: p.hi <= 3
    cover: p == (Pair { lo: 1, hi: 1 })
    assert: prev(p).lo == p.hi

    y = p.hi
}

// Output net (ADR-0079):
//~ LINT-ALLOW: CMPCONST: 'p.hi <= 3' is always true for 2 bits; the fixture shows contract syntax on struct fields
