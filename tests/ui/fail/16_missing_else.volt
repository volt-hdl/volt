//~ E0008
// if ifadesinde else eksik — latch riski.

module MissingElse {
    in  cond : bool
    in  a    : u8
    out r    : u8

    r = if cond { a }
    //~^ ERROR 'if' ifadesinde 'else' dalı zorunlu
}
