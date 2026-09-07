//~ E2020
// Cyclic constant dependency.
// const-eval.md section 5

const A : u32 = B + 1;
//~^ ERROR cyclic constant dependency
const B : u32 = A + 1;

module CyclicConst {
    in  clk : clock
    out r   : bits<A>

    r = 0
}
