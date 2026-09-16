// Bundle arrays (ADR-0056): `[Handshake<T>; N]` and `[struct port; N]`
// ports flatten element by element (`rx_0_data`, `rx_0_valid`,
// `rx_0_ready`, ...). `rx[i].valid` with a constant index -- a literal,
// a const, or a module-level `for` variable, which the unroller turns
// into a literal -- is rewritten to the flat name; direction flipping
// (ADR-0039) and the automatic protocol contracts (ADR-0050) apply to
// every element.

const PORTS : u32 = 3

struct port Req {
    out addr  : u8
    out valid : bool
    in  ack   : bool
}

module Merge {
    in  clk : clock
    in  rx  : [Handshake<u8>; PORTS]
    out tx  : Handshake<u8>
    in  req : [Req; 2]

    wire any_valid : [bool; PORTS]
    reg  seen : bool = false
    on clk { seen <= tx.valid }

    // Fixed priority: port 0 wins, the others wait.
    rx[0].ready = tx.ready
    rx[1].ready = tx.ready && !rx[0].valid
    rx[2].ready = tx.ready && !rx[0].valid && !rx[1].valid

    for i in 0..PORTS {
        any_valid[i] = rx[i].valid
    }

    tx.valid = any_valid[0] || any_valid[1] || any_valid[PORTS - 1]
    tx.data  = if rx[0].valid { rx[0].data }
               else if rx[1].valid { rx[1].data }
               else { rx[2].data }

    // `in req` flips the struct port: addr/valid are inputs, ack outputs.
    req[0].ack = req[0].valid && req[0].addr == 0
    req[1].ack = req[1].valid && req[1].addr != 0

    invariant: rx[0].fired -> tx.valid
    invariant: rx[2].ready -> !rx[0].valid
    invariant: seen == prev(tx.valid)
}
