// Compile-time constants and for-loop unrolling
// const-eval.md sec. 7, sec. 8

const WIDTH : u32 = 8;
const DEPTH : u32 = 4;

module ConstAndGenerate {
    in  clk  : clock
    in  data : bits<WIDTH>
    in  mask : bits<WIDTH>
    out result : bits<WIDTH>

    wire temp : bits<WIDTH>

    // for is unrolled at compile time -- no loop in hardware
    for i in 0..WIDTH {
        temp[i] = data[i] & mask[i]
    }

    result = temp
}
