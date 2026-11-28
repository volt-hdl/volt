//~ E2003
// #475 (ADR-0147): an index must be unsigned. SystemVerilog reads an
// index as an unsigned number, so a signed index with a negative value
// selected a high bit or element without a diagnostic: for i : i3 = -1,
// x[i] read x[7]. Convert the index explicitly at the address width of
// the base: x[i as u3] for x : u8, arr[i as u2] for arr : [u8; 4].
// The rule looks at the type: a + b of two i2 is a signed integer, i3
// (type-inference.md sec. 3.3).

module SignedIndexExpression {
    in  a : i2
    in  b : i2
    in  x : u8
    out y : bool

    y = x[a + b]
    //~^ ERROR an index must be unsigned, found 'i3'
}
