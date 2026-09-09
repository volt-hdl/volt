//~ E2003
// Implication operands must be Bool (ADR-0034): a -> b == !a || b.

module ImplicationNotBool {
    in  clk   : clock
    in  speed : u8
    in  start : bool
    out r     : u8

    invariant: speed -> start
    //~^ ERROR implication operand is not Bool
    r = speed
}
