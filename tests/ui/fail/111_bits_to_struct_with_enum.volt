//~ E2009
// ADR-0077 Karar 4: raw bits into a struct with an enum field could make
// a code that is no variant (ADR-0074) — build the struct field by field
// and decode the enum with a match.
enum Kind { Read, Write, Idle }

struct Req {
    kind : Kind
    addr : u6
}

module M {
    in  raw  : u8
    out addr : u6

    let r : Req = raw as Req
//~^ ERROR raw bits may hold a code
    addr = r.addr
}
