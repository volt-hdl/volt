package b;

use a::A;

pub module B {
    in  clk : clock
    in  x   : bool
    out y   : bool
    y = !x
}
