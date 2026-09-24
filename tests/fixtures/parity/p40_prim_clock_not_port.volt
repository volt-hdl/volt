// parity: E2003
module M {
    in  clk : clock
    in  a   : bits<8>
    in  en  : bool
    out q   : u16
    let r = Ram<u16, 256> { clk: en, addr: a, wr_data: 0u16, wr_en: false }
    q = r.rd_data
}
