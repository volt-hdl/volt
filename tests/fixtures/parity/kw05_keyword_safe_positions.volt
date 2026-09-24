// parity: ok
struct Cfg {
    packed : u4
}
enum release { force, wait }
module Safe {
    in  clk       : clock
    in  Packed    : u8
    in  p         : Cfg
    in  go        : bool
    out interrupt : u8
    out z         : bool
    reg s : release = release::force
    on clk { if go { s <= release::wait } }
    interrupt = Packed + (p.packed as u8)
    z = s == release::wait
}
