//~ W3007
// Unsynchronised read of a bidirectional pad (ADR-0051): the other end
// of `sda` is an external device with its own timing, so sampling the
// line directly into a register can go metastable. An 'in' port is
// trusted to be in the module's domain (K2); an inout/opendrain read is
// treated as external and must go through sync(). Reads inside contracts
// and as the source of sync() are exempt.
module Sampler {
    in  clk : clock
    in  en  : bool
    opendrain sda : bool
    out bit : bool

    invariant: !en -> sda.released

    reg bit_r : bool = false
    on clk {
        bit_r <= sda.read()
        //~^ ERROR
        if en { sda.drive_low() } else { sda.release() }
    }
    bit = bit_r
}
