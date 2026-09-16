//~ E2008
// A bundle array is flattened per element at compile time (ADR-0056), so
// the index must be a compile-time constant: a literal, a const, or a
// module-level `for` variable. A signal cannot select a bundle -- there
// is no `rx_sel_data` wire to read. Mux the flat fields instead:
// `tx.data = if sel == 0 { rx[0].data } else { rx[1].data }`.
module Pick {
    in  clk : clock
    in  sel : u1
    in  rx  : [Handshake<u8>; 2]
    out tx  : Handshake<u8>

    rx[0].ready = tx.ready
    rx[1].ready = tx.ready
    tx.valid = rx[0].valid || rx[1].valid
    tx.data  = rx[sel].data
//~^ ERROR E2008: index into bundle array 'rx' must be a compile-time constant
}
