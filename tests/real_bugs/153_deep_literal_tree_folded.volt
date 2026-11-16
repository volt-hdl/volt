//~ PASS
// #153: the emitter folded a literal-arithmetic tree with its exact value
// (#144) only down to a depth of 64; a deeper tree fell back to writing
// every literal at the context width. Here the sum of seventy 1s is a tree
// 70 levels deep: the 65-term subtree was written as 3'h1 (65 mod 8) and
// the divisor 10 as 3'h2, so y was (1 + 5) / 2 = 3. type-inference.md
// §3.3, §4 and const-eval.md §1, §4: the whole tree has its exact value,
// 70 / 10 = 7, which fits u3. Found and fixed on the branch of PR #151
// before it was merged.
module Deep {
    out y : u3
    y = (1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1) / 10
}

test "seventy ones divided by ten" {
    let dut = Deep { };
    step(1);
    // 70 * 1 = 70; 70 / 10 = 7 (toward zero); 7 fits u3 (0..=7).
    // Before the fix: 3.
    assert_eq(dut.y, 7);
}
