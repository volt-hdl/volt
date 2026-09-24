//~ E0003
// ADR-0077 Karar 2: arrays of structs are deferred (the right SV mapping
// is a packed element per entry — a separate ADR).
struct Entry {
    tag   : u4
    valid : bool
}

module M {
    in  clk : clock
    in  t   : u4
    out y   : bool

    reg entries : [Entry; 4] = [Entry { tag: 0, valid: false }; 4]
//~^ ERROR arrays of structs
    on clk { entries[0] <= Entry { tag: t, valid: true } }
    y = entries[1].valid
}
