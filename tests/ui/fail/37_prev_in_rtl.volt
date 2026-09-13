//~ E5017
// prev() is a contract-only builtin (ADR-0040). Using it in RTL — here
// in a continuous assignment — is E5017: hardware has no implicit
// history, a past value in RTL must be an explicit register.
module Delay {
    in  clk : clock
    in  x   : u8
    out y   : u8

    y = prev(x)
    //~^ ERROR E5017
}
