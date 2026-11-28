//~ E2003
// #475: an index of a signed integer type compiled with no diagnostic and
// read the wrong position. SystemVerilog reads an index as an unsigned
// number; spec type-inference.md sec. 3.5 did not say what the type of an
// index may be, and the type checker did not look at a signed one. The
// emitter wrote `x[i]` as `x[i]`, a const array element as `TABLE_at(i)`
// and an array port element as `arr[8 * (i) +: 8]`, so for i : i3 = -1
// (the bits 111) the design below read x[7] and TABLE[7] (80), and arr[i]
// read element 7 (measured for v0.1.6 with volt test in Verilator 5.054,
// comments of #475). The literal `x[-1]` was already E2006. Right answer
// (ADR-0147, the maintainer's decision (A) on #475, 2026-11-28): an index
// whose type is a signed integer is E2003, like an index that holds a
// Trit (ADR-0146); the rule looks at the type, so an iN index whose value
// is never negative is rejected too. Convert the index explicitly at the
// address width of the base: `x[i as u3]` for x : u8 reads x[7] for
// i = -1, as before, but the source says so. Fixed by the pull request
// that closes #475. The design is rejected, so it carries no test block;
// the conversion of the help is checked in Verilator by
// crates/volt-driver/tests/signed_index_sim_tests.rs.

const TABLE : [u8; 8] = [10, 20, 30, 40, 50, 60, 70, 80]

module SignedIndex {
    in  i : i3
    in  x : u8
    out b : bool
    out e : u8

    b = x[i]
    e = TABLE[i]
}
