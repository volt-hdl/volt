// Generic module instantiation with const arguments (ADR-0041).
// `Delay<4, 8>` and `Delay<2, 8>` are monomorphised into `Delay_4_8`
// and `Delay_2_8`; the second `Delay<4, 8>` reuses the first module.

module Delay<const N: u32, const W: u32> {
    in  clk : clock
    in  x   : uint<W>
    out y   : uint<W>

    reg line : [uint<W>; N] = [0; N]

    on clk {
        for i in 1..N { line[i] <= line[i - 1] }
        line[0] <= x
    }

    y = line[N - 1]
}

module Top {
    in  clk : clock
    in  x   : u8
    out a   : u8
    out b   : u8
    out c   : u8

    let d4  = Delay<4, 8> { clk, x }
    let d2  = Delay<2, 8> { clk, x }
    let d4b = Delay<4, 8> { clk, x: d2.y }

    a = d4.y
    b = d2.y
    c = d4b.y
}
