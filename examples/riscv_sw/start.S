# Minimal startup for examples/riscv_core.volt (RV32IM + Zicsr, M-mode).
#
# Reset enters at 0x0000_0000 (pc resets to zero), so _start must be the
# first thing in ROM: link.ld places .text.init ahead of everything else.

    .section .text.init
    .globl _start
_start:
    la   t0, trap_handler
    csrw mtvec, t0
    la   sp, _stack_top

    # Copy .data from its ROM load address to RAM, then clear .bss.
    # Both loops are empty for hello.c; they keep start.S correct for
    # programs that do have initialised or zeroed globals.
    la   t0, _data_load
    la   t1, _data_start
    la   t2, _data_end
1:  bgeu t1, t2, 2f
    lw   t3, 0(t0)
    sw   t3, 0(t1)
    addi t0, t0, 4
    addi t1, t1, 4
    j    1b
2:  la   t1, _bss_start
    la   t2, _bss_end
3:  bgeu t1, t2, 4f
    sw   zero, 0(t1)
    addi t1, t1, 4
    j    3b
4:  call main

# `j halt` assembles to 0x0000006F (jal x0, 0); the simulation SoC
# (hello_soc.volt) watches for exactly this word to raise `halted`.
halt:
    j    halt

# Any trap parks the core here. Distinct from `halt` (the word is a
# jal with a zero offset too, so the SoC tells them apart by its sticky
# `trap_seen` flag, not by the instruction).
    .balign 4
trap_handler:
    j    trap_handler
