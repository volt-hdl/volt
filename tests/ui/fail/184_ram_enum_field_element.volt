//~ E2009
// ADR-0087: a never-written Ram address reads back the memory's initial
// contents — raw bits that need not be a valid 'Mode' code. Storing an
// enum-bearing T in the RAM family would be an implicit bits → enum cast
// (forbidden, ADR-0077); store 'u8' and decode the field explicitly.

enum Mode { Off, Slow, Fast }

struct Cmd {
    mode : Mode
    arg  : u6
}

module RamOfCmd {
    in  clk  : clock
    in  addr : bits<4>
    in  cmd  : Cmd
    in  we   : bool
    out q    : Cmd

    let mem = Ram<Cmd, 16> { clk: clk, addr: addr, wr_data: cmd, wr_en: we }
    //~^ ERROR Ram<Cmd, ...> is invalid: field 'mode' of 'Cmd' is an enum or Trit, and a never-written address reads back raw bits that may be no valid value
    q = mem.rd_data
}
