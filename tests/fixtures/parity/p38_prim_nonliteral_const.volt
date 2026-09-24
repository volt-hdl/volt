// parity: E2008
module M {
    in  clk : clock
    in  n   : u8
    in  a   : bits<8>
    out q   : u16
    let r = Ram<u16, n> { clk: clk, addr: a, wr_data: 0u16, wr_en: false }
    q = r.rd_data
}
