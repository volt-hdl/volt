# {{name}}

A Volt project created with `volt new --template fifo`: a packet buffer
on the built-in `SyncFifo`. The FIFO stores whole `struct Packet`
values (one 12-bit word each in hardware); a sticky `overflow` flag
records a push into a full buffer.

```
volt check                        # check the design
volt build                        # SystemVerilog in build/rtl/
volt test                         # run packet_buffer_test.volt (needs Verilator)
volt verify                       # prove the contracts (needs SymbiYosys)
```

The commands work on the project (Volt.toml): `volt check` checks every
source file; `volt build`, `volt verify` and `volt run` use `top = "PacketBuffer"`.
A file still works too: `volt check packet_buffer.volt`. When a test fails, the report
ends with a `Waveform gtkwave ...` line; `volt test --watch` re-runs the
tests on every save.

`volt doctor` tells you which of these work on this machine.

| File | What |
|---|---|
| `packet_buffer.volt` | `struct`, `SyncFifo<Packet, 8>`, an `invariant`, `cover`s |
| `packet_buffer_test.volt` | simulation tests with whole-struct values |
| `Volt.toml` | package manifest |

The compiler generates the FIFO's body and its own contracts (no
overflow, no underflow). For two clocks use `AsyncFifo<T, DEPTH>` with
`wr_clk`/`rd_clk` — see `volt new --template cdc` and
`volt explain stdlib`.
