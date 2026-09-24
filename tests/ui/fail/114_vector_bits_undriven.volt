//~ E4012
// ADR-0077 (stage 1 note 1): the same coverage rule for vectors — a
// signal driven slice by slice must have every bit driven (Verilator
// reported 'Bits of signal are not driven' while Volt was silent).
module M {
    in  a : u8
    out y : u8
//~^ ERROR bits 4..=7 of 'y' are never driven

    y[3:0] = a[3:0]
}
