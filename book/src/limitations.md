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

## Known bugs

These are bugs, not design limits: each one has an issue and is planned
for a fix. Until then, the workaround after each one avoids it. The
division bug ([#150](https://github.com/volt-hdl/volt/issues/150)) and
the cut operand of `+`, `-` and `*`
([#266](https://github.com/volt-hdl/volt/issues/266)) are under
[Language](#language) below.

**A struct `let` whose flattened field name equals another name.** A
struct `let` in a block becomes one signal per field in the generated
SystemVerilog: `let p : S` with a field `x` becomes `p_x`. If a port or
signal is also called `p_x`, a read of it in that block reads the field
of the local, without a warning. Do not name a port or signal
`<let>_<field>`; rename the `let`. Tracked in
[issue #154](https://github.com/volt-hdl/volt/issues/154).

**A module-level `let` with the name of a bundle port or of `regs`** is
accepted without error `E1003`, and the name then means two different
things: the port in one place, the `let` in another. Give the `let` a
name of its own. Tracked in
[issue #155](https://github.com/volt-hdl/volt/issues/155).

**An import alias reaches the generated SystemVerilog.** With
`use other::X as J`, a constant used as a value (`y = J`) is written
into the SystemVerilog as `assign y = J;`, a name that does not exist
there, without a diagnostic; an aliased type, function, domain or module
is rejected. Import such items without `as`. Tracked in
[issue #161](https://github.com/volt-hdl/volt/issues/161).

**A test block can read a private constant of another file**, without
error `E1001`. Make the constant `pub` and import it with `use` in the
test file, as the visibility rules require. Tracked in
[issue #162](https://github.com/volt-hdl/volt/issues/162).

**Constant evaluation stops at depth 64 in `for` bounds and bundle
arrays.** A module-level `for` bound or a bundle array length that is a
constant expression deeper than 64 levels, or the end of a chain of 64
constants, is rejected with `E2021`; a cycle there is `E2021` instead of
`E2020`; and a counter whose limit is such a chain gets no inferred
contract. Keep constant chains and expressions shallower than 64 levels.
Tracked in [issue #159](https://github.com/volt-hdl/volt/issues/159).

**A generic module that is never instantiated and uses its parameter in
a type** (`in x : uint<N>`) is rejected with `E2021` ("this is a generic
parameter (runtime value)"), also in a file loaded with `use`. Instantiate
the module once, for example `Fir<8> { ... }`. Tracked in
[issue #172](https://github.com/volt-hdl/volt/issues/172).

**A `match` is computed at a narrower width than the type checker
uses.** With `u8` operands, `let m = match sel { 0 => a + b, _ => a - b }`
is an 8-bit signal in the generated SystemVerilog, so 200 + 100 gives 44;
`match a + b` with an arm `44 =>` also takes the value 300; and a
contract with `prev(a + b)` or a signed `prev()` compares at the wrong
width. Write the choice as `if`/`else`, or put the subject in a `let`
with an explicit width (`let s : u9 = a + b`, then `match s`). Tracked in
[issue #215](https://github.com/volt-hdl/volt/issues/215).

**In a statement `match`, an arm after the wildcard wins.** In a `comb`
block, `match s { 0 => { … } _ => { y = 7 } 1 => { y = 9 } }` gives 9 for
`s = 1`, while the same `match` written as an expression gives 7. Write
the wildcard arm last. Tracked in
[issue #216](https://github.com/volt-hdl/volt/issues/216).

**A `const` that does not fit its type is accepted.** With
`const A : u8 = 255` and `const B : u8 = A + 1`, compile time sees 256 (a
`bits<B>` port is 256 bits wide) and the hardware sees 0. Make sure every
constant computed from other constants fits its declared type. Tracked
in [issue #214](https://github.com/volt-hdl/volt/issues/214).

**The clock outputs of an `extern` module have no clock domain.** With a
PLL or a clock buffer declared as an `extern module` with
`out c0 : clock`, nothing clocked by `c0` is checked: a crossing between
the input clock and an output clock gets no `E3001`. Synchronize every
value between these clocks yourself with `sync()` or an `AsyncFifo`.
Tracked in [issue #208](https://github.com/volt-hdl/volt/issues/208).

**A register's reset value can be a signal, and it is not checked.**
`reg r : u8 = a` loads `a` at reset without the clock-domain check
(`E3001`) and the trust check (`E3009`). Use a constant reset value and
load the signal in the `on` block. Tracked in
[issue #209](https://github.com/volt-hdl/volt/issues/209).

**A clock port declared through a type alias is not a clock everywhere.**
With `type Clk = clock`, an asynchronous reset shared by two clock
domains gets no `E3003`, the SDC file has no `create_clock` for the
clock, and a test block can drive it. Write `clock` for clock ports.
Tracked in [issue #234](https://github.com/volt-hdl/volt/issues/234).

**`E5010` is missed in a `comb` block.** In an `@strict_timing` module,
choosing between values of different delays inside a `comb` block
(`comb { if x[0] { w = a } else { w = b } }`) compiles, while the same
choice written as `let w = if x[0] { a } else { b }` gets `E5010`. Write
such a choice as a `let`. Tracked in
[issue #217](https://github.com/volt-hdl/volt/issues/217).

**A timing constraint can apply to a second register.** `@multicycle`,
`@false_path` and `@timing(max_delay)` on a register `slow` are written
to the SDC file as `get_cells {slow_reg*}`, which also matches a
register named `slow_reg`. Do not name a register `<name>_reg…` next to
a constrained register `<name>`. Tracked in
[issue #219](https://github.com/volt-hdl/volt/issues/219).

**A secret passes through an unclassified input port of a submodule.**
With `module Mid { in x : u8  out o : u8 @Pub  o = x }`, binding a
`@Sec` value to `x` carries it to a public output without `E3009`.
Annotate the input ports of submodules with their trust domain. Tracked
in [issue #210](https://github.com/volt-hdl/volt/issues/210).

**The trust annotation of a `wire` or a `let` is not read.** In a module
without a clock, `wire w : u8 @Pub` driven by a `@Sec` value gets no
`E3009`; in a module whose clock is `@Pub`, `wire w : u8 @Sec` gets a
false `E3009`. Annotate the ports instead. Tracked in
[issue #211](https://github.com/volt-hdl/volt/issues/211).

**An `@mmio` module next to a declared AXI bundle name does not
compile.** When the file declares `AxiWriteAddr` or another of the five
AXI bundle names itself, the generated module reports `E1001` for names
such as `aw_valid` that you did not write. Do not declare these names
next to an `@mmio` module. Tracked in
[issue #244](https://github.com/volt-hdl/volt/issues/244).

**A `bits<K>` value cannot cross a pipeline stage.** It is rejected with
`E2003` on a literal that the compiler generates. Carry the value as
`uK` across the stage and cast it back to `bits<K>` where it is used.
Tracked in [issue #245](https://github.com/volt-hdl/volt/issues/245).

**`frequency = 100mhz` gets errors that do not say what is wrong.** The
domain key gets `W0020` "unknown domain key: 'mhz'" followed by `E0001`.
Write `100.mhz`. Tracked in
[issue #223](https://github.com/volt-hdl/volt/issues/223).

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

**Integer literals stop at 2^128 - 1.** A literal holds a value up to
2^128 - 1 in any base. A larger one, such as
`0x1_0000_0000_0000_0000_0000_0000_0000_0000`, is `E0005` ("value does
not fit in 128 bits"), also when the target is `bits<256>`. Nothing is cut
silently: a literal up to 2^128 - 1 cast to `bits<256>` keeps its value
and the upper bits are zero. To build a wider constant, write it in
128-bit parts and join them with a struct cast. `hi`, declared before
`lo`, holds the upper 128 bits:

```volt
struct Key256 {
    hi : bits<128>,
    lo : bits<128>,
}

module Key {
    out key : bits<256>
    let k : Key256 = Key256 {
        hi: 0x0123_4567_89AB_CDEF_0011_2233_4455_6677 as bits<128>,
        lo: 0x8899_AABB_CCDD_EEFF_FEDC_BA98_7654_3210 as bits<128>,
    }
    key = k as bits<256>
}
```

`key[255:192]` is `0x0123_4567_89AB_CDEF` and `key[63:0]` is
`0xFEDC_BA98_7654_3210`.

**The width of a division with a wider arithmetic operand will change.**
`/` and `%` do not widen: `a * b / c` (all `u8`) divides at 8 bits, and
the product, up to 16 bits wide, is cut to 8 bits before the division.
The generated SystemVerilog does not always do the same, and the rule
changes in v0.2.0. Until then Volt warns (`W2015`) wherever an operand is
cut. Write the width you mean on both operands, as in
`(a * b) as u16 / (c as u16)`: the warning goes away and the value does
not depend on the rule. Tracked in
[issue #150](https://github.com/volt-hdl/volt/issues/150).

**The width of a sum, difference or product with a wider arithmetic
operand will change.** `+`, `-` and `*` are done at the width both
operands share, plus their own carry: `a * b - a` (all `i32`) is computed
at 33 bits, and the product, up to 64 bits wide, is cut to 33 bits before
the subtraction. With `a = b = 2^20` the result is `-2^20` instead of
`2^40 - 2^20`. The rule changes in v0.2.0. Until then Volt warns
(`W2016`) wherever the cut can change the value you read; a result
assigned to a declared width keeps that many bits anyway and gets no
warning. Put the intermediate result in a `let` with an explicit width,
as in `let d : i64 = a * b - a`: the warning goes away and the value does
not depend on the rule. A product of two `i64` or `u64` values has no
wider integer type yet, so before v0.2.0 there is no way around the cut.
Tracked in [issue #266](https://github.com/volt-hdl/volt/issues/266).

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

**The handshake check does not look inside an `extern` module with a
clock.** On a `Handshake<T>` output, `valid` must not be computed from
the same port's `ready` without a register in between: if each side
waits for the other, no transfer ever finishes. Volt reports this as
`E4007` when the path runs through assignments, `let`s, `comb` blocks,
`if` and `match` conditions, function calls, submodules, `extern`
modules without a clock and the built-in modules whose outputs follow
their inputs in the same cycle (`EdgeDetect`, `RoundRobinArbiter`,
`PriorityArbiter`). An `extern` module with a clock is not looked into:
when `ready` goes into such a module and `valid` comes out of it through
a path inside it, the design compiles without an error. Volt cannot tell
that path from a registered one, such as the `empty` flag of a FIFO,
which is the common and safe case. Register the value that comes back
from the module, or compute `valid` in the module that owns the port.
Tracked in [issue #196](https://github.com/volt-hdl/volt/issues/196).

## Clock and reset domains

**CDC bridges are `sync()`, `sync3()` and the built-in dual-clock
components** (`AsyncFifo`, `HandshakeSync`, `PulseSync`,
`AsyncDualPortRam`). A synchronizer you write yourself is not recognized
as a bridge: the crossing into it is an `E3001` error. Wrap a
hand-written or vendor synchronizer as an `extern` module with a clock
domain on each port. A multi-bit `sync()` is a warning (`W3003`), not an
error; use `AsyncFifo` or `HandshakeSync` for data.

**A submodule's dependencies are tracked per port, not per bit.** For
each output port of a module, Volt records which input ports it depends
on without a register in between. An output whose bit 0 follows `a` and
whose bit 1 follows `b` counts as depending on both. Through a module
without a clock, `a` from one clock domain and `b` from another then
make that output an `E3001` error, although each bit stays in its own
domain; through a submodule on a handshake path, the same output can
give an `E4007` that the bits do not deserve. Split such an output into
two ports. An `extern` module without a clock is assumed to connect each
output to every input, because its body is not visible: give its ports
clock domain annotations when its paths belong to different domains.

**A handshake path through an `extern` module without a clock is
assumed.** Volt cannot see inside such a module, so it assumes that each
output depends on every input. When `ready` goes into it and `valid`
comes out of it, `E4007` is reported even if the real module has no path
from that input to that output; the error's note says that the path
rests on this assumption. `@no_protocol_check` does not change this: it
turns off the automatic protocol contracts of the port, not the check.
Decide `valid` from a register, as `E4007`'s help shows (`reg valid_r
...; on clk { ... }; tx.valid = valid_r`): that cuts the path whatever
the module does. A way to declare which outputs of an `extern` module
depend on which inputs is planned for v0.2.0.

**Reset checks cover reset release, not reset ordering.** Volt reports an
asynchronous reset shared by several clock domains or synchronized twice
(`E3003`), but it does not check the order in which resets are released,
and conditional resets are not modelled. `extern` modules carry no reset
contract. The domain keys `reset_cycles` and `reset_sequence` are
reserved for this: writing one is `E0003` ("not supported yet"). Keep
the reset length and order in your reset generator.

**An `extern` gets the raw reset, not the synchronized one.** For a raw
reset port (`in rst : reset(...)`) Volt puts a release synchronizer in
front of the module's own registers. An `extern` instance connected to
the same port gets the port itself, so its flip-flops leave reset on a
different cycle than the registers beside it, and their release is not
aligned to the clock unless the SystemVerilog module synchronizes it.
Volt warns about this (`W3011`) and changes nothing in the output; there
is no way yet to hand the synchronized copy to an `extern`. Tracked in
[issue #93](https://github.com/volt-hdl/volt/issues/93).

**No crossing report.** Volt knows every crossing, but it does not list
them in a report. With `volt build --emit=sdc` (or `xdc`), the
constraint file has a comment line for each crossing with its kind and
its source and destination clocks. Roadmap:
[CDC crossing report](https://github.com/volt-hdl/volt/blob/main/docs/roadmap.md#cdc-crossing-report).

**No power domains.** The `@Domain` annotation covers clock, reset and
trust level; power domains, isolation checks and UPF output do not exist.
The power keys of a `domain` (`voltage`, `always_on`, `retention`,
`isolation`) are reserved words: `volt check` rejects them with `E0003`,
so a design cannot declare a power domain at all. The power-domain codes
`E3006`, `E3007` and `E3008` are defined and `volt explain` describes the
planned checks, but the compiler never reports them: nothing checks
isolation, power-up order or retention. Roadmap:
[Power domains and UPF](https://github.com/volt-hdl/volt/blob/main/docs/roadmap.md#power-domains-and-upf).

## Simulation and tests

**No built-in simulator.** `volt test` and `volt run` need Verilator. When
Verilator is not installed and Docker is running, Volt runs it in a
pinned container image by itself; `volt doctor` shows which way each
command will run.

**Verilator runs with its DFG optimizer turned off.** `volt test` and
`volt run` pass `-fno-dfg` to every Verilator build, locally and in
Docker. With that optimizer on, Verilator 5.052 prints wrong values for
some correct SystemVerilog
([#148](https://github.com/volt-hdl/volt/issues/148)). On the example
designs the option changed the time of `volt test` by less than the
difference between two runs. If you run Verilator on the generated
SystemVerilog yourself, add `-fno-dfg` too.

**Every clock runs at the same rate in simulation.** `volt test` and
`volt run` give each clock port of the design one rising edge per cycle,
all at the same moment. A design with several clocks cannot be simulated
with clocks of different speeds or with a phase shift between them, so
a test checks the order and the latency of a crossing, not its timing.
The crossing itself is checked by the compiler (`E3001`). An example is
in [Two clocks and CDC](tutorial/two-clocks.md#test-it).

**Test values are at most 64 bits wide.** The test language cannot write
to a port wider than 64 bits (`E2003`) or compare a wider value.
Workaround: test the design through a small wrapper module whose ports
are 64 bits or narrower. `volt run` does drive and print wide ports.
Roadmap:
[Port values wider than 64 bits in tests](https://github.com/volt-hdl/volt/blob/main/docs/roadmap.md#port-values-wider-than-64-bits-in-tests).

**The test language has no `if` or `match` expressions, and it cannot
call a design's functions** (`E8505`).

**No machine-readable test results.** `volt test` and `volt run` write
their report for people; they have no `--format=json` option yet, unlike
`volt check`, `volt build` and `volt verify`. In CI, use the exit code
(`volt test` exits with 5 when a test fails) and keep the text report as
a log. Roadmap:
[Machine-readable test results](https://github.com/volt-hdl/volt/blob/main/docs/roadmap.md#machine-readable-test-results).

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

**Array literals in contracts are not supported yet.** A contract that
compares with an array literal (`r == [prev(a), prev(b)]`) is passed to
the tools as an assignment pattern, and most tools reject it: `volt
verify` stops with a Yosys syntax error (`unexpected OP_CAST`) for every
form; `volt test` and a separate `.sva` file fail in Verilator when the
literal is compared with another literal or with an array port, and a
separate `.sva` file comparing an array register with a literal that
uses `prev()` makes Verilator report an internal fault. `volt test`
does compile the comparison of an array register with a literal. Volt
does not report the failing forms itself yet. Workaround: compare element by element
(`r[0] == prev(a) && r[1] == prev(b)`). Roadmap:
[Array literals in contracts](https://github.com/volt-hdl/volt/blob/main/docs/roadmap.md#array-literals-in-contracts).

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

**No parameterized SystemVerilog modules.** A generic module becomes
SystemVerilog when it is instantiated: `Fir<8> { ... }` generates the
module `Fir_8`, with the parameter replaced by its value. A generic module
that nothing in the file instantiates generates no SystemVerilog file, and
Volt warns about it (`W1006`). Writing it as a parameterized module
(`module Fir #(parameter N)`) is a proposal for v0.2.0, tracked in
[issue #171](https://github.com/volt-hdl/volt/issues/171).

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
