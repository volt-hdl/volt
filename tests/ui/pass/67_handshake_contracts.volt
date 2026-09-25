// Handshake<T> protocol contracts (ADR-0050).
//
// A plain struct payload is flattened field by field (req_data_addr,
// req_data_prot); the automatic contracts cover every data field:
//   in  req : Handshake<Req>   -> assume:    hold + data stable (env)
//   out rsp : Handshake<u2>    -> invariant: hold + data stable (proven)
// '@no_protocol_check' switches the generation off for one port: 'dbg'
// is a debug tap that mirrors the response without waiting for its own
// ready, so the automatic hold rule would (correctly) fail on it; the
// attribute documents that the port does not speak the protocol.
struct Req {
    addr : u32
    prot : u3
}

module Responder {
    in  clk : clock
    in  req : Handshake<Req>
    out rsp : Handshake<u2>
    @no_protocol_check
    out dbg : Handshake<u8>

    // Every accepted request is answered on the next cycle.
    invariant: prev(req.fired) -> rsp.valid
    // No new request while a response is pending.
    invariant: rsp.valid -> !req.ready
    // The unchecked tap follows the response channel, not its own ready.
    invariant: dbg.valid == rsp.valid
    cover: rsp.fired
    cover: req.stalled
    cover: dbg.fired

    reg rvalid_r : bool = false
    reg rerr_r   : bool = false
    reg last_r   : u8   = 0

    on clk {
        if req.fired {
            rvalid_r <= true
            rerr_r   <= req.data.prot != 0
            last_r   <= req.data.addr[7:0] as u8
        }
        if rsp.fired {
            rvalid_r <= false
        }
    }

    req.ready = !rvalid_r
    rsp.valid = rvalid_r
    rsp.data  = if rerr_r { 2 } else { 0 }
    dbg.valid = rvalid_r
    dbg.data  = last_r
}

// Output net (ADR-0079):
//~ LINT-ALLOW: UNUSEDSIGNAL: the upper address bits and 'dbg_ready' are read only by the handshake contracts
