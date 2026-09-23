//~ E4009
// A `struct port` that contains itself has no finite flat form: a bundle
// is flattened to plain ports at compile time (ADR-0039), and 'Req.req'
// would need 'req_addr', 'req_req_addr', 'req_req_req_addr', ... forever.
// Before ADR-0067 the compiler silently stopped at depth 8 and, with k
// self-referencing fields, produced k^9 ports (found by the fuzzer: >10 GB).
struct port Req {
    out addr : u32
    in  ready : bool
    in  req  : Req
    //~^ ERROR E4009
}

module Slave {
    in  req  : Req
    out seen : u32

    seen = req.addr
}
