//~ E2023
// #144: literal arithmetic is evaluated at compile time; division by zero
// is E2023 (const-eval.md §4 `DivisionByZero`, §9, §12 `1 / 0`).

module LiteralDivisionByZero {
    out y : u2

    y = 7 / 0
    //~^ ERROR division by zero
}
