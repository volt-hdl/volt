//~ E2023
// #144: the remainder by zero is E2023 as well (const-eval.md §4: `Rem`
// with b == 0 returns `DivisionByZero`); the divisor is literal
// arithmetic whose value is 0.

module LiteralRemainderByZero {
    out y : u2

    y = 5 % (2 - 2)
    //~^ ERROR remainder by zero
}
