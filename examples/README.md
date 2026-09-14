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
| `soc/` | Multi-module SoC (see [`soc/README.md`](soc/README.md)): `SocTop` → `BusDecoder` + `Gpio` + `Timer` + `UartCtrl` (`SyncFifo<u8,16>` + the reused `UartTx`) + the reused `Axi4LiteSlave`, one AXI4-Lite host port, four 256-byte pages, SLVERR outside the map. Ten instances, 133 port bindings, 24 forward `wire`s (bodies resolve top-down), 70 contracts. Written as a scale/composition test; since ADR-0042 the six per-module `.volt` files compile as one unit through `use` (`volt build examples/soc/top.volt`), with `UartTx` and `Axi4LiteSlave` reused by reference. Instance-name/port-name clashes (`timer` + `irq` vs port `timer_irq`) produced duplicate SV declarations silently. Verilator `-Wall` clean across all 8 modules, 5/5 simulation tests, 74 properties `bmc 12` / `prove 3 --engine boolector` / `cover 48`. |
| `vga/` | The first **two-clock** design (see [`vga/README.md`](vga/README.md)): 640x480 @ 60 Hz sync generator (`VgaTiming`, PixDomain only), an 80x60 1-bit frame buffer written from `SysDomain` and read from `PixDomain` (`FrameBuffer`), and a checkerboard top (`VgaTop`). Four crossings, all explicit: pixel writes through `AsyncFifo<u14,16>`, three 1-bit controls through `sync()`. The stdlib `DualPortRam` is single-clock and a `reg` array read from the other domain is E3001, so the memory stays in PixDomain and the *writes* cross — the only shape the type system accepts; it maps to one RAMB18E1. Ten deliberate CDC violations were all caught (E3010, 9× E3001, W3003/W3002). `@timing(...)` parses and is ignored (no float literal, no SDC). 7 simulation tests, 1.22 M cycles (a full 420 k-cycle frame costs ~40 ms of run time; the 14 s wall is Verilator compile), both clocks driven as one by the harness. VgaTiming 8 properties `bmc 12` / `prove 3`; FrameBuffer 15 properties `bmc 24` (212 s under `multiclock on`); `prove` on the multi-clock modules is blocked by a formal-wrapper reset-ordering gap documented in the README. |

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

## Simulation tests (Verilator, Docker)

`volt test examples/<name>_test.volt` compiles the test file together
with its sibling `<name>.volt` and needs Verilator (`VOLT_VERILATOR` or
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
