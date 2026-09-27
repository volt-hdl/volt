# {{name}}

A Volt project created with `volt new --template cdc`: two clock
domains and a safe crossing. Pulses arrive in the fast domain, a toggle
carries each one through a `sync()` two-flop synchronizer, and the slow
domain counts them.

```
volt check event_counter.volt     # check the design
volt build event_counter.volt     # SystemVerilog in build/rtl/
volt test                         # run event_counter_test.volt (needs Verilator)
volt verify event_counter.volt    # prove the contracts (needs SymbiYosys)
```

`volt doctor` tells you which of these work on this machine.

| File | What |
|---|---|
| `event_counter.volt` | two `domain`s, a raw reset, `sync()` |
| `event_counter_test.volt` | simulation tests (all clocks advance together) |
| `Volt.toml` | package manifest |

Try it: replace `sync(toggle_r, slow_clk)` with `toggle_r` and run
`volt check` — the crossing is rejected with E3001. `sync()` is for a
single bit; multi-bit data crosses through the built-in `AsyncFifo`
(`volt explain domains`, `volt explain stdlib`).
