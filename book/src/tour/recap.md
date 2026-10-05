# What we learned

<div class="chapter-goal">

A short look back at the Tour, and where the Tutorial picks up.

</div>

<div class="before-you-start">

**Before you start**

- **Window:** none. This page has nothing to type.
- **Folder:** none. Your work is in `volt-projects`: the project `blinky`
  and the single design in `cdc`.
- **Running:** nothing. You may close Docker Desktop until the next
  `volt test`.

</div>

In Setup you installed a single binary and asked `volt doctor` what works
on your machine. Then, in about 15 minutes, you have:

- created a project with `volt new`, checked it with `volt check` and
  turned it into readable SystemVerilog with `volt build`;
- written a design with two clock domains in a folder of its own, seen
  the compiler reject a crossing without a synchronizer (`E3001`), and
  fixed it with `sync()`;
- run simulation tests with `volt test`, with Verilator running in
  Docker, and read the cover summary;
- followed a failing test to its waveform, where a state register shows
  the names of its states.

The commands you used:

| Command | What it does | Needs |
|---|---|---|
| `cd FOLDER` | makes `FOLDER` the working folder of the terminal | nothing |
| `volt new NAME` | creates a project from a template | nothing |
| `volt check` | finds errors, writes no files | nothing |
| `volt build` | writes SystemVerilog to `build/rtl/` | nothing |
| `volt test` | runs the `test` blocks of the project | Verilator, or Docker Desktop running |
| `volt explain CODE` | explains an error code or a topic | nothing |
| `volt doctor` | says which commands work here | nothing |

Two commands did not come up: `volt verify`, which proves contracts for
every input sequence, and `volt run`, which simulates a design without
tests. The Tutorial covers both.

## Next

[Part II, the Tutorial](../tutorial/counter.md), starts again from a blank
file and builds a counter line by line: modules, ports, registers, clocked
blocks, bit widths and tests. Each later chapter adds one idea on top.
