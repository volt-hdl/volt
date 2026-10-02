# Known limitations

<div class="chapter-goal">

What Volt 0.1 does not do yet, so you can decide early whether it fits
your design. Each limit says what happens when you hit it, how to work
around it where a way exists, and where the work is planned in the
[roadmap](https://github.com/volt-hdl/volt/blob/main/docs/roadmap.md).

</div>

## Stability

**No stability promise.** Volt 0.1 is an early release. Syntax,
diagnostic codes and the shape of the generated SystemVerilog may change
in any later release, without a deprecation period. Pin the Volt version
you use (the install script takes `VOLT_VERSION`) and read the changelog
before upgrading. Roadmap:
[A stability policy](https://github.com/volt-hdl/volt/blob/main/docs/roadmap.md#a-stability-policy-semver-for-hardware).

**Volt is used by its maintainer, nobody else yet.** There are no FPGA
board or tape-out results; the examples run in simulation and formal
verification.

## Language

**Some constructs pass the type checker but have no SystemVerilog mapping
yet.** They are rejected with `E0003` ("not supported yet"), and
`volt explain E0003` describes the error. Today this covers:

- integer types wider than 64 bits written as `u128` or `i100`. Use
  `bits<N>`, which has no width limit;
- arrays of structs, arrays of enums, generic structs, enums whose
  variants carry data, and generic enums;
- generic functions, contracts on a function, and `for` inside a function
  body;
- `match` arm guards (`0 if c =>`) and matching a whole struct value.
  Range, binding and struct patterns do not exist;
- `todo!()`. It is parsed, but `volt check` and `volt build` reject it.
  Roadmap:
  [todo!() placeholders](https://github.com/volt-hdl/volt/blob/main/docs/roadmap.md#todo-placeholders).

The README's [Limitations](https://github.com/volt-hdl/volt/blob/main/README.md#limitations)
section has the details and the measurements behind some of these
decisions.

**SystemVerilog is the sole output language.** There is no VHDL output.

**Combinational loops are not detected.** `volt check` and `volt build`
accept wires that feed each other (`a = b + x` and `b = a`), a `comb`
block that reads the signal it assigns, and a loop through an output port
or a submodule instance. `volt build` writes the looping SystemVerilog
without a warning. When `volt test` or `volt run` builds such a design,
Verilator stops with `%Warning-UNOPTFLAT ... Circular combinational
logic` followed by an example path around the loop, and the command
fails: that message is the loop in your design, not a Verilator problem.
Break it by removing one dependency on the path, or by registering one
of its signals in an `on` block. A
register cannot close a loop this way: assigning it with `=` outside an
`on` block is `E0019`. Roadmap:
[Combinational loop check](https://github.com/volt-hdl/volt/blob/main/docs/roadmap.md#combinational-loop-check).

## Clock and reset domains

**CDC bridges are `sync()`, `sync3()` and the built-in dual-clock
components** (`AsyncFifo`, `HandshakeSync`, `PulseSync`,
`AsyncDualPortRam`). A synchronizer you write yourself is not recognized
as a bridge: the crossing into it is an `E3001` error. Wrap a
hand-written or vendor synchronizer as an `extern` module with a clock
domain on each port. A multi-bit `sync()` is a warning (`W3003`), not an
error; use `AsyncFifo` or `HandshakeSync` for data.

**Reset checks cover reset release, not reset ordering.** Volt reports an
asynchronous reset shared by several clock domains or synchronized twice
(`E3003`), but it does not check the order in which resets are released,
and conditional resets are not modelled. `extern` modules carry no reset
contract. The domain keys `reset_cycles` and `reset_sequence` are
reserved for this: writing one is `E0003` ("not supported yet"). Keep
the reset length and order in your reset generator.

**No crossing report.** Volt knows every crossing, but it does not list
them in a report. With `volt build --emit=sdc` (or `xdc`), the
constraint file has a comment line for each crossing with its kind and
its source and destination clocks. Roadmap:
[CDC crossing report](https://github.com/volt-hdl/volt/blob/main/docs/roadmap.md#cdc-crossing-report).

**No power domains.** The `@Domain` annotation covers clock, reset and
trust level; power domains, isolation checks and UPF output do not exist.
Roadmap:
[Power domains and UPF](https://github.com/volt-hdl/volt/blob/main/docs/roadmap.md#power-domains-and-upf).

## Simulation and tests

**No built-in simulator.** `volt test` and `volt run` need Verilator. When
Verilator is not installed and Docker is running, Volt runs it in a
pinned container image by itself; `volt doctor` shows which way each
command will run.

**Test values are at most 64 bits wide.** The test language cannot write
to a port wider than 64 bits (`E2003`) or compare a wider value.
Workaround: test the design through a small wrapper module whose ports
are 64 bits or narrower. `volt run` does drive and print wide ports.
Roadmap:
[Port values wider than 64 bits in tests](https://github.com/volt-hdl/volt/blob/main/docs/roadmap.md#port-values-wider-than-64-bits-in-tests).

**The test language has no `if` or `match` expressions, and it cannot
call a design's functions** (`E8505`).

**Generated drivers are not simulated.** The C and Rust drivers generated
from an `@mmio` register map are not run against the RTL. `volt
check-regmap` checks that a driver matches the register map. Roadmap:
[Generated drivers in simulation](https://github.com/volt-hdl/volt/blob/main/docs/roadmap.md#generated-drivers-in-simulation).

## Formal verification

**`volt verify` needs SymbiYosys, Yosys and an SMT solver.** They run on
Linux; on Windows use WSL, or let Volt run them in Docker.

**Properties look back with `prev()`, nothing more.** There are no
sequences (`##`) and no liveness properties; the proof is bounded
(`--depth`).

**No modular proofs.** Each module is verified with the logic of its
submodules included; a verified submodule is not replaced by its
contracts. A submodule's `requires` and `assume` are checked as
obligations of the module that instantiates it. Large hierarchies take
longer to prove. Roadmap:
[Assume-guarantee verification](https://github.com/volt-hdl/volt/blob/main/docs/roadmap.md#assume-guarantee-verification-of-module-hierarchies).

**Contracts need a clock port.** Properties are checked on a clock edge,
so `volt verify` stops with `E5005` on a contract in a module without a
clock port. Give the module a clock port, or state the property in the
clocked module that instantiates it.

**Contradictory assumptions are not detected.** If the `requires` and
`assume` contracts of the module under proof cannot hold together, every
assertion passes without proving anything; the solver can also hold
reset for the whole run. Check that a `cover` of the behaviour you care
about is reached with `--mode cover`. Roadmap:
[Vacuity check](https://github.com/volt-hdl/volt/blob/main/docs/roadmap.md#vacuity-check-for-formal-proofs).

**A submodule's reset assumption stays an assumption in its parent's
proof.** Each module with a reset assumes that reset is asserted when
the proof starts, also when it is an instance in a parent's proof. When the
parent drives the submodule's reset from its own logic or from another
reset domain, this assumption constrains the parent: it can hide a
failure, or contradict the parent's reset so that every assertion passes
without proving anything. Designs whose submodules use the parent's reset
are not affected. Roadmap:
[Submodule reset assumptions](https://github.com/volt-hdl/volt/blob/main/docs/roadmap.md#submodule-reset-assumptions-in-parent-proofs).

**Information flow is checked by types, not proved formally.** `E3009`
catches secret data that reaches a public output at compile time; no
formal property is generated for it.

## Generated output

**Most of the generated SystemVerilog carries no Volt line numbers.**
Each file names its source file, and contract assertions and expanded
function calls carry `file:line` comments. Other statements do not, and
no command maps a SystemVerilog line back to Volt. Signal names follow
the Volt names, so searching for the name is the workaround. Roadmap:
[Source mapping](https://github.com/volt-hdl/volt/blob/main/docs/roadmap.md#source-mapping-in-generated-systemverilog).

**Register maps use AXI4-Lite with 32-bit registers**, and reset values
are 0. Register documentation is Markdown (`--emit=regmap-md`); there is
no SVD, IP-XACT or UVM output.

**Constraints, not projects.** `--emit=sdc,xdc` writes timing constraints.
There are no Vivado or Quartus project files and no pin constraints.
Roadmap:
[FPGA project files](https://github.com/volt-hdl/volt/blob/main/docs/roadmap.md#fpga-project-files).

**No diagrams and no project documentation.** Volt does not draw state
machines or generate documentation for a whole project. Roadmap:
[FSM state diagrams](https://github.com/volt-hdl/volt/blob/main/docs/roadmap.md#fsm-state-diagrams),
[Project documentation](https://github.com/volt-hdl/volt/blob/main/docs/roadmap.md#project-documentation-volt-doc).

## Editor

**Some language server features see one file.** Diagnostics, inlay hints
and quick fixes analyze every file a design loads with `use`. Hover,
completion, go to definition and the symbol outline look at the open
file: go to definition does not jump into another file. Roadmap:
[Multi-file language server features](https://github.com/volt-hdl/volt/blob/main/docs/roadmap.md#multi-file-language-server-features).

**The VS Code extension is a `.vsix` file from the release page.** It is
not on the Visual Studio Marketplace or Open VSX, it needs `volt` on
`PATH` (or the `volt.serverPath` setting), and it has no "Run test"
action. Roadmap:
[VS Code extension](https://github.com/volt-hdl/volt/blob/main/docs/roadmap.md#vs-code-extension-running-tests-and-installing-volt).

## Projects and installation

**No package dependencies.** `use` loads files inside the project.
`Volt.toml` has no dependency section and there is no lock file; copy
shared files into the project. Roadmap:
[Package dependencies](https://github.com/volt-hdl/volt/blob/main/docs/roadmap.md#package-dependencies).

**No package managers.** Install Volt with the install script or from
source, as [Install Volt](tour/install.md) shows; winget, Scoop and
Homebrew packages do not exist. Roadmap:
[Package managers](https://github.com/volt-hdl/volt/blob/main/docs/roadmap.md#package-managers).

**No browser playground.** Volt runs on your machine. Roadmap:
[Browser playground](https://github.com/volt-hdl/volt/blob/main/docs/roadmap.md#browser-playground).

**The design decisions are in Turkish.** The book, the language
specification and the diagnostics are in English (diagnostics also in
Turkish, `--lang=tr`); the ADRs are not translated. Roadmap:
[English summary of the design decisions](https://github.com/volt-hdl/volt/blob/main/docs/roadmap.md#english-summary-of-the-design-decisions).
