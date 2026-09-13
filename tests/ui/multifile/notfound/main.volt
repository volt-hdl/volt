//~ E1011
use nowhere::Thing;
//~^ ERROR module not found: 'nowhere'

module Top {
    in  clk : clock
    in  a   : bool
    out b   : bool
    b = a
}
