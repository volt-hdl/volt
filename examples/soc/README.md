# SoC example — multi-file composition

A small system-on-chip that exercises Volt at the scale of a real design:
eight modules (six written here across six files, two reused from
`examples/` by reference, plus the stdlib FIFO), one AXI4-Lite host port,
an address decoder and four peripherals on a single clock. The point of
the exercise is composition — instantiation, name resolution across
files, multi-file layout, lint of a whole hierarchy — rather than any
single peripheral.

```
SocTop                      top.volt        package soc::top
 ├── BusDecoder             bus.volt        package soc::bus    addr[31:8] -> peripheral, SLVERR outside the map
 ├── Gpio                   gpio.volt       package soc::gpio   0x0000-0x00FF  DATA_OUT / DIR / DATA_IN
 │    └── AxiToReg          axi.volt        package soc::axi    AXI4-Lite subset -> RegBus
 ├── Timer                  timer.volt      package soc::timer  0x0100-0x01FF  CTRL / COUNT / COMPARE / STATUS, irq
 │    └── AxiToReg
 ├── UartCtrl               uart.volt       package soc::uart   0x0200-0x02FF  TXDATA / STATUS
 │    ├── AxiToReg
 │    ├── SyncFifo<u8, 16>  stdlib
 │    └── UartTx            ../uart_tx.volt          (reused by reference)
 └── Axi4LiteSlave          ../axi4lite_slave.volt   (reused by reference)  0x0300-0x03FF
```

Every peripheral is itself a complete AXI4-Lite slave, so the decoder
only steers `*_valid` / `*_ready` / `*_resp` / `r_data`; `addr`, `data`,
`strb` and `prot` fan out from the host to all of them.

## Files

| File | Package | Content |
|---|---|---|
| `top.volt` | `soc::top` | `SocTop` — the entry point of the build |
| `bus.volt` | `soc::bus` | `BusDecoder` (+ the file-private `AxiSel` bundle) |
| `axi.volt` | `soc::axi` | `RegBus`, `AxiReadCtl` bundles, `AxiToReg` bridge; imports the five AXI channel bundles from `../axi4lite_slave.volt` |
| `gpio.volt` | `soc::gpio` | `Gpio` |
| `timer.volt` | `soc::timer` | `Timer` |
| `uart.volt` | `soc::uart` | `UartCtrl` (bridge + FIFO + `UartTx` from `../uart_tx.volt`) |
| `top_test.volt` | — | five simulation tests (sibling of `top.volt`) |
| `../Volt.toml` | — | `[package] name = "examples", src = "."` — makes `examples/` the search root |

There is no generated file any more: `volt build examples/soc/top.volt`
follows the `use` lines (ADR-0042). `use soc::gpio::Gpio` is looked up
as `./soc/gpio.volt` next to `top.volt`, then as
`examples/./soc/gpio.volt` under the `Volt.toml` root; `use
axi4lite_slave::{...}` resolves to `examples/axi4lite_slave.volt` the
same way. Items crossing a file boundary are `pub`; `AxiSel` is not and
stays private to `bus.volt`.

## Building, linting, testing, verifying

```
volt build examples/soc/top.volt
#    Finished 0.02s (8 source file(s), 8 SV file(s))
#    build/rtl/SocTop.sv, BusDecoder.sv, AxiToReg.sv, Gpio.sv, Timer.sv,
#    UartCtrl.sv, UartTx.sv, Axi4LiteSlave.sv      (one module per file, ADR-0024)

# Verilator lints the whole hierarchy directly — no awk split needed
MSYS_NO_PATHCONV=1 docker run --rm -v "$PWD/build/rtl:/work" -w //work verilator/verilator:latest \
    --lint-only -Wall --top-module SocTop SocTop.sv BusDecoder.sv Gpio.sv Timer.sv UartCtrl.sv \
    Axi4LiteSlave.sv AxiToReg.sv UartTx.sv

volt test examples/soc/top_test.volt                   # 5 tests (Verilator; Docker recipe in ../README.md)

VOLT_SBY=build/sby-docker.cmd volt verify --mode bmc   --depth 12 examples/soc/top.volt
VOLT_SBY=build/sby-docker.cmd volt verify --mode prove --depth 3 --engine boolector examples/soc/top.volt
VOLT_SBY=build/sby-docker.cmd volt verify --mode cover --depth 48 examples/soc/top.volt
```

Results after the multi-file rewrite (2026-09-13): Verilator `-Wall`
clean for all eight modules linted together, no DECLFILENAME; 5/5 tests
pass; 74 properties (own contracts + the stdlib FIFO's) pass `bmc 12`
(24.6 s wall including Docker start-up). Build time 0.02–0.09 s for the
eight files (debug binary, Windows) — process start dominates, the
compiler itself is well under 10 ms.

## Findings

This example was written as a scale test and every friction point was
kept in the code as a comment. The short version:

### Multi-file (`use`) — resolved by ADR-0042

Before: `volt build` accepted one file, `use` was parsed and forgotten,
and the SoC lived in a 1005-line `soc.volt` produced by `flatten.sh`,
which also copied `UartTx` and `Axi4LiteSlave` verbatim. Now the six
files compile as one unit; the two reused modules are imported by
reference (their SV carries `// Source:  uart_tx.volt`); a private item
imported from another file is `E1004`, a missing package is `E1011` with
the searched paths listed, an import cycle is `E1006`.

Still open: the root scope is shared across the unit, so two files
cannot each define a private item with the same name (E1003); a
per-package namespace is a follow-up ADR.

### Instantiation

Ten instances, 133 port bindings, 130 instance-output reads, 24 forward
`wire`s. Two things dominate the boilerplate:

- **Bundles cannot be bound as a whole** (ADR-0039 limit). Passing the
  five AXI channels to one instance is 12 `field: expr` bindings plus 7
  `chan.field = inst.chan_field` assignments for the outputs — 19 lines
  per AXI slave, written five times. The natural `aw: aw, w: w, ...`
  would be 5 lines.
- **Module bodies resolve top-down** (name-resolution.md §5), so two
  instances that feed each other need forward `wire` declarations
  assigned after the instances: 22 wires + 22 assignments in `SocTop`,
  2 + 2 in `UartCtrl`.

### Name resolution

- `inst.port` works for outputs and is the only cross-module access.
  There is no `top.dec_i.wsel_r`; internal state is observed through the
  bus.
- **Silent name clash, wrong SV.** Instance outputs are emitted as
  `<inst>_<port>` signals. The instances `timer` and `uart` in a module
  with ports `timer_irq` and `uart_tx` produced duplicate `logic`
  declarations; `volt build` reports nothing, Verilator rejects the
  file. The instances are now `timer_i`, `uart_i`, ...
- Contracts see only ports and registers (`tx_busy` wire in an
  invariant is E1001).

### SV output

One file per module (`build/rtl/<Module>.sv`, ADR-0024), header names
the module and its own source file. Module names are preserved, the
hierarchy is readable (`SocTop.uart_i.br` in Verilator messages).
Lint-driven design changes remain: an unused instance output
(`sys_i.reg0`) and partially used instance outputs are Verilator
warnings Volt cannot silence (no `_` on a binding, no pragma).

### What would help next

- **Bundle-level binding**: `aw: aw` and `slave.aw` on the instance
  side, and `[AxiSel; 4]` arrays of bundles for the decoder.
- **Order-independent module bodies** — removes all 24 wires.
- **Per-package namespaces** so that private helper names may repeat
  across files.
- **`onehot0(...)`** / `at_most_one` contract builtin; `@mmio` register
  maps; `Handshake<T>` in the stdlib; instance-name clash detection;
  `match` as an expression in combinational assignments.
