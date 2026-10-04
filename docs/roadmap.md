# Volt roadmap

## How to read this

This page lists the work we plan, not what Volt does today: what already
works is in [CHANGELOG.md](../CHANGELOG.md), and what does not work yet is
in the book's
[Known limitations](https://volt-hdl.github.io/volt/limitations.html) page.

- **No dates.** Volt has a single maintainer; an item is done when it is
  done.
- **The order can change.** Sections are ordered by intent (before v0.1,
  after v0.1, before 1.0, research), and items within a section roughly by
  priority. Feedback from users moves items.
- **Every item links to its design decision** (ADR) when one exists. The
  ADRs are written in Turkish; their index is
  [docs/adr/README.md](adr/README.md). A new feature that changes the
  language or the command line gets a new ADR before it is built.
- **Status** is checked against the code: *Partial* means part of the item
  works today (the item says which part), *Not started* means nothing of
  it is implemented. Finished items leave this page.

## Toward v0.1

v0.1 is the earliest public release. It makes no stability promise:
syntax, diagnostics and output may change in any later release.

### The v0.1.0 release

- **What:** tag `v0.1.0`, so that the release workflow publishes the
  prebuilt binaries, the VS Code extension (`.vsix`) and the checksums.
- **Why:** until a release exists, the one-line install commands have
  nothing to download and building from source is the sole way to install
  Volt.
- **Status:** Partial. The release workflow and the install scripts are in
  place; no release has been published.
- **ADR:** [ADR-0093](adr/ADR-0093-hazir-ikililer-surum-is-akisi.md),
  [ADR-0096](adr/ADR-0096-kurulum-betikleri.md)

## Next

Work after v0.1. The five items at the top come in this order; the rest
follow user feedback.

### FSM state diagrams

- **What:** generate a state diagram (Mermaid and Graphviz) from every
  `enum`-typed state register and its transitions.
- **Why:** a reviewer can check a state machine against its specification
  without reading the code, and the diagram never drifts from the design.
- **Status:** Not started. The compiler already finds state registers and
  their transitions for the automatic FSM contracts.
- **ADR:** [ADR-0074](adr/ADR-0074-enum-destegi.md),
  [ADR-0066](adr/ADR-0066-otomatik-fsm-sayac-kontratlari.md),
  [ADR-0086](adr/ADR-0086-otomatik-cover-erisilebilirligi.md)

### CDC crossing report

- **What:** a human-readable report of every clock domain crossing in a
  design: source and destination domain, signal, and synchronizer type.
- **Why:** a CDC sign-off review needs the list of crossings, and Volt
  already knows each of them because it rejects the unsynchronized ones.
- **Status:** Partial. With `volt build --emit=sdc` (or `xdc`) each
  crossing appears as a comment line in the constraint file (kind, name,
  source and destination clock); there is no standalone report.
- **ADR:** [ADR-0027](adr/ADR-0027-stdlib-mimarisi.md),
  [ADR-0054](adr/ADR-0054-sdc-uretimi.md),
  [ADR-0065](adr/ADR-0065-rdc-ve-hedefli-sdc.md)

### Source mapping in generated SystemVerilog

- **What:** `.volt` file and line for each generated statement (comments
  and `(* src *)` attributes), and a command that maps a SystemVerilog line
  reported by another tool back to the Volt source.
- **Why:** lint, synthesis and timing tools report SystemVerilog lines; the
  user needs the Volt line to fix the problem.
- **Status:** Partial. Each generated file names its source file, and
  contract assertions and expanded function calls carry `file:line`
  comments. Other statements carry no source location, and there is no
  reverse-lookup command.
- **ADR:** [ADR-0012](adr/ADR-0012-sv-cikti-ongorulebilirlik.md)

### Multi-file language server features

- **What:** hover, go to definition, completion and symbol search that
  follow `use` across the files of a project, plus workspace-wide symbols.
- **Why:** real designs span several files; jumping to a module defined in
  another file is the most common editor action.
- **Status:** Partial. Diagnostics, inlay hints and quick fixes already
  analyze the whole multi-file unit. Hover, completion, go to definition
  and document symbols look at the open file; go to definition never opens
  another file, and there is no workspace symbol search.
- **ADR:** [ADR-0042](adr/ADR-0042-coklu-dosya-derleme.md),
  [ADR-0091](adr/ADR-0091-lsp-inlay-quick-fix.md)

### Project documentation (volt doc)

- **What:** a `volt doc` command that writes browsable documentation for a
  project: modules, ports with their clock domains, contracts, register
  maps and doc comments.
- **Why:** the documentation of a hardware block is generated from the
  same source as the hardware, so it cannot fall out of date.
- **Status:** Not started. Register maps already produce Markdown
  (`--emit=regmap-md`); nothing else does.
- **ADR:** [ADR-0021](adr/ADR-0021-artifact-uretim-ve-cli-sozlesmesi.md)

### The rest of the book

- **What:** the Tutorial chapters after "A counter" and the Cookbook
  chapters.
- **Why:** the Tour shows what Volt does; the Tutorial teaches how to
  build a design with it.
- **Status:** Partial. The Tour and "A counter" are written; the other
  chapters list existing examples on their topic.

### Contracts in hover

- **What:** hovering over a module or an instance shows its `requires`,
  `ensures` and `invariant` contracts.
- **Why:** the contracts are the interface of a module; the user sees them
  where the module is used.
- **Status:** Not started. Hover shows the type, clock domain, struct
  layout and doc comment.
- **ADR:** [ADR-0011](adr/ADR-0011-kontrat-sistemi.md)

### Port values wider than 64 bits in tests

- **What:** let the test language write and compare values on ports wider
  than 64 bits.
- **Why:** keys, wide data buses and packed structs are tested directly,
  without a wrapper module.
- **Status:** Partial. `volt run` drives and prints wide ports; in the test
  language a value is 64 bits wide, and writing to a wider port is rejected
  (`E2003`).
- **ADR:** [ADR-0058](adr/ADR-0058-test-dili-genisletme.md),
  [ADR-0059](adr/ADR-0059-test-port-genislik-kontrolu.md),
  [ADR-0095](adr/ADR-0095-proje-kipi-test-dalga-formu-watch.md)

### VS Code extension: running tests and installing Volt

- **What:** a "Run test" action above each `test` block, an extension that
  downloads `volt` when it is missing, and publication on the Visual
  Studio Marketplace and Open VSX.
- **Why:** installing from the editor's extension view and running a test
  with one click removes the terminal from the common loop.
- **Status:** Partial. The release workflow builds a `.vsix` that starts
  the language server; it expects `volt` on `PATH`, has no test action and
  is not published to a marketplace.
- **ADR:** [ADR-0093](adr/ADR-0093-hazir-ikililer-surum-is-akisi.md)

### Coverage in the editor

- **What:** show in the editor which `cover` properties and FSM
  transitions a test run never reached.
- **Why:** a missed transition is a missing test; seeing it next to the
  code points at the test to write.
- **Status:** Partial. `volt test` and `volt run` count the hits of every
  `cover`, including the automatic FSM covers, and print a summary; the
  counts are not written to a file and the editor does not show them.
- **ADR:** [ADR-0064](adr/ADR-0064-simulasyonda-kontratlar.md),
  [ADR-0066](adr/ADR-0066-otomatik-fsm-sayac-kontratlari.md),
  [ADR-0086](adr/ADR-0086-otomatik-cover-erisilebilirligi.md)

### Waveform sessions grouped by clock domain

- **What:** the generated GTKWave session groups signals by clock domain
  and keeps the fields of a struct together.
- **Why:** in a multi-clock design, the signals of one domain are read
  against their own clock.
- **Status:** Partial. The session translates enum codes into variant
  names; it has no groups, and designs without enum signals get no
  session file.
- **ADR:** [ADR-0092](adr/ADR-0092-dalga-formunda-enum-adlari.md)

### Generated drivers in simulation

- **What:** run the C driver generated from an `@mmio` register map
  against the Verilator model of the same design.
- **Why:** it tests the hardware and the software of a peripheral
  together, before either reaches a board.
- **Status:** Not started. Generated C headers pass a C compiler in CI, and
  `volt check-regmap` compares a driver with the register map statically;
  no test runs a driver against the RTL.
- **ADR:** [ADR-0053](adr/ADR-0053-hw-sw-koprusu.md),
  [ADR-0063](adr/ADR-0063-regmap-tutarlilik-denetimi.md)

### Importing SystemVerilog modules

- **What:** generate an `extern` declaration from an existing
  SystemVerilog module.
- **Why:** using existing IP from Volt starts with its port list, which
  today is written by hand.
- **Status:** Not started. `extern` modules with clock domain annotations
  and their SystemVerilog sources work; the declaration is written by hand.
- **ADR:** [ADR-0047](adr/ADR-0047-extern-domain-anotasyonu.md),
  [ADR-0076](adr/ADR-0076-extern-kaynaklari.md)

### todo!() placeholders

- **What:** `todo!()` marks unfinished logic: the design checks and
  simulates, and simulation stops when it reaches the placeholder.
- **Why:** a design can be written and tested top-down, one block at a
  time.
- **Status:** Partial. `todo!()` is parsed; `volt check` and `volt build`
  reject it (`E0003`).
- **ADR:** [ADR-0019](adr/ADR-0019-todo-semantigi.md)

### English summary of the design decisions

- **What:** an English index of the ADRs: number, title and a one-line
  summary of each decision.
- **Why:** readers who do not read Turkish can find why a feature works
  the way it does.
- **Status:** Not started. The ADRs and their index are in Turkish.

### Package managers

- **What:** install Volt with winget, Scoop and Homebrew.
- **Why:** many users install and update tools through their package
  manager.
- **Status:** Not started. Volt installs with the install scripts or from
  source; ADR-0096 records why package managers were deferred.
- **ADR:** [ADR-0096](adr/ADR-0096-kurulum-betikleri.md)

### A Volt tools image

- **What:** a container image with the same Verilator, Yosys, SymbiYosys
  and solver versions as CI (including bitwuzla), for amd64 and arm64.
- **Why:** a local run with Docker matches CI exactly, also on arm64
  machines.
- **Status:** Not started. Volt's Docker bridge uses digest-pinned
  third-party images; the formal image is amd64 and has no bitwuzla.
- **ADR:** [ADR-0094](adr/ADR-0094-docker-koprusu.md)

### FPGA project files

- **What:** generate Vivado and Quartus project files (sources,
  constraints, top module) for a board.
- **Why:** going from a Volt design to a bitstream needs no hand-written
  project setup.
- **Status:** Not started. `volt build --emit=sdc,xdc` writes timing
  constraints; there are no project files and no pin constraints.
- **ADR:** [ADR-0054](adr/ADR-0054-sdc-uretimi.md)

### Vacuity check for formal proofs

- **What:** an optional `volt verify --vacuity` that checks, for each
  task, that the assumptions can hold together after reset is released,
  and a weekly CI run of it over the examples.
- **Why:** contradictory `requires` and `assume` contracts make every
  assertion pass without proving anything, and nothing reports it today.
- **Status:** Not started. The manual check is a `cover` of the intended
  behaviour run with `--mode cover`.
- **ADR:** [ADR-0097](adr/ADR-0097-alt-ornek-yukumlulukleri.md)

### Contracts over arrays

- **What:** contracts that hold for every element of an array, written
  inside a module-level `for` or in a for-each form.
- **Why:** an array of instances cannot state the per-element
  preconditions of its elements, so it cannot be verified on its own:
  `TernaryArray` in `examples/hybrid_accel` fails `volt verify` alone and
  passes inside `HybridTop`.
- **Status:** Not started. A contract inside `for` is rejected (`E0001`).
  Needs a new ADR.
- **ADR:** [ADR-0056](adr/ADR-0056-duzenli-yapilar.md),
  [ADR-0097](adr/ADR-0097-alt-ornek-yukumlulukleri.md)

### Array literals in contracts

- **What:** contracts that compare with an array literal
  (`r == [prev(a), prev(b)]`) in every mode, or a Volt diagnostic for each
  form a tool cannot take.
- **Why:** `volt test` compiles the comparison of an array register with
  a literal, the other forms do not: `volt verify` stops with a Yosys syntax error, and
  `volt test` and separate `.sva` files fail in Verilator for the other
  forms, once with an internal fault. The user meets a tool error, not a
  Volt diagnostic.
- **Status:** Not started. Measured and listed in ADR-0098 (appendix 2);
  the workaround is an element-by-element comparison.
- **ADR:** [ADR-0098](adr/ADR-0098-sessiz-kabul-ikinci-tur.md)

### Submodule reset assumptions in parent proofs

- **What:** check a submodule's first-cycle reset assumption in its
  parent's proof instead of keeping it as an assumption, as is done for
  `requires` and `assume`.
- **Why:** when the parent drives the submodule's reset from its own logic
  or from another reset domain, the kept assumption constrains the parent:
  it can hide a failure, or contradict the parent's reset and make the
  proof pass without proving anything.
- **Status:** Not started. Each module with a reset assumes it asserted in
  the first cycle, in its own proof and in every parent's proof.
- **ADR:** [ADR-0097](adr/ADR-0097-alt-ornek-yukumlulukleri.md)

### Combinational loop check

- **What:** a compile-time error for a combinational loop: wires that
  feed each other, a `comb` block that reads what it assigns, and loops
  through output ports and submodule instances, with the loop's signals
  named in the message.
- **Why:** `volt check` and `volt build` accept such a loop today; it
  surfaces only when Verilator stops with `UNOPTFLAT` in `volt test`, or
  in synthesis.
- **Status:** Partial. A register assigned with `=` outside an `on`
  block, the one loop a single statement can make, is `E0019`.
- **ADR:** [ADR-0098](adr/ADR-0098-sessiz-kabul-ikinci-tur.md)

### Machine-readable test results

- **What:** `volt test --format=json` and `volt run --format=json`: the
  diagnostics envelope that `volt check` writes, plus one record per test
  (file, name, result, failed assertions with their source line, the
  waveform path) and, for `volt run`, the printed cycle table.
- **Why:** CI systems show test results from a report file; today a CI job
  sees only the exit code and a text log.
- **Status:** Not started. `volt check`, `volt build` and `volt verify`
  have `--format=json`; the command-line contract shows
  `volt test --format=json` in its CI example, but the option does not
  exist and the test record has no schema yet.
- **ADR:** [ADR-0021](adr/ADR-0021-artifact-uretim-ve-cli-sozlesmesi.md),
  [ADR-0033](adr/ADR-0033-test-bloklari-ve-simulasyon.md)

### A machine-wide limit on Docker containers

- **What:** a limit on how many Verilator or sby containers the Docker
  backend runs at once on one machine, shared by every `volt` process
  (for example a lock file per running container under the user's cache
  directory); a command waits for a free slot and says so.
- **Why:** each `volt test`, `volt run` or `volt verify` starts its own
  container, and a user who runs several commands in parallel (several
  terminals, an editor task and a watch loop) can exhaust the memory of
  Docker Desktop; the containers then die with exit code 137.
- **Status:** Not started. `volt verify -j N` already runs all jobs in
  one container, and an out-of-memory exit (137) is reported with a hint,
  but separate commands do not know about each other.
- **ADR:** [ADR-0094](adr/ADR-0094-docker-koprusu.md)

## Toward 1.0

1.0 will make a stability promise. These items are its preconditions: a
policy that defines the promise, reproducible release builds, and a way to
share code between projects.

### A stability policy (SemVer for hardware)

- **What:** a written policy for what may change between releases, and
  `@version` / `@abi_version` checks that report an incompatible interface
  change.
- **Why:** users and library authors know which upgrades are safe.
- **Status:** Not started. The attributes are parsed and ignored
  (`W0021`); their diagnostics are reserved.
- **ADR:** [ADR-0022](adr/ADR-0022-semver-donanim-kurallari.md)

### Lock file and release builds

- **What:** fast incremental builds for daily work, `volt build --release`
  for a from-scratch build that refuses unfinished code (`todo!()`), and
  `volt.lock` recording the inputs of a release build.
- **Why:** a released design can be rebuilt later, bit for bit, as
  evidence for certification and IP delivery.
- **Status:** Not started. Builds are already deterministic (the same
  source gives byte-identical output); there is a single build mode and no
  lock file.
- **ADR:** [ADR-0015](adr/ADR-0015-determinizm-garantisi.md),
  [ADR-0019](adr/ADR-0019-todo-semantigi.md)

### Package dependencies

- **What:** depend on other Volt packages from `Volt.toml`, with version
  resolution.
- **Why:** reusable IP is shared as packages instead of copied files.
- **Status:** Not started. `use` loads files inside the project;
  `Volt.toml` has no dependency section.
- **ADR:** [ADR-0017](adr/ADR-0017-modul-sistemi-ve-paket-semantigi.md),
  [ADR-0042](adr/ADR-0042-coklu-dosya-derleme.md)

## Exploring

Long-term and research work. These items need a design decision before
they are built, and some may not be built.

### Information flow and formal verification

- **What:** prove the absence of leaks between trust levels formally, in
  addition to the type check.
- **Why:** a formal proof is evidence that a security review can check
  independently of Volt's type checker.
- **Status:** Partial. The compile-time information-flow check (`E3009`,
  `declassify`) works. ADR-0052 decided not to generate formal properties
  for it, because non-interference is a property of two executions; going
  further needs a new decision.
- **ADR:** [ADR-0020](adr/ADR-0020-guvenlik-akis-denetimi.md),
  [ADR-0052](adr/ADR-0052-guven-seviyeleri.md)

### Assume-guarantee verification of module hierarchies

- **What:** verify each module against its own contracts, then use its
  guarantees in place of its logic when verifying the parent.
- **Why:** formal verification scales to large designs one module at a
  time.
- **Status:** Not started. `volt verify` checks each module with its
  submodules' logic included.
- **ADR:** [ADR-0011](adr/ADR-0011-kontrat-sistemi.md),
  [ADR-0055](adr/ADR-0055-paralel-formal-dogrulama.md)

### Power domains and UPF

- **What:** power domains in the `@Domain` annotation, a check for
  crossings without isolation, and UPF output.
- **Why:** low-power designs get the same compile-time safety for power
  crossings as for clock crossings.
- **Status:** Not started. The diagnostic for an unisolated power crossing
  is reserved.
- **ADR:** [ADR-0002](adr/ADR-0002-domain-semantigi.md),
  [ADR-0021](adr/ADR-0021-artifact-uretim-ve-cli-sozlesmesi.md)

### Browser playground

- **What:** `volt check` and `volt build` compiled to WebAssembly, and a
  "Try it" button on the book's examples.
- **Why:** people can try Volt before installing anything.
- **Status:** Not started.

### Timeline types (L2 timing)

- **What:** types that describe when a value is valid over several cycles,
  with resource-conflict analysis.
- **Why:** multi-cycle protocols are checked by the type system, beyond
  the fixed latencies of `Delayed<T, N>`.
- **Status:** Not started. L1 timing (`Delayed<T, N>`, `@strict_timing`)
  works.
- **ADR:** [ADR-0007](adr/ADR-0007-zamanlama-seviyeleri.md),
  [ADR-0037](adr/ADR-0037-l1-zamanlama.md)

### Ternary as an opt-in module

- **What:** move the `Trit` type behind `import volt::ternary`, with a
  warning when ternary logic is mapped to a binary FPGA.
- **Why:** designs that do not use ternary logic do not see it.
- **Status:** Not started. `Trit` is a built-in type.
- **ADR:** [ADR-0003](adr/ADR-0003-trit-tipi.md)

### Contract hooks

- **What:** user-written policy (for example the choice rule of an
  arbiter) plugged into a built-in component, bound by an `ensures`
  contract that formal verification checks.
- **Why:** built-in components become configurable without losing their
  guarantees.
- **Status:** Not started. The keyword is reserved.
- **ADR:** [ADR-0016](adr/ADR-0016-hook-kontrat-uyumu.md)

### More generated artifacts

- **What:** cocotb testbench skeletons and DFT (scan) output.
- **Why:** Volt designs fit into existing verification and test flows.
- **Status:** Not started.
- **ADR:** [ADR-0021](adr/ADR-0021-artifact-uretim-ve-cli-sozlesmesi.md)

### A Turkish edition of the book

- **What:** the book in Turkish, next to the English one.
- **Why:** the compiler's diagnostics and explanations are already
  bilingual; the book is not.
- **Status:** Not started.
