//~ E2030
// ADR-0074: either every variant has an explicit value or none does.
// '{ Add = 0, Sub, Jal = 8 }' reads two ways: Sub = 1 (by position) or
// Sub = 1 (previous + 1) — and after Jal the readings split for good.
enum Op : u4 {
    Add = 0,
    Sub,
//~^ ERROR mixes explicit and implicit values
    Jal = 8
}

module M {
    in  a : u8
    out y : u8

    y = a
}
