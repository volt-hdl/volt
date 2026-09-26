//~ PASS
// `use` loads ./fnlib.volt (ADR-0042); sat_inc is a pub fn there.
use fnlib::sat_inc;

module Top {
    in  a : u8
    out y : u8
    y = sat_inc(a) + 1
}
