# Volt Examples

Standalone Volt designs that exercise the language end to end:
compile to SystemVerilog, lint with Verilator, and formally verify
the contracts with `volt verify`.

## Examples

| File | Demonstrates |
|---|---|
| `riscv_pipeline.volt` | RV32I 5-stage pipeline (12-instruction subset), hand-built: manual inter-stage registers, EX→EX / MEM→EX forwarding plus the WB→ID bypass, load-use stall, branch/JAL flush. 10 inductive invariants (x0, PC alignment, per-stage wb_en/rd validity) proven with `--mode prove --depth 3 --engine boolector`; `stall_o`/`flush_o` debug ports work around contracts only seeing ports and registers. |
| `uart_tx.volt` | UART transmitter (8N1). Four-state FSM written as a `match` inside the sequential block (ADR-0032), a `u10` baud counter compared directly against the `CLKS_PER_BIT` constant (ADR-0031), start/busy handshake, LSB-first shift register, safety invariants proven by induction (`--mode prove`), and `cover` targets showing all four states are reachable. |
| `axi4lite_slave.volt` | AXI4-Lite register slave built from five `struct port` bundles (ADR-0039) with sequential protocol contracts via `prev()` (ADR-0040: responses held until accepted, every accepted request answered next cycle, master hold rules as assumptions): one master-view definition per channel, the slave declares each with `in` so every field direction flips; the generated SV is flat (`aw_addr`, `aw_ready`, ...). Four RW registers with byte strobes, a read-only status word, SLVERR for non-zero `prot`. Five simulation tests, Verilator `-Wall` clean, 16 contracts (9 invariants + 3 assumptions + 4 covers) proven with `--mode prove --depth 3 --engine boolector`. |
| `fir_filter.volt` | FIR low-pass (kernel `const COEFFS : [i16; 8]`, DC gain 20) as a generic `FirFilter<const TAPS, const WIDTH>` (ADR-0041): `sint<WIDTH>` sample, `[sint<WIDTH>; TAPS]` tap line shifted by a `for`, MAC as a `comb` accumulation over `COEFFS[i]` — zero casts, the `: i32` targets widen the 16×16 products (same-sign widening). Monomorphised twice, `Fir8` = `FirFilter<8, 16>` and `Fir4` = `FirFilter<4, 16>` (SV modules `FirFilter_8_16`, `FirFilter_4_16`), plus `FirFilterPipe` (`pipeline(3)` Multiply → Add1 → Add2, 3-cycle latency). Ten simulation tests (impulse/step/full-scale/zero/valid gating for 8 and 4 taps, pipeline latency/valid), Verilator `-Wall` clean, contracts (no-overflow bound, valid delay, per-stage induction helpers, covers) pass `bmc 12` / `prove 4 --engine boolector` / `cover 12`. |

## Building

```
volt build examples/uart_tx.volt
```

The generated RTL lands in `build/rtl/uart_tx.sv`.

## Linting (Verilator, Docker)

Verilator's `DECLFILENAME` check wants the file named after the module,
so copy before linting:

```
cp build/rtl/uart_tx.sv build/rtl/UartTx.sv
docker run --rm -v "$PWD/build/rtl:/work" -w /work \
    verilator/verilator:latest --lint-only -Wall UartTx.sv
```

A file with several modules (`fir_filter.volt`: `FirFilter_8_16`,
`FirFilter_4_16`, `Fir8`, `Fir4`, `FirFilterPipe`) is emitted as one
`.sv`; split it per module before linting so `DECLFILENAME` stays
happy, and lint the tops with the submodules on the command line:

```
awk '/^module [A-Za-z0-9_]+ \(/{f=$2".sv"} f{print > f} /^endmodule/{f=""}' build/rtl/fir_filter.sv
docker run --rm -v "$PWD/build/rtl:/work" -w /work \
    verilator/verilator:latest --lint-only -Wall Fir8.sv FirFilter_8_16.sv
```

(Git Bash on Windows rewrites `/work`; prefix the `docker run` with
`MSYS_NO_PATHCONV=1`.)

## Formal verification (SymbiYosys, Docker)

Point `VOLT_SBY` at the Docker wrapper, then run the three modes:

```
VOLT_SBY=build/sby-docker.cmd volt verify --mode bmc   --depth 48 examples/uart_tx.volt
VOLT_SBY=build/sby-docker.cmd volt verify --mode prove --depth 4  examples/uart_tx.volt
VOLT_SBY=build/sby-docker.cmd volt verify --mode cover --depth 48 examples/uart_tx.volt
```

A full 8N1 frame at 4 clocks per bit takes ~38 cycles, so the `bmc`
and `cover` runs need `--depth 48` to reach the STOP state.
