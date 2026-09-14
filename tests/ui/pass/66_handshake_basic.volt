// Builtin Handshake<T> bundle (ADR-0050): single-domain valid/ready.
// 'out tx' is the producer (data/valid outputs, ready input); 'in rx'
// flips every field. The parser flattens to tx_data / tx_valid /
// tx_ready and rewrites the virtual fields: tx.fired = valid && ready,
// tx.stalled = valid && !ready. Protocol contracts (valid held until
// ready, data stable while stalled) are generated automatically.
module Producer {
    in  clk : clock
    out tx  : Handshake<u8>

    cover: tx.fired

    reg count   : u8   = 0
    reg valid_r : bool = false
    on clk {
        if !valid_r {
            valid_r <= true
        } else if tx.ready {
            valid_r <= false
            count   <= count + 1
        }
    }
    tx.data  = count
    tx.valid = valid_r
}

module Consumer {
    in  clk : clock
    in  rx  : Handshake<u8>
    out sum : u8

    cover: rx.stalled

    reg acc  : u8   = 0
    reg busy : bool = false
    on clk {
        if rx.fired {
            acc  <= acc + rx.data
            busy <= true
        } else {
            busy <= false
        }
    }
    rx.ready = !busy
    sum = acc
}

module Link {
    in  clk : clock
    out sum : u8

    // Bodies resolve top-down: the consumer's ready is forward-declared.
    wire ready : bool
    let p = Producer { clk: clk, tx_ready: ready }
    let c = Consumer { clk: clk, rx_data: p.tx_data, rx_valid: p.tx_valid }
    ready = c.rx_ready
    sum = c.sum
}
