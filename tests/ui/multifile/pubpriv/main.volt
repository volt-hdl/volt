//~ E1004
use lib::Ticker;
use lib::Hidden;
//~^ ERROR 'Hidden' is private to module 'lib'

module Top {
    in  clk    : clock
    in  enable : bool
    out count  : u8
    let c = Ticker { clk: clk, enable: enable }
    count = c.count
}
