// Output net fixture (ADR-0042 addendum): the only module of the unit;
// the fns come from the module-less library file checksum.volt.

use checksum::{fold, add_sat};

module SumTop {
    in  clk  : clock
    in  word : u16
    out sum  : u8
    reg acc : u8 = 0
    on clk { acc <= add_sat(acc, fold(word)) }
    sum = acc
}
