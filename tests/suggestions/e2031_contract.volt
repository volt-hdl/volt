// suggestion: E2031
// In a contract: prev() cannot go into a let, so the fixes use 'as'.
module CmpContract {
    in  clk   : clock
    out level : u3

    reg level_r : u3 = 0
    on clk {
        level_r <= level_r + 1
    }
    level = level_r

    ensures: prev(level) != 7 -> level == prev(level) + 1
}
