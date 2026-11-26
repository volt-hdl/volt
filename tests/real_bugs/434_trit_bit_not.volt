//~ E2003
// #434: '~' on a Trit worked on its two-bit code instead of its value. A
// Trit is -1, 0 or +1 (ADR-0003), coded +1 = 01, 0 = 00, -1 = 11
// (ADR-0062). '~w' kept the type Trit (type-inference.md sec. 3.4 said
// `UnOp::BitNot => ot` for every type) and compiled with no diagnostic:
// for w = +1 it gave the code 10, read as -2, outside the Trit values, so
// the MAC below added -2 * x for a weight of +1. Right answer (ADR-0142,
// the maintainer's decision on #434): '~' is not defined for a Trit,
// E2003; negate a Trit with '-w'. Fixed by the pull request that closes
// #434. The design is rejected, so it carries no test block; the values
// of '-w' are checked in Verilator by
// crates/volt-driver/tests/trit_negate_sim_tests.rs.

module TernaryMac {
    in  w : Trit
    in  x : i8
    out y : i8

    y = (~w) * x
}
