//~ E1013
// ADR-0078: a port keeps its name in SystemVerilog; 'small' and 'large'
// are SystemVerilog keywords (this was ui/pass/10 before ADR-0078 — its
// generated SV did not compile).
module ExplicitCast {
    in  small : u8
//~^ ERROR 'small' is a SystemVerilog keyword
    in  large : u16
    out sum   : u17

    sum = (small as u16) + large
}
