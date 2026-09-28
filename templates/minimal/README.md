# {{name}}

A Volt project created with `volt new --template minimal`: an 8-bit
counter with an enable input, simulation tests and two contracts.

```
volt check                  # check the design
volt build                  # SystemVerilog in build/rtl/
volt test                   # run counter_test.volt (needs Verilator)
volt verify                 # prove the contracts (needs SymbiYosys)
```

The commands work on the project (Volt.toml): `volt check` checks every
source file; `volt build`, `volt verify` and `volt run` use `top = "Counter"`.
A file still works too: `volt check counter.volt`. When a test fails, the report
ends with a `Waveform gtkwave ...` line; `volt test --watch` re-runs the
tests on every save.

`volt doctor` tells you which of these work on this machine and what to
install for the others.

| File | What |
|---|---|
| `counter.volt` | the design: `Counter`, a `fn`, an `invariant` and a `cover` |
| `counter_test.volt` | simulation tests (`X_test.volt` sees the modules of `X.volt`) |
| `Volt.toml` | package manifest |

Next: change `MAX` in `counter.volt` and watch a test fail, then fix
the test. `volt explain getting-started` and `volt explain contracts`
explain the language.
