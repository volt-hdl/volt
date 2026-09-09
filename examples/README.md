# Volt Examples

Standalone Volt designs that exercise the language end to end:
compile to SystemVerilog, lint with Verilator, and formally verify
the contracts with `volt verify`.

## Examples

| File | Demonstrates |
|---|---|
| `uart_tx.volt` | UART transmitter (8N1). Four-state FSM written as a `match` inside the sequential block (ADR-0032), a `u10` baud counter compared directly against the `CLKS_PER_BIT` constant (ADR-0031), start/busy handshake, LSB-first shift register, safety invariants proven by induction (`--mode prove`), and `cover` targets showing all four states are reachable. |

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

## Formal verification (SymbiYosys, Docker)

Point `VOLT_SBY` at the Docker wrapper, then run the three modes:

```
VOLT_SBY=build/sby-docker.cmd volt verify --mode bmc   --depth 48 examples/uart_tx.volt
VOLT_SBY=build/sby-docker.cmd volt verify --mode prove --depth 4  examples/uart_tx.volt
VOLT_SBY=build/sby-docker.cmd volt verify --mode cover --depth 48 examples/uart_tx.volt
```

A full 8N1 frame at 4 clocks per bit takes ~38 cycles, so the `bmc`
and `cover` runs need `--depth 48` to reach the STOP state.
