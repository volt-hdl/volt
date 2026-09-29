# What we learned

<div class="chapter-goal">

A short look back at the Tour, and where the Tutorial picks up.

</div>

In about 15 minutes you have:

- installed a single binary and asked `volt doctor` what works on your
  machine;
- created a project with `volt new`, checked it with `volt check` and
  turned it into readable SystemVerilog with `volt build`;
- written a design with two clock domains, seen the compiler reject a
  crossing without a synchronizer (`E3001`), and fixed it with `sync()`;
- run simulation tests with `volt test`, with Verilator running in
  Docker;
- followed a failing test to its waveform, where a state register shows
  the names of its states.

The commands you used:

| Command | What it does | Needs |
|---|---|---|
| `volt new NAME` | creates a project from a template | nothing |
| `volt check` | finds errors, writes no files | nothing |
| `volt build` | writes SystemVerilog to `build/rtl/` | nothing |
| `volt test` | runs the `test` blocks of the project | Verilator or Docker |
| `volt explain CODE` | explains an error code or a topic | nothing |
| `volt doctor` | says which commands work here | nothing |

Two commands did not come up: `volt verify`, which proves contracts for
every input sequence, and `volt run`, which simulates a design without
tests. The Tutorial covers both.

## Next

[Part II, the Tutorial](../tutorial/counter.md), starts again from a blank
file and builds a counter line by line: modules, ports, registers, clocked
blocks, bit widths and tests. Each later chapter adds one idea on top.
