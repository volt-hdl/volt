# {{name}}

A Volt project created with `volt new --template fifo`: a packet buffer
on the built-in `SyncFifo`. A `struct Packet` is packed into the FIFO
with `as u12` and unpacked with `as Packet`; a sticky `overflow` flag
records a push into a full buffer.

```
volt check packet_buffer.volt     # check the design
volt build packet_buffer.volt     # SystemVerilog in build/rtl/
volt test                         # run packet_buffer_test.volt (needs Verilator)
volt verify packet_buffer.volt    # prove the contracts (needs SymbiYosys)
```

`volt doctor` tells you which of these work on this machine.

| File | What |
|---|---|
| `packet_buffer.volt` | `struct`, `SyncFifo<u12, 8>`, an `invariant`, `cover`s |
| `packet_buffer_test.volt` | simulation tests with whole-struct values |
| `Volt.toml` | package manifest |

The compiler generates the FIFO's body and its own contracts (no
overflow, no underflow). For two clocks use `AsyncFifo<T, DEPTH>` with
`wr_clk`/`rd_clk` — see `volt new --template cdc` and
`volt explain stdlib`.
