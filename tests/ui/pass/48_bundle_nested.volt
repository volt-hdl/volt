// Nested bundles (ADR-0039): a 'struct port' field whose type is another
// 'struct port'. Directions compose: 'in rx : Chan' flips Chan inside
// Link, and 'in link : Link' flips Link again, so
//   link.tx.data  -> input   link.tx.ready -> output
//   link.rx.data  -> output  link.rx.ready -> input
// Flattened names are link_tx_data, link_tx_valid, link_tx_ready,
// link_rx_data, link_rx_valid, link_rx_ready.
struct port Chan {
    out data  : u8
    out valid : bool
    in  ready : bool
}

struct port Link {
    out tx : Chan
    in  rx : Chan
}

module Loopback {
    in  link : Link

    link.rx.data  = link.tx.data
    link.rx.valid = link.tx.valid
    link.tx.ready = link.rx.ready
}
