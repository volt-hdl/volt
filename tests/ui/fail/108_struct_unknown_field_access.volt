//~ E1008
// ADR-0077 Karar 4: reading a field the struct does not have is an error
// (it was silently accepted before).
struct Cfg {
    enable : bool
    div    : u4
}

module M {
    in  cfg : Cfg
    out y   : u4

    y = cfg.divider
//~^ ERROR has no field 'divider'
}
