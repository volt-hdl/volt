//~ E2020
// Döngüsel sabit bağımlılığı.
// const-eval.md §5

const A : u32 = B + 1;
//~^ ERROR döngüsel sabit bağımlılığı
const B : u32 = A + 1;

module CyclicConst {
    in  clk : clock
    out r   : bits<A>

    r = 0
}
