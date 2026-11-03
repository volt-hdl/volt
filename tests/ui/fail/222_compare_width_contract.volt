//~ E2031
// Contracts follow the same rule: `prev(level) + 1` is 4 bits wide.
module CompareContract {
    in  clk   : clock
    out level : u3

    reg level_r : u3 = 0
    on clk {
        level_r <= level_r + 1
    }
    level = level_r

    ensures: prev(level) != 7 -> level == prev(level) + 1
    //~^ ERROR E2031
}
