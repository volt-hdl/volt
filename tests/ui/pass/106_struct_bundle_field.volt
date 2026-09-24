// ADR-0077 Karar 1: a 'struct port' bundle field may carry a plain
// struct; the bundle flattens to `link_data` and the struct to its fields
// (`link_data_addr`, `link_data_len`), the direction applies to all.
struct Header {
    addr : u8
    len  : u4
}

struct port Link {
    out data  : Header
    out valid : bool
    in  ready : bool
}

module Source {
    in  clk  : clock
    in  go   : bool
    out link : Link

    reg addr : u8 = 0
    on clk { if go && link.ready { addr <= addr + 1 } }

    link.data = Header { addr: addr, len: 4 }
    link.valid = go
}

module Sink {
    in  link : Link
    out got  : u8

    link.ready = true
    got = if link.valid { link.data.addr } else { 0 }
}
