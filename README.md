# Volt

An HDL where clock domain crossing bugs won't compile.

> **Status:** early-stage project (started 2026-09). Not used in production.
> No silicon. See [Limitations](#limitations).

![CI](https://github.com/volt-hdl/volt/actions/workflows/ci.yml/badge.svg)
![tests](https://img.shields.io/badge/tests-2420-brightgreen)
![diagnostics](https://img.shields.io/badge/diagnostic_codes-126-blue)
![license](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue)

Volt has a Rust-like syntax and compiles to readable SystemVerilog. Clock
domains are part of the type system: a signal crossing from one domain to
another without a synchronizer is a compile error, not a silicon bug.

## The problem

SystemVerilog compiles this without a single warning:

```systemverilog
always_ff @(posedge slow_clk)
    slow_reg <= fast_signal;   // compiles fine
```

If `fast_signal` comes from another clock domain, this bug usually stays
invisible through simulation and surfaces in silicon as intermittent
metastability failures.

`tests/ui/fail/01_cdc_violation.volt` assigns a signal from domain `@Fast`
to a port in domain `@Slow` without a synchronizer. `volt build` rejects it
(copied verbatim; two `W1001` unused-port warnings omitted):

```text
error[E3001]: direct assignment between clock domains
   ┌─ tests/ui/fail/01_cdc_violation.volt:21:5
   │
 5 │ domain Fast {
   │        ---- source @Fast defined here
   ·
10 │ domain Slow {
   │        ---- destination @Slow defined here
   ·
21 │     slow_data = fast_data
   │     ^^^^^^^^^   --------- @Fast
   │     │
   │     @Slow
   │
   = reason: the destination register may sample the source signal during an unstable window (metastability)
   = note: for multi-bit data, AsyncFifo may be safer
   = help: synchronize into the target domain with sync(): dest = sync(src, <clock of Slow>)
   = for more: volt explain E3001


     Error: build failed due to 1 error(s), 2 warning(s)
```

The build exits with code 1 and no SystemVerilog is produced. The fix is an
explicit bridge, `slow_data = sync(fast_data, slow_clk)`, which compiles to
a source-capture register plus a two-flop synchronizer in the target domain.
Single-clock modules need no domain annotations at all
([`tests/fixtures/counter.volt`](tests/fixtures/counter.volt)).

## What Volt checks at compile time

Each row is a failing test in `tests/ui/fail/`; `volt build` on that file
exits with code 1 and the listed error.

| Check | Error | Example |
|---|---|---|
| Clock domain crossing without a synchronizer | `E3001` | [`01_cdc_violation.volt`](tests/ui/fail/01_cdc_violation.volt) |
| Pipeline alignment: combining values of different latency (`Delayed<T, N>`, opt-in with `@strict_timing`) | `E5010` | [`31_delay_mismatch.volt`](tests/ui/fail/31_delay_mismatch.volt) |
| Information flow: `secret` data reaching a `public` output | `E3009` | [`55_trust_leak.volt`](tests/ui/fail/55_trust_leak.volt) |
| Handshake protocol: `valid` derived combinationally from `ready` | `E4007` | [`52_handshake_protocol_violation.volt`](tests/ui/fail/52_handshake_protocol_violation.volt) |

The information-flow check, verbatim:

```text
error[E3009]: secret data flows to a public output
   ┌─ tests/ui/fail/55_trust_leak.volt:31:5
   │
 9 │     trust_level = secret
   │     -------------------- source trust level here
   ·
15 │     trust_level = public
   │     -------------------- destination trust level here
   ·
31 │     debug_out = key_r
   │     ^^^^^^^^^   ----- @SecureCore (secret)
   │     │
   │     @Debug (public)
   │
   = reason: information from a higher trust level cannot reach a lower one; this could leak key material (ADR-0052)
   = help: if intentional, use declassify(expr, "reason")
   = for more: volt explain E3009
```

All 126 diagnostic codes have a long-form explanation built into the
compiler, in English and Turkish: `volt explain E3009`, `volt explain
E3009 --lang=tr`.

## A CPU that runs C

[`examples/riscv_core.volt`](examples/riscv_core.volt) is an RV32IM + Zicsr
single-cycle core with traps, one external interrupt and a memory-mapped
UART. [`examples/riscv_sw/`](examples/riscv_sw/) holds a freestanding C
program built with a stock GCC (`-march=rv32im -O2`, no libc):

```c
int main(void)
{
    puts("Hello from Volt!\n");

    volatile int a = 7, b = 6, ten = 10;
    int c = a * b;                 /* MUL -> 42 */
    putchar('0' + c / ten);        /* DIV -> 4  */
    putchar('0' + c % ten);        /* REM -> 2  */
    putchar('\n');
    return 0;
}
```

The test loads the committed `hello.hex` into the simulated SoC, captures
the UART output byte by byte and checks it against `Hello from Volt!\n42\n`.
Run in the `verilator/verilator` Docker image (recipe in
[`examples/README.md`](examples/README.md)), output trimmed:

```console
$ volt test riscv_core_test.volt
   Compiling riscv_core_test.volt
running 59 tests
test reset_drives_pc_to_zero ... ok
test nop_advances_pc_by_four ... ok
...
test uart_status_readable ... ok
test hello_program_prints_greeting ... ok

test result: ok. 59 passed; 0 failed
```

The same test asserts `cycles == 913` and `instrs == 847`: the program
executes 847 instructions in 913 cycles, with no trap and no stray bus
access. `hello.hex` is committed, so no RISC-V toolchain is needed to run it.

## Formal verification

Contracts are part of the language: `requires`, `ensures`, `invariant`,
`cover`, and `prev(x, N)` for values from earlier cycles. `volt verify`
generates a model and an `.sby` script, runs SymbiYosys and maps the result
back to the source line.

```console
$ volt verify tests/ui/pass/23_provable_invariant.volt
   Verifying tests/ui/pass/23_provable_invariant.volt
     [1/1] BoundedCounter (1 property) ... ok (0.35s)
    Finished 1.40s
      Result 1 property verified in 1.4s (16 jobs; bmc, depth 20)
```

A violated contract exits with code 6 and points at the contract, with a
counterexample VCD (output trimmed):

```console
$ volt verify tests/ui/fail/24_violated_invariant.volt
error[E5001]: contract violated
   ┌─ tests/ui/fail/24_violated_invariant.volt:12:16
   │
12 │     invariant: count_r < 5
   │                ^^^^^^^^^^^ violated at cycle 7
   │
   = counterexample: build\formal\leakycounter_cex.vcd
```

Limits: SymbiYosys, Yosys and an SMT solver must be installed separately
(Linux; WSL or Docker on Windows). The engine is `smtbmc` (solver z3,
boolector or yices). Sequential properties are limited to `prev()`; there
are no sequences (`##`) and no liveness properties.

## Hardware/software bridge

An `@mmio` register map generates the AXI4-Lite slave in the RTL and the
matching drivers (`volt build --emit=rust,c,regmap,regmap-md`). From
[`tests/ui/pass/72_mmio_driver_generation.volt`](tests/ui/pass/72_mmio_driver_generation.volt):

```text
@mmio(base = 0x4001_0000, bus = AXI4Lite)
module PwmRegs {
    /// Duty threshold: pwm is high while count < duty.
    @reg(offset = 0x04, access = ReadWrite)
    duty : { value : u16, @reserved : bits<16> }
    ...
}
```

Selected lines of the generated `build/sw/pwm_regs.rs` (`no_std`):

```rust
impl PwmRegs {
    pub const BASE: usize = 0x4001_0000;
    pub const DUTY_OFFSET: usize = 0x04;
    pub const DUTY_MASK: u32 = 0x0000_FFFF;
    // ...
    pub fn control_enable(&self) -> bool {
        (self.read(Self::CONTROL_OFFSET) & 0x1) != 0
    }
    // ...
}
```

Limits: the bus is AXI4-Lite and registers are 32 bits wide. No SVD,
IP-XACT or UVM output.

## Other features

- `pipeline(N)` blocks with generated stage registers — [ADR-0038](docs/adr/ADR-0038-pipeline-sozdizimi.md), [`examples/fir_filter.volt`](examples/fir_filter.volt)
- `Delayed<T, N>` latency types — [ADR-0037](docs/adr/ADR-0037-l1-zamanlama.md), [`tests/ui/pass/44_delayed_aligned.volt`](tests/ui/pass/44_delayed_aligned.volt)
- Port bundles (`struct port`) — [ADR-0039](docs/adr/ADR-0039-bundle-port-gruplari.md), [`tests/ui/pass/47_bundle_basic.volt`](tests/ui/pass/47_bundle_basic.volt)
- Enum state types with exhaustive `match` and a generated state-valid invariant — [ADR-0074](docs/adr/ADR-0074-enum-destegi.md), [`examples/uart_tx.volt`](examples/uart_tx.volt), [`examples/i2c/`](examples/i2c/)
- `Handshake<T>` with generated valid/ready contracts — [ADR-0050](docs/adr/ADR-0050-handshake-primitifi.md), [`examples/axi4lite_slave.volt`](examples/axi4lite_slave.volt)
- `inout` / `opendrain` ports — [ADR-0051](docs/adr/ADR-0051-cift-yonlu-portlar.md), [`examples/i2c/`](examples/i2c/)
- SDC/XDC constraint output (`--emit=sdc,xdc`) — [ADR-0054](docs/adr/ADR-0054-sdc-uretimi.md), [`tests/ui/pass/74_sdc_multi_clock.volt`](tests/ui/pass/74_sdc_multi_clock.volt)
- Test language (`test` blocks, `read_hex`, `load`) run on Verilator — [ADR-0033](docs/adr/ADR-0033-test-bloklari-ve-simulasyon.md), [ADR-0058](docs/adr/ADR-0058-test-dili-genisletme.md), [`examples/uart_tx_test.volt`](examples/uart_tx_test.volt)
- 12 built-in standard library primitives (FIFOs, RAMs, arbiters, synchronizers) — [ADR-0027](docs/adr/ADR-0027-stdlib-mimarisi.md), [docs/stdlib.md](docs/stdlib.md)
- Language server (`volt lsp`) and a VS Code extension — [`editors/vscode/`](editors/vscode/)

More designs: [`examples/README.md`](examples/README.md).

## Limitations

- **RDC checking covers reset release, not reset ordering.** `E3003`
  reports an asynchronous reset port shared by several clock domains, a raw
  reset synchronized twice on one clock, and a raw reset port that does not
  match the domain it feeds. `W3009` marks an asynchronous reset whose
  release is assumed to be synchronized outside the unit, `W3010` a
  synchronous reset shared by several clocks (a warning, not an error).
  Reset sequencing (`E3004`) and conditional resets (`E3005`) are reserved
  and never emitted; `extern` modules carry no reset contract. The
  generated `.sdc`/`.xdc` write no `set_clock_groups`, so a crossing the
  checker misses stays visible to the timing tool; CI proves this with
  OpenSTA on an injected crossing (ADR-0065).
- **No built-in simulator.** `volt run` and `volt test` require Verilator.
- **SystemVerilog is the sole output language.** No VHDL.
- **Some constructs are not yet emitted.** A few constructs pass the type
  checker but are rejected at SystemVerilog generation with `E0003`.
  Structs are not a signal type yet (ports, `reg`, `wire`, `let`; port
  bundles are the exception). Enums with plain variants are
  (`enum State { Idle, Run }`, explicit codes with `enum Op : u7 { ... }`,
  ADR-0074); enums with data-carrying variants, generic enums and enum
  arrays are not.
- **Sequential properties are limited to `prev()`.** No sequences, no
  liveness.
- **Formal verification requires SymbiYosys** (Linux; WSL or Docker on
  Windows); the timing proof requires Yosys and OpenSTA (Docker). Both CI
  jobs are required.
- **Register maps support AXI4-Lite with 32-bit registers**; reset values
  are always 0.
- **CDC bridges are limited to `sync()` / `sync3()` and the built-in
  dual-clock primitives.** User-written synchronizers are not recognized;
  a multi-bit `sync()` is a warning (`W3003`), not an error.
- **The VS Code extension is not published** (local install from
  `editors/vscode/`).
- **Single maintainer, no external users yet.** No published crate, no
  paper, no FPGA board or tape-out results.

## How Volt compares

Several HDLs check clock domain crossings at compile time, including Clash,
Bluespec, Veryl, Arch and SpinalHDL. SpinalHDL and Arch also offer pipeline
constructs and formal flows. If you need a mature tool today, look at those
before Volt.

As of 2026-09 we found no compile-time information-flow checking combined
with compile-time CDC checking in the production-oriented HDLs we surveyed
(Clash, Bluespec, Veryl, Arch, Spade, Chisel, Amaranth, SpinalHDL, Hardcaml,
TL-Verilog, PipelineC, ROHD); see
[docs/research/rekabet-2026-09.md](docs/research/rekabet-2026-09.md).
Academic languages (SecVerilog, ChiselFlow) go further on information flow,
with formal noninterference proofs.

Full comparison with sources:
[docs/research/rekabet-2026-09.md](docs/research/rekabet-2026-09.md)
(in Turkish).

## Installation

```console
$ git clone https://github.com/volt-hdl/volt
$ cd volt
$ cargo build --release
$ ./target/release/volt build tests/fixtures/counter.volt
   Compiling tests/fixtures/counter.volt
    Finished 0.00s (1 source file(s), 1 SV file(s))
     Output build\rtl\Counter.sv (35 lines)
```

Requires stable Rust. `volt build`, `volt check`, `volt explain` and
`volt lsp` need nothing else. Optional external tools:

- **Verilator** for `volt run` and `volt test`
- **SymbiYosys** (with Yosys and z3, boolector or yices) for `volt verify`
- On **Windows**, run both through WSL or Docker (`verilator/verilator`,
  `hdlc/formal`); `volt explain simulation-setup` and `volt explain
  verify-setup` print the setup guides

## Documentation

- [docs/spec/](docs/spec/): the binding language specification (grammar,
  type inference, domain inference, SV mapping, CLI contract)
- [docs/adr/](docs/adr/): 57 architecture decision records (in Turkish)
- [docs/stdlib.md](docs/stdlib.md): standard library reference
- [examples/README.md](examples/README.md): example designs with
  verification results
- [docs/README.md](docs/README.md): full document index

## License

Licensed under either of the Apache License, Version 2.0
([LICENSE-APACHE](LICENSE-APACHE)) or the MIT license
([LICENSE-MIT](LICENSE-MIT)), at your option.
