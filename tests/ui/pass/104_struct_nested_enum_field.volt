// ADR-0077: nested structs and enum fields; whole-struct equality
// compares every field (first field most significant), a sub-struct can
// be assigned as a whole.
enum Kind { Read, Write, Idle }

struct Addr {
    bank : u2
    row  : u6
}

struct Req {
    kind : Kind
    at   : Addr
    last : bool
}

module Tracker {
    in  clk  : clock
    in  req  : Req
    out same : bool
    out bank : u2
    out busy : bool

    reg last_req : Req = Req { kind: Kind::Idle, at: Addr { bank: 0, row: 0 }, last: false }

    on clk {
        last_req.kind <= req.kind
        last_req.at <= req.at
        last_req.last <= req.last
    }

    same = last_req == req
    bank = last_req.at.bank
    busy = last_req.kind != Kind::Idle
}
