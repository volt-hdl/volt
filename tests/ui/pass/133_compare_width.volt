// Comparisons whose sides have different natural widths (ADR-0106):
// accepted when one side is a constant that fits, or when the width is
// written with `as`. Bit and range selects of a computed value lower to
// size casts, not to a select of a parenthesized expression.
const LIMIT : u8 = 6

module CompareWidth {
    in  clk   : clock
    in  a     : u3
    in  b     : u3
    in  c     : u8
    in  d     : u8
    out bound : bool
    out lit   : bool
    out big   : bool
    out same  : bool
    out wrap  : bool
    out keep  : bool
    out carry : bool
    out low   : u4

    reg count_r : u8 = 0
    on clk {
        if count_r == LIMIT - 1 {
            count_r <= 0
        } else {
            count_r <= count_r + 1
        }
    }

    bound = count_r == LIMIT - 1
    lit   = c + d == 5
    big   = c + d < 300
    same  = c + d == c + count_r
    wrap  = a == (b + 1)[2:0] as u3
    keep  = (a as u4) == b + 1
    carry = (c + d)[8]
    low   = (c + 1)[3:0] as u4

    invariant: count_r <= LIMIT - 1
}
