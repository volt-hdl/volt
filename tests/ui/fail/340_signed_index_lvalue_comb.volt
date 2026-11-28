//~ E2003
// #475 (ADR-0147): an index must be unsigned. SystemVerilog reads an
// index as an unsigned number, so a signed index with a negative value
// selected a high bit or element without a diagnostic: for i : i3 = -1,
// x[i] read x[7]. Convert the index explicitly at the address width of
// the base: x[i as u3] for x : u8, arr[i as u2] for arr : [u8; 4].

module SignedLvalueComb {
    in  i : i3
    out y : u8

    comb {
        y = 0
        y[i] = true
        //~^ ERROR an index must be unsigned, found 'i3'
    }
}
