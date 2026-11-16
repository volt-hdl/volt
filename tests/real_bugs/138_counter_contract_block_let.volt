//~ W1002
// #138: the inferred counter contract (ADR-0066) took the bound `K` of
// `r == K` for the root `const K` (5), although the block `let K` shadows it
// (name-resolution.md §2, §6: W1002): the compiler added an `invariant
// r <= 5` that the design breaks whenever `lim` > 5, so simulation and
// `volt verify` failed on a correct design. With the bound a signal (as with
// a fresh name) no counter contract is inferred and `r` counts up to `lim`.
// Fixed by #157 (the auto-contract step resolves through the scope chain,
// ADR-0081 appendix "Ayrıştırıcının öteki adımları da kapsam zincirinden
// geçer").
const K : u8 = 5

module M {
    in  clk : clock
    in  lim : u8
    out q   : u8
    reg r : u8 = 0
    on clk {
        let K : u8 = lim
        if r == K {
            r <= 0
        } else {
            r <= r + 1
        }
    }
    q = r
}

test "the bound is the block let" {
    let dut = M { };
    dut.lim = 10;
    step(8);
    // r starts at 0 and counts up while r != lim: after 8 edges r = 8. The
    // bug's invariant r <= 5 is broken at r = 6 and fails the test.
    assert_eq(dut.q, 8);
    step(3);
    // Edges 9, 10: r = 9, 10; edge 11 sees r == lim (10) and wraps to 0.
    assert_eq(dut.q, 0);
}
