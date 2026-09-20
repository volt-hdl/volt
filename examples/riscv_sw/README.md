# riscv_sw — a C program on the Volt RV32IM core

`hello.c` is compiled by a stock GCC, linked into a memory image and run
on `examples/riscv_core.volt` in simulation. The test reads the image
with `read_hex()` and writes it into the SoC's program memory with
`load()` (ADR-0058). The test
`hello program prints greeting` (in `examples/riscv_core_test.volt`)
checks that the UART prints

```
Hello from Volt!
42
```

where `42` is `7 * 6` (MUL) and its two digits come from a real DIV and a
real REM — the operands are `volatile`, so `-O2` can neither fold the
arithmetic nor replace the division by a multiply-by-reciprocal.

| File | Role |
|------|------|
| `hello.c` | the program: `putchar` polls UART status `0x2000_0004` bit 0, writes `0x2000_0000` |
| `start.S` | reset entry at address 0: `mtvec`, stack pointer, `.data` copy, `.bss` clear, `call main`, `halt: j halt` |
| `link.ld` | ROM `0x0000_0000` 64 KB, RAM `0x1000_0000` 64 KB, stack at the end of RAM |
| `Makefile` | `hello.elf` → `hello.hex` |
| `hello.hex` | **generated, committed** — `$readmemh` text, one 32-bit word per element |
| `hello_soc.volt` | `HelloSoc`: core + 1024-word program memory `imem` + 64-word RAM + UART receiver + sanity monitor |

## Running it (no RISC-V toolchain needed)

The image is in the repository, so this is an ordinary
`volt test` run (see `examples/README.md` for the Verilator Docker
recipe):

```
cd examples
volt test riscv_core_test.volt        # 59 tests, the last one is the C program
volt build riscv_sw/hello_soc.volt    # HelloSoc.sv, RiscvCore.sv, UartTx.sv
```

## Rebuilding the program

Any bare-metal RISC-V GCC works. Debian's `gcc-riscv64-unknown-elf`
targets RV32 through `-march=rv32im_zicsr -mabi=ilp32`; since nothing
links against libc or libgcc (the M extension covers `*`, `/`, `%`), the
missing rv32 multilib does not matter. One-time image, then `make`:

```
printf 'FROM debian:bookworm-slim\nRUN apt-get update && apt-get install -y --no-install-recommends gcc-riscv64-unknown-elf binutils-riscv64-unknown-elf make\n' \
    | docker build -t volt-riscv-gcc -
docker run --rm -v "$PWD:/work" -w /work volt-riscv-gcc make
```

(Git Bash on Windows: prefix the `docker run` with `MSYS_NO_PATHCONV=1`.)
With a `riscv32-unknown-elf-` toolchain on the PATH:
`make CROSS=riscv32-unknown-elf-`. `make dump` prints the disassembly.

After changing the program, update the expected bytes — and the
`cycles` / `instrs` numbers — in the test.

## How the program gets into the memory

```
test "hello program prints greeting" {
    let dut = HelloSoc { };
    let program = read_hex("riscv_sw/hello.hex");
    load(dut.imem, program);
    reset();
    step(5000);
    ...
}
```

- **Program**: `read_hex` parses `hello.hex` at compile time (the path
  is relative to the test file and may not leave the project), `load`
  writes the words into `reg imem : [u32; 1024]` of the simulated
  design. The core has been fetching zeros — illegal instructions —
  since the implicit start-up reset, so `reset()` restarts it; loaded
  memories survive a reset. `riscv_core.volt` and `hello_soc.volt`
  never change when the program does. In hardware the same memory is
  written through the `prog_we` / `prog_addr` / `prog_data` port, which
  the test leaves idle (it also keeps `imem` from being a never-written
  register, W3001).
- **The hex file holds words, not bytes**: the Makefile passes
  `--verilog-data-width=4` to objcopy so that one `$readmemh` element is
  one `u32`. objcopy still prints *byte* addresses in the `@` lines while
  `$readmemh` (and `read_hex`) count elements; that is harmless for an
  image that starts at 0, but a second section would land at four times
  its element index.
- **Size**: `imem` is a register array, so the 4096-word limit of `const`
  tables does not apply; the 1024 words (4 KB) here are a choice. A
  longer image is rejected at compile time (E8510).
- **UART capture**: a test still cannot watch a serial line. `HelloSoc`
  has an 8N1 receiver on `uart_txd` (4 clocks per bit, like
  `uart_tx.volt`) that stores each byte in `rx_buf`. The test keeps the
  expected text in an array and pages through the buffer in a `for`
  loop: select a byte with `rx_sel`, step once so the output settles
  (ports are not re-evaluated between a port write and an assert), and
  compare `rx_byte`.

Before ADR-0058 a test could only set ports, `step` and assert, so the
program had to be a generated Volt file (`hex2volt.py` → `hello_rom.volt`,
a `const` table behind a ROM module). Both are gone.

`HelloSoc` also exposes `halted` (the core fetched `j halt`),
`trap_seen`, `unexpected` (a bus access outside ROM/RAM, a store to ROM,
a pc outside the program, an interrupt or an MRET) and the `cycles` /
`instrs` counters, which stop at the halt.

## Numbers

90 program words (360 bytes). 847 instructions in 913 cycles: the 20
UART frames (40 cycles each, `putchar` busy-waits in between) dominate,
and the DIV and the REM hold the core for 33 extra cycles each. No trap,
no stray bus access. A `+ 1` mutant on the core's multiplier output
prints `43` and fails the test.

The RAM model is 64 words that alias through the 64 KB window; `hello.c`
only uses a few stack words at the top. A program with real data needs a
larger `ram` array in `hello_soc.volt`.
