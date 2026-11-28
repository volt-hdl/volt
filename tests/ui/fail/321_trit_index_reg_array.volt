//~ E2003
// #465 (ADR-0146): a Trit is not an index. A Trit is -1, 0 or +1
// (ADR-0003) and -1 is no bit or element position; the index read the
// two bits that encode the Trit (ADR-0062: -1 = 11) as an unsigned number,
// so for t = -1 it selected position 3. Convert the Trit explicitly:
// x[(t as i3 + 1) as u3] reads x[0], x[1], x[2] for t = -1, 0, +1.

module TritRegArray {
    in  clk : clock
    in  t   : Trit
    in  d   : u8
    out y   : u8

    reg ra : [u8; 4] = [0, 0, 0, 0]
    on clk {
        ra[0] <= d
    }
    y = ra[t]
    //~^ ERROR an index must not be a Trit, found 'Trit'
}
