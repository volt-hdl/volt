//~ E4008
// Direct assignment to an open-drain port (ADR-0051): `sda = expr` would
// generate a push-pull driver on a line that other devices also pull
// low. The drive state of a bidirectional pad is a register the compiler
// owns; the only way to change it is sda.drive_low() / sda.release()
// inside an 'on' block. The same rule applies to `inout` ports.
module BadPad {
    in  clk : clock
    in  en  : bool
    opendrain sda : bool
    out q   : bool

    reg q_r : bool = false
    on clk { q_r <= sda.read() }

    sda = if en { false } else { true }
    //~^ ERROR
    q = q_r
}
