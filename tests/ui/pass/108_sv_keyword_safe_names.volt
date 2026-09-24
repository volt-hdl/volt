// ADR-0078: SystemVerilog keywords are refused only where they would be an
// SV name. Here every keyword stays out of the generated SV: SV is
// case-sensitive ('Packed'), a struct field or bundle field is only the
// suffix of '<signal>_<field>' ('p_packed', 'b_table'), an enum type and
// variant only form '<Enum>_<Variant>' ('release_force'), an unrolled
// 'let' carries its index ('edge_0') and a built-in primitive's name is
// only a prefix ('buf_rd_data').
struct Cfg {
    packed : u4
    table  : u4
}

struct port Link {
    out table : u8
}

enum release { force, wait }

module Safe {
    in  clk    : clock
    in  Packed : u8
    in  p      : Cfg
    in  go     : bool
    out b      : Link
    out y      : u8
    out z      : bool

    reg s : release = release::force
    on clk { if go { s <= release::wait } }

    for i in 0..1 {
        let edge : u8 = Packed + 1
    }

    let buf = SyncFifo<u8, 4> {
        clk:     clk,
        wr_data: Packed,
        wr_en:   go,
        rd_en:   go,
    }

    b.table = (p.packed as u8) + (p.table as u8)
    y = edge_0 ^ buf.rd_data
    z = s == release::wait
}
