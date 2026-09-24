//~ E2014
// ADR-0077 Karar 4: each field is given exactly once.
struct Cfg {
    enable : bool
    div    : u4
}

module M {
    in  d : u4
    out y : u4

    let cfg : Cfg = Cfg { enable: true, div: d, div: 0 }
//~^ ERROR given twice
    y = cfg.div
}
