//~ E4005
// Assigning a bundle field whose effective direction is input:
// 'in req : Req' flips 'out addr' into an input of Slave (ADR-0039).
struct port Req {
    out addr  : u32
    in  ready : bool
}

module Slave {
    in  req  : Req
    out seen : u32

    req.addr = 0
    //~^ ERROR E4005
    req.ready = true
    seen = req.addr
}
