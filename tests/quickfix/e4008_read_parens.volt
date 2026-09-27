// quickfix: E4008
// read is a method of a bidirectional port; the fix adds '()'.
module Pad {
    in  clk  : clock
    in  en   : bool
    in  d    : bool
    inout io : bool
    out q    : bool

    on clk {
        if en {
            io.drive(d)
        } else {
            io.release()
        }
    }
    q = io.read
}
