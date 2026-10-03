// suggestion: E4008
// read is a method of a bidirectional port; the fix adds '()'. The pad
// is read through sync(), so the fixed design has no W3007.
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
    q = sync(io.read, clk)
}
