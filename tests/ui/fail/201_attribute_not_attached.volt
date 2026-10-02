//~ W0024
// An attribute before a contract was parsed and dropped without a word:
// contracts take no attributes (grammar-full.ebnf §2), so the user
// believed the automatic FSM contracts were turned off.

module AttributeNotAttached {
    in  clk : clock
    out q   : u8

    @no_auto_contracts
    //~^ ERROR attribute '@no_auto_contracts' is not attached to anything and is ignored
    invariant: q < 200

    reg r : u8 = 0
    on clk { r <= r + 1 }
    q = r
}
