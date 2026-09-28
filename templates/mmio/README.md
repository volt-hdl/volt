# {{name}}

A Volt project created with `volt new --template mmio`: an LED blinker
behind an AXI4-Lite register map. `@mmio` and `@reg` generate the bus
slave, the address decode and their contracts; the same map generates
the firmware drivers.

```
volt check                                           # check the design
volt build --emit=c,rust,regmap,regmap-md
                                                     # RTL + drivers in build/sw/, docs in build/docs/
volt test                                            # run blinker_test.volt (needs Verilator)
volt verify                                          # prove the contracts (needs SymbiYosys)
```

The commands work on the project (Volt.toml): `volt check` checks every
source file; `volt build`, `volt verify` and `volt run` use `top = "Blinker"`.
A file still works too: `volt check blinker.volt`. When a test fails, the report
ends with a `Waveform gtkwave ...` line; `volt test --watch` re-runs the
tests on every save.

`volt doctor` tells you which of these work on this machine.

| File | What |
|---|---|
| `blinker.volt` | `@mmio` map (control, period, status), `match` |
| `blinker_test.volt` | bus-level simulation tests (write, read, blink) |
| `Volt.toml` | package manifest |

Keep firmware in sync with the hardware:
`volt check-regmap blinker.volt --against build/sw/blinker.h` fails
when the register map and a generated driver drift apart.
