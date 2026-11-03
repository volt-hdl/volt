//~ E2031
// The wide side is a constant that no u8 can hold (0 - 1): the
// comparison has the same result in every cycle (ADR-0106).
const ZERO : u8 = 0

module CompareConstant {
    in  r  : u8
    out eq : bool

    eq = r == ZERO - 1
    //~^ ERROR E2031
}
