// parity: E4001
// drivers: 16 15
pipeline(2) P {
    in  clk : clock
    in  x : u32
    out y : u32

    stage A {
        let a : u32 = x + 1
    }
    stage B {
        let b : u32 = a + 2
    }

    y = stage(B).b
    y = x
}
