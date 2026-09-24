// parity: E5016
// drivers: ok
pipeline(2) P {
    in  clk : clock
    in  x : u32
    out y : u32

    stage A {
        let a : u32 = x + 1
    }
    stage B {
        let a : u32 = x + 2
    }

    y = stage(B).a
}
