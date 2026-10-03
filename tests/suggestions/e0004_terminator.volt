// suggestion: E0004
// The terminator must repeat the module name; the fix rewrites it.
module Blink {
    in  clk : clock
    out led : bool

    reg on_r : bool = false
    on clk {
        on_r <= !on_r
    }
    led = on_r
} module Blnk
