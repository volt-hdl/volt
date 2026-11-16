//~ PASS
// #207: the emitter converted an integer literal to i128 and saturated a
// value that did not fit: every literal from 2^127 to 2^128 - 1 was
// written as 128'h7FFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF (reset values,
// masks, comparisons). ast-nodes.md (`IntLit { value: u128 }`: every value
// up to 2^128 - 1 is a literal) and type-inference.md §3.6 (a literal goes
// to any numeric type with `as`; it fits bits<128>, so it keeps its
// value). Fixed in PR #265.
module Key {
    out hi : bits<64>
    out lo : bits<64>
    let k : bits<128> = 0x8000_0000_0000_0000_0000_0000_0000_0001 as bits<128>
    hi = k[127:64]
    lo = k[63:0]
}

test "a literal above 2^127 keeps its value" {
    let dut = Key { };
    step(1);
    // k = 2^127 + 1: bits 127 and 0 set. Upper 64 bits 0x8000_0000_0000_0000,
    // lower 64 bits 1. Before the fix: 0x7FFF_FFFF_FFFF_FFFF and
    // 0xFFFF_FFFF_FFFF_FFFF (2^127 - 1).
    assert_eq(dut.hi, 0x8000_0000_0000_0000);
    assert_eq(dut.lo, 1);
}
