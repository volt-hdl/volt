// parity: E2005
module M {
    in  clk : clock
    in  a   : bits<8>
    out q   : u16
    let r = Ram<u16, 256> { addr: a, wr_data: 0u16, wr_en: false }
    q = r.rd_data
}
