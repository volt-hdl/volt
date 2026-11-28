//~ E2003
// #475 (ADR-0147): an index must be unsigned. SystemVerilog reads an
// index as an unsigned number, so a signed index with a negative value
// selected a high bit or element without a diagnostic: for i : i3 = -1,
// x[i] read x[7]. Convert the index explicitly at the address width of
// the base: x[i as u3] for x : u8, arr[i as u2] for arr : [u8; 4].
// A range bound is a compile-time constant; a signed const as a bound has
// a signed type, and the rule looks at the type, not at the value: K = 1
// is rejected too. Declare the constant unsigned (const K : u3 = 1).

const K : i8 = 1

module SignedRangeBound {
    in  x : u8
    out y : bits<2>

    y = x[K : 0]
    //~^ ERROR an index must be unsigned, found 'i8'
}
