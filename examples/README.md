# Volt Examples

Standalone Volt designs that exercise the language end to end:
compile to SystemVerilog, lint with Verilator, and formally verify
the contracts with `volt verify`.

## Examples

| File | Demonstrates |
|---|---|
| `riscv_pipeline.volt` | RV32I 5-stage pipeline (12-instruction subset), hand-built: manual inter-stage registers, EX→EX / MEM→EX forwarding plus the WB→ID bypass, load-use stall, branch/JAL flush. 10 inductive invariants (x0, PC alignment, per-stage wb_en/rd validity) proven with `--mode prove --depth 3 --engine boolector`; `stall_o`/`flush_o` debug ports work around contracts only seeing ports and registers. |
| `riscv_core.volt` | RV32IM + Zicsr single-cycle core with traps, one external interrupt and a memory-mapped UART: the 37 base instructions, the 8 M-extension instructions, the 6 CSR instructions, ECALL/EBREAK/MRET (521 lines; RV32IM + Zicsr without traps was 380, RV32I only 183). Register file as one `reg regs : [u32; 32]` with dynamic indexing (ADR-0035), signed ops through `as i32` (ADR-0036). **Multiply is single-cycle**: one unsigned `u32 * u32` into a `: u64` target (ADR-0041 widening), the MULH/MULHSU high word fixed up with `hi - a31*b - b31*a` so all four share one multiplier (4 DSP48E1 under `synth_xilinx`). **Divide is multi-cycle**: a 1-bit restoring divider on magnitudes, 34 cycles, `stall_o` holds the pc — a combinational divider would put a 32-deep subtract chain on every instruction's path. The RISC-V corner cases (x/0 = -1, x%0 = x, MIN/-1 = MIN rem 0) fall out of the datapath and are all simulated; the zero-divisor result is also *proven* through a per-step inductive invariant (`div_q == (1 << div_cnt) - 1`, `div_r == div_n >> (32 - div_cnt)`). CSRs (`mstatus`, `mie`, `mip`, `mtvec`, `mscratch`, `mepc`, `mcause`, 64-bit `mcycle`/`minstret`) are hand-written, not `@mmio`: @mmio generates an AXI4-Lite handshake slave with a registered read, a CSR access is an atomic same-cycle read-modify-write on a 12-bit number. `mcycle`/`minstret` are read-only here so `minstret <= mcycle` can hold; a sticky `cyc_wrap` flop (exposed as `cyc_wrap_o`, because contracts see only ports and registers and an unread flop fails lint) keeps it inductive across the 2^64 wrap. **Traps**: illegal instruction (2), instruction-address-misaligned on a taken jump whose target has bit 1 set (0), EBREAK (3), misaligned load/store (4/6), ECALL (11); a trap is taken *instead of* the instruction (no register/CSR/memory/I-O write, `minstret` does not count): `mepc <- pc`, `MPIE <- MIE`, `MIE <- 0`, `pc <- mtvec`; MRET undoes it. Trapping misaligned jumps is what makes "mepc is a valid pc" provable (`!pc_r[1]`, `!mtvec[1:0]`, `!mepc[1:0]` are mutually inductive). **Interrupt**: one level-sensitive `irq` line (`mip.MEIP`), taken when `mstatus.MIE && mie.MEIE` with `mcause = 0x8000000B`. A single-cycle core starts every cycle on an instruction boundary except inside a division, so `take_irq` is gated with `!div_busy && !div_done`: the interrupt waits, the division retires, and `mepc` is the pc of the *next* instruction (simulated: irq raised one cycle into a DIV, acknowledged 33 cycles later, quotient intact). An interrupt that arrives before the divider has latched displaces the DIV instead. `trap_o` / `irq_ack_o` / `mret_o` are real ports because contracts see only ports and registers: `prev(trap_o) -> mepc == prev(pc_r)`, `prev(mret_o) -> pc_r == prev(mepc)`, `irq_ack_o -> mstatus[3] && mie_meie && irq`, `irq_ack_o -> !div_busy && !div_done`; three mutants (gate removed, `mepc <- pc + 4`, misaligned-jump trap removed) each fail induction. **Memory-mapped I/O**: addresses `0x2xxx_xxxx` never reach the memory bus (`mem_read`/`mem_write` stay low); a store to `0x2000_0000` pulses `start` on the reused `UartTx` (`use uart_tx::UartTx`), a load from `0x2000_0004` returns its `busy` bit through the same sub-word load path; ROM `0x0000_xxxx` / RAM `0x1000_xxxx` are split by the external decoder. Finding (fixed since by ADR-0057; the contracts still use the workaround): the SV emitter decided parentheses with Volt's precedence table, where `&` binds tighter than `==`; SV is the other way round, so `(mepc & 3) == 0` is emitted as `mepc & 32'd3 == 32'd0` and silently means something else — the contracts avoid bitwise-inside-comparison (`!mepc[0] && !mepc[1]`). 58/58 simulation tests, Verilator `-Wall` clean (RiscvCore + UartTx), 29 properties (20 invariants + 9 covers) plus the 7 of the instantiated `UartTx`: `prove 3 --engine boolector` (2 s) / `bmc 12` (22 s) / `cover 40` (94 s): all 9 core covers are reached (interrupt at step 4, UART start bit at step 5, division by zero at step 35), but `volt verify` still reports E5001, because the submodule's own `cover: state_r == 3` (STOP) is checked inside the core and an 8N1 frame at 4 clocks per bit cannot finish before step 40 when the earliest possible UART write (reset, `lui`, `sb`) is at step 2; past step 40 the solver no longer terminates in useful time with the 32x32 multiplier in the cone. `volt verify examples/uart_tx.volt --mode cover --depth 48` reaches it standalone. Word of warning: `stall` is a reserved word (E0001 on `let stall`). Under `synth_xilinx`, RV32I → RV32IM + Zicsr → + traps/irq/UART: DSP48E1 0 → 4 → 4, FDRE/FDSE 1056 → 1415 → 1478 (28 of them the UART), LUT3..6 (the LC estimate) 1657 → 2364 → 2560. |
| `uart_tx.volt` | UART transmitter (8N1). Four-state FSM written as a `match` inside the sequential block (ADR-0032), a `u10` baud counter compared directly against the `CLKS_PER_BIT` constant (ADR-0031), start/busy handshake, LSB-first shift register, safety invariants proven by induction (`--mode prove`), and `cover` targets showing all four states are reachable. |
| `axi4lite_slave.volt` | AXI4-Lite register slave built from five builtin `Handshake<Payload>` bundles (ADR-0050 on top of ADR-0039): each channel is one `Handshake<T>` whose payload struct carries the data fields; `in aw`/`in w`/`in ar` flip the request channels into inputs, `out b`/`out r` produce the responses; `aw.fired` = valid && ready. The valid/ready protocol rules (valid held until ready, payload stable while stalled) are generated by the compiler — assumptions on the request channels, proven invariants on the responses; only the bridge-specific rules (`prev(aw.fired) -> b.valid`, ADR-0040) are hand-written. The generated SV is flat (`aw_data_addr`, `aw_valid`, `aw_ready`, ...). Four RW registers with byte strobes, a read-only status word, SLVERR for non-zero `prot`. 131 lines (163 before Handshake), five simulation tests, Verilator `-Wall` clean, 25 properties (11 hand-written + 14 automatic) proven with `--mode prove --depth 3 --engine boolector`, `bmc 12`, `cover 12`. |
| `fir_filter.volt` | FIR low-pass (kernel `const COEFFS : [i16; 8]`, DC gain 20) as a generic `FirFilter<const TAPS, const WIDTH>` (ADR-0041): `sint<WIDTH>` sample, `[sint<WIDTH>; TAPS]` tap line shifted by a `for`, MAC as a `comb` accumulation over `COEFFS[i]` — zero casts, the `: i32` targets widen the 16×16 products (same-sign widening). Monomorphised twice, `Fir8` = `FirFilter<8, 16>` and `Fir4` = `FirFilter<4, 16>` (SV modules `FirFilter_8_16`, `FirFilter_4_16`), plus `FirFilterPipe` (`pipeline(3)` Multiply → Add1 → Add2, 3-cycle latency). Ten simulation tests (impulse/step/full-scale/zero/valid gating for 8 and 4 taps, pipeline latency/valid), Verilator `-Wall` clean, contracts (no-overflow bound, valid delay, per-stage induction helpers, covers) pass `bmc 12` / `prove 4 --engine boolector` / `cover 12`. |
| `soc/` | Multi-module SoC (see [`soc/README.md`](soc/README.md)): `SocTop` → `BusDecoder` + `Gpio` + `Timer` + `UartCtrl` (`SyncFifo<u8,16>` + the reused `UartTx`) + the reused `Axi4LiteSlave`, one AXI4-Lite host port, four 256-byte pages, SLVERR outside the map. Ten instances, 133 port bindings, 24 forward `wire`s (bodies resolve top-down), 70 contracts. Written as a scale/composition test; since ADR-0042 the six per-module `.volt` files compile as one unit through `use` (`volt build examples/soc/top.volt`), with `UartTx` and `Axi4LiteSlave` reused by reference. Instance-name/port-name clashes (`timer` + `irq` vs port `timer_irq`) produced duplicate SV declarations silently. Since ADR-0050 the AXI channels are `Handshake<Payload>` bundles with automatic protocol contracts, which caught a decoder bug (owner switch while a response was pending). Verilator `-Wall` clean across all 8 modules, 5/5 simulation tests, 137 properties `bmc 12` / `prove 3 --engine boolector` / `cover 48`. |
| `vga/` | The first **two-clock** design (see [`vga/README.md`](vga/README.md)): 640x480 @ 60 Hz sync generator (`VgaTiming`, PixDomain only), an 80x60 1-bit frame buffer written from `SysDomain` and read from `PixDomain` (`FrameBuffer`), and a checkerboard top (`VgaTop`). Four crossings, all explicit: pixel writes through `AsyncFifo<u14,16>`, three 1-bit controls through `sync()`. The frame buffer is one `AsyncDualPortRam<bool,8192>` (ADR-0049: write port in SysDomain, read port in PixDomain, the array is the crossing) that maps to one RAMB18E1 with no other cells; the original FIFO-the-writes version that motivated the primitive is kept in the README as the record. Ten deliberate CDC violations were all caught (E3010, 9× E3001, W3003/W3002). `@timing(...)` parses but is not enforced (no float literal, no SDC) -- since ADR-0048 the compiler says so with W0021 instead of staying silent. 7 simulation tests, 1.22 M cycles (a full 420 k-cycle frame costs ~40 ms of run time; the 14 s wall is Verilator compile), both clocks driven as one by the harness. VgaTiming 8 properties `bmc 12` / `prove 3`; FrameBuffer 15 properties `bmc 24` (212 s under `multiclock on`); `prove` on the multi-clock modules is blocked by a formal-wrapper reset-ordering gap documented in the README. |
| `i2c/` | I2C master (see [`i2c/README.md`](i2c/README.md)) — the design that got `inout` writing and the `opendrain` port kind into the language (ADR-0051): 100/400 kHz, 7-bit addressing, single-byte write/read, ACK/NACK, START / STOP / REPEATED START, clock stretching. `opendrain sda`/`scl` pads driven with `sda.drive_low()` / `sda.release()`, read through `sync(sda.read(), clk)` (a direct read is W3007), contracts on the drive intent (`sda.released`). Pure Volt: the compiler emits `assign sda = sda_drive_low ? 1'b0 : 1'bz`; the test bench binds master and slave to two `wire`s that become `tri1` nets. 7-state FSM as a `match` in the `on clk` block, 4-phase bit divider from `const CLKS_PER_BIT_*` (3.2 MHz: 32 / 8 clocks per bit). Verilator `-Wall` clean (tri1 + 1'bz), 12/12 tests, 13 properties `bmc 12` / `prove 3 --engine boolector`, 7/7 covers at `cover 190`. |
| `crypto/` | AES key store (see [`crypto/README.md`](crypto/README.md)) — the first design with `trust_level` (ADR-0052): `SecureCore` (secret) and `Debug` (public) on one clock, the key on secret ports and registers, status bits reaching the host only through `declassify(expr, "reason")` (three W3008 audit warnings). `debug_out = key_r[7:0]` is refused with E3009 naming both trust levels; a register loaded from the key, a slice, or a write under a key-dependent `if` are caught the same way. Trust is type-level only: the generated SV is identical to the unannotated module. 65 lines of SV, Verilator `-Wall` clean, 3 invariants + 3 covers `bmc 12` / `prove 3` / `cover 12`. |

| `systolic/` | 4x4 output-stationary systolic array (see [`systolic/README.md`](systolic/README.md)) -- the first design written with **regular-structure** support (ADR-0056): two nested compile-time `for` loops instantiate the 16 `Pe`s (`pe_<y>_<x>`) and wire the link arrays; array-typed ports/wires (`[i8; N]`, `[i16; N * N]`) become packed SV vectors with `+:` part-selects. 96 lines of source (hand-written ~400), Verilator `-Wall` clean, 5 properties `bmc 8` / `prove 3 --engine boolector` / `cover 12`. |
| `hybrid_accel/` | Hybrid ternary/binary systolic accelerator, **design only** (no synthesis numbers yet): an 8x8 array of `TernaryPe` (weight in {-1, 0, +1}) in `TernaryCore` (400 MHz) feeding a 4x4 array of `BinaryPe` (`i8` weight) in `BinaryCore` (200 MHz) through `AsyncFifo<i32, 16>`; activations flow east, column sums flow south, both grids are two nested `for` loops (ADR-0056). The ternary product has **no multiplier** in the generated SV (`(weight == 2'sd1) ? act_w : ((weight == -2'sd1) ? act_neg : 9'sd0)`), the binary one is `16'(weight) * 16'(act)`. Wiring the ternary sum straight into the binary window is refused with E3001 (tried, then fixed with the FIFO). Findings: (1) `Trit` type-checks but the SV emitter rejects it (E0003), so the weight is carried as its `i2` encoding and the mux is hand-written; (2) unary minus on a widened operand is emitted as `-9'(act)`, which Yosys rejects ("static cast with negative size") -- written as `0 - x`; (3) an untyped `let product = weight * act` on `i8 * i8` is 8 bits wide (caught by the 100 * 100 test), the `: i16` target is required; (4) indexing an instance's array output (`ta.acc[x]`) emits a 1-bit select, and a `reg` array cannot be bound to an array port (unpacked vs packed) -- both go through a wire array; (5) `sync()` refuses an instance-field source. 429 lines of design in 5 files + 220 lines of tests, Verilator `-Wall` clean across 7 modules, 6/6 simulation tests (clocks lock-step 1:1), the six single-clock modules 26 properties `bmc 12` / `prove 3 --engine boolector` / `cover 24` (overflow bounds +-1024 / +-65536, FIFO-never-overflows proven on `TernaryCtl`). The two-clock `HybridTop` does **not** pass `volt verify`: the AsyncFifo contract's `always @(posedge clk) assume (!rst)` means rst is never seen on a clock edge under `multiclock on`, so registers start random and the FIFO's own occupancy invariant "fails at cycle 2" (covers are "reached" at step 2 the same way) -- the reset-ordering gap from `vga/README.md`, now with the cause. |

## Building

```
volt build examples/uart_tx.volt
```

The generated RTL lands in `build/rtl/uart_tx.sv`.

## Linting (Verilator, Docker)

`volt build` writes one `.sv` per module (`build/rtl/<Module>.sv`,
ADR-0024), so Verilator's `DECLFILENAME` check is satisfied as is:

```
volt build examples/uart_tx.volt          # build/rtl/UartTx.sv
docker run --rm -v "$PWD/build/rtl:/work" -w /work     verilator/verilator:latest --lint-only -Wall UartTx.sv
```

A source file with several modules (`fir_filter.volt`: `FirFilter_8_16`,
`FirFilter_4_16`, `Fir8`, `Fir4`, `FirFilterPipe`) yields one file per
module; lint the tops with the submodules on the command line:

```
docker run --rm -v "$PWD/build/rtl:/work" -w /work     verilator/verilator:latest --lint-only -Wall Fir8.sv FirFilter_8_16.sv
```

(Git Bash on Windows rewrites `/work`; prefix the `docker run` with
`MSYS_NO_PATHCONV=1`. `volt build --single-file` restores the old
`build/rtl/<source>.sv` layout.)

For a whole multi-file hierarchy build the top and pass every module
file, naming the top (`use` pulls the other files in, ADR-0042):

```
volt build examples/soc/top.volt          # 8 source files -> 8 .sv files
docker run --rm -v "$PWD/build/rtl:/work" -w /work     verilator/verilator:latest --lint-only -Wall --top-module SocTop     SocTop.sv BusDecoder.sv Gpio.sv Timer.sv UartCtrl.sv Axi4LiteSlave.sv AxiToReg.sv UartTx.sv
```

A design with bidirectional pins needs no hand-written SV since
ADR-0051: `opendrain` / `inout` ports generate their own tri-state
buffer (`examples/i2c/`):

```
volt build examples/i2c/i2c_master.volt   # build/rtl/I2cMaster.sv, inout wire sda + assign sda = ... : 1'bz
docker run --rm -v "$PWD/build/rtl:/work" -w /work     verilator/verilator:latest --lint-only -Wall --top-module I2cMaster I2cMaster.sv
```

## Simulation tests (Verilator, Docker)

`volt test examples/<name>_test.volt` compiles the test file together
with its sibling `<name>.volt` (or whatever the test file pulls in
through `use`, as `i2c/i2c_test.volt` does) and needs Verilator (`VOLT_VERILATOR` or
`PATH`). Without a local install, run the driver inside the Verilator
image (the cargo caches live in named volumes):

```
docker run --rm --entrypoint bash -v "$PWD:/work" \
    -v volt-cargo:/usr/local/cargo -v volt-rustup:/usr/local/rustup \
    -v volt-target:/work/target-linux -e CARGO_TARGET_DIR=/work/target-linux \
    -e RUSTUP_HOME=/usr/local/rustup -e CARGO_HOME=/usr/local/cargo \
    verilator/verilator -c 'export PATH=/usr/local/cargo/bin:$PATH; cd /work && cargo run -q -p volt-driver -- test examples/soc/top_test.volt'
```

## Formal verification (SymbiYosys, Docker)

Point `VOLT_SBY` at the Docker wrapper, then run the three modes:

```
VOLT_SBY=build/sby-docker.cmd volt verify --mode bmc   --depth 48 examples/uart_tx.volt
VOLT_SBY=build/sby-docker.cmd volt verify --mode prove --depth 4  examples/uart_tx.volt
VOLT_SBY=build/sby-docker.cmd volt verify --mode cover --depth 48 examples/uart_tx.volt
```

A full 8N1 frame at 4 clocks per bit takes ~38 cycles, so the `bmc`
and `cover` runs need `--depth 48` to reach the STOP state.
