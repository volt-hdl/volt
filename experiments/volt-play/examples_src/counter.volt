// An 8-bit counter with an enable input and a wrap-around value.
//
//   volt check     check the design (no output files)
//   volt build     write SystemVerilog to build/rtl/
//   volt test      run counter_test.volt in Verilator
//   volt verify    prove the contracts with SymbiYosys

// The counter goes back to 0 after this value.
const MAX : u8 = 9

// The next count. A `fn` is pure combinational logic, expanded where
// it is called.
fn next(count: u8) -> u8 {
    if count == MAX { 0 } else { count + 1 }
}

pub module Counter {
    in  clk    : clock
    in  enable : bool
    out count  : u8
    out wrap   : bool

    // Contracts. `volt verify` proves them for every input sequence;
    // `volt test` also checks them on every cycle of every test.
    invariant: count_r <= MAX
    cover: count_r == MAX

    reg count_r : u8 = 0

    on clk {
        if enable {
            count_r <= next(count_r)
        }
    }

    count = count_r
    wrap  = enable && count_r == MAX
}
