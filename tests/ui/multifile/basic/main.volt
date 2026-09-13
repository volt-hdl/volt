//~ PASS
// `use` loads ./lib.volt (ADR-0042); Ticker is pub there.
use lib::Ticker;

module Top {
    in  clk    : clock
    in  enable : bool
    out count  : u8
    let c = Ticker { clk: clk, enable: enable }
    count = c.count
}
