//~ W1002
// #137: the pipeline step rewrote every path named like a stage value into
// its boundary register by the text of the name, also inside a `for a` loop
// that redeclares `a`: both writes became `r[A_a_r] <= x`. The loop variable
// is declared in an inner block scope and shadows the stage value
// (name-resolution.md §2, §6: W1002, warned by the resolver because the
// stage value becomes a module-level let, ADR-0081 appendix "Ayrıştırıcının
// öteki adımları da kapsam zincirinden geçer"); `r[a]` is indexed with the
// loop variable, so one edge writes r[0] and r[1]. Outside the loop `a` is
// still the stage value (ADR-0038 §2). Fixed by #157.
pipeline(2) P {
    in  clk : clock
    in  x   : u32
    out y0  : u32
    out y1  : u32
    out z   : u32
    reg r : [u32; 2] = [0; 2]
    reg q : u32 = 0
    stage A {
        let a : u32 = x + 1
    }
    stage B {
        q <= a
        for a in 0..2 {
            r[a] <= x
        }
    }
    y0 = r[0]
    y1 = r[1]
    z = q
}

test "the loop variable shadows the stage value" {
    let dut = P { };
    dut.x = 10;
    step(1);
    // The loop writes r[0] and r[1] with x on the edge: 10 and 10. The bug
    // wrote both to r[A_a_r], whose value before the edge is its init 0, so
    // r[1] stayed 0.
    assert_eq(dut.y0, 10);
    assert_eq(dut.y1, 10);
    // In stage B, `a` reads the boundary register of stage A (ADR-0038 §2);
    // before the first edge it holds its init value 0 (numeric -> 0).
    assert_eq(dut.z, 0);
    step(1);
    // After the first edge the boundary register holds x + 1 = 11; the second
    // edge copies it into q.
    assert_eq(dut.z, 11);
}
