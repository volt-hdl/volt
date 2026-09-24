//~ E2003
// ADR-0074: enum values cannot be ordered — the order is the encoding's,
// and with explicit values it differs from the declaration order.
enum Level { Low, Mid, High }

module M {
    in  clk : clock
    in  up  : bool
    out hot : bool

    reg lvl : Level = Level::Low

    on clk { if up { lvl <= Level::High } }

    hot = lvl > Level::Mid
//~^ ERROR cannot be ordered
}
