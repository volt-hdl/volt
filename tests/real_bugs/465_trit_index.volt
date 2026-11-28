//~ E2003
// #465: a Trit used as an index compiled with no diagnostic and read the
// wrong position. A Trit is -1, 0 or +1 (ADR-0003), coded +1 = 01, 0 = 00,
// -1 = 11 (ADR-0062); spec type-inference.md sec. 3.5 did not say what the
// type of an index may be, and the type checker did not look at it. The
// emitter wrote `x[t]` as `1'((x) >> t)` and an array element as `arr[t]`,
// so the index read the code 11 as the unsigned number 3: for t = -1 the
// design below read x[3] and TABLE[3] (40). `x[t +: 2]` and an assignment
// `wa[t] <= ...` went wrong the same way. Right answer (ADR-0146, the
// maintainer's decision on #465): an index whose type holds a Trit is
// E2003, like `x >> t`; convert the Trit explicitly, for example
// `x[(t as i3 + 1) as u3]` reads x[0], x[1], x[2] for t = -1, 0, +1.
// Fixed by the pull request that closes #465. The design is rejected, so
// it carries no test block; the values of the conversion are checked in
// Verilator by crates/volt-driver/tests/trit_index_sim_tests.rs.

const TABLE : [u8; 4] = [10, 20, 30, 40]

module TritIndex {
    in  t : Trit
    in  x : u8
    out b : bool
    out e : u8

    b = x[t]
    e = TABLE[t]
}
