// ADR-0078: 'interrupt' and 'char' are valid SystemVerilog, but Verilator
// treats them as C++ words: as top-level ports it renames them to
// '__SYM__interrupt' / '__SYM__char' and stops with SYMRSVDWORD. Volt
// wraps such port declarations in 'verilator lint_off SYMRSVDWORD' and its
// test bench writes the renamed members.
module IrqLatch {
    in  clk       : clock
    in  char      : u8
    out interrupt : bool

    reg seen : bool = false
    on clk { if char != 0 { seen <= true } }
    interrupt = seen
}
