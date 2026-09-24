//~ E2014
// ADR-0077 Karar 4: every bit needs an explicit source — a struct literal
// gives every field; an implicit zero would hide a forgotten field.
struct Cfg {
    enable : bool
    div    : u4
}

module M {
    in  clk : clock
    in  d   : u4
    out y   : u4

    reg cfg : Cfg = Cfg { div: 0 }
//~^ ERROR missing field(s) 'enable'
    on clk { cfg.div <= d }
    y = if cfg.enable { cfg.div } else { 0 }
}
