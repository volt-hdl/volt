# Volt

An HDL where clock domain crossing bugs won't compile.

![CI](https://github.com/volt-hdl/volt/actions/workflows/ci.yml/badge.svg)
![tests](https://img.shields.io/badge/tests-719-brightgreen)
![coverage](https://img.shields.io/badge/coverage-84%25-green)
![license](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue)

## The problem

SystemVerilog compiles this without a single warning:

```systemverilog
always_ff @(posedge slow_clk)
    slow_reg <= fast_signal;   // compiles fine
```

If `fast_signal` comes from another clock domain, this bug usually stays
invisible through simulation and surfaces in silicon as intermittent
metastability failures.

## The same bug in Volt

`tests/ui/fail/01_cdc_violation.volt` assigns a signal from domain `@Fast`
to a port in domain `@Slow` without a synchronizer. `volt build` rejects it
(output below is copied verbatim; two `W1001` unused-port warnings omitted):

```text
error[E3001]: direct assignment between clock domains
   ┌─ tests/ui/fail/01_cdc_violation.volt:21:5
   │
 5 │ domain Fast {
   │        ---- source @Fast defined here
   ·
10 │ domain Slow {
   │        ---- destination @Slow defined here
   ·
21 │     slow_data = fast_data
   │     ^^^^^^^^^   --------- @Fast
   │     │
   │     @Slow
   │
   = reason: the destination register may sample the source signal during an unstable window (metastability)
   = note: for multi-bit data, AsyncFifo may be safer
   = help: synchronize into the target domain with sync(): dest = sync(src, <clock of Slow>)
   = for more: volt explain E3001


     Error: build failed due to 1 error(s), 2 warning(s)
```

The build exits with code 1 and no SystemVerilog is produced. The accepted
fix is an explicit bridge — `slow_data = sync(fast_data, slow_clk)` — which
compiles to a source-capture register plus a two-flop synchronizer clocked
in the target domain.

## Single-clock designs stay simple

Single-clock designs never write `domain` — a module with one clock is
inferred to live entirely in that clock's domain, with zero annotations.
This is `tests/fixtures/counter.volt` (doc comments are in Turkish; the
fixture is the byte-exact golden input for the emitter tests):

```text
/// 8-bit yukarı sayaç
/// enable yüksekken her saat kenarında artar
module Counter {
    in  clk    : clock
    in  enable : bool
    out count  : u8

    reg count_r : u8 = 0

    on clk {
        if enable {
            count_r <= count_r + 1
        }
    }

    count = count_r
}
```

## Generated SystemVerilog

```console
$ volt build tests/fixtures/counter.volt
   Compiling tests/fixtures/counter.volt
    Finished 0.00s
     Output build\rtl\counter.sv (34 lines)
```

First 20 lines of `build/rtl/counter.sv`, copied verbatim (the header
comments are emitted in Turkish today; doc comments carry over from the
source file):

```systemverilog
// Bu dosya Volt tarafından otomatik üretilmiştir.
// Kaynak: counter.volt
// Volt sürümü: 0.1.0
//
// DÜZENLEMEYİN — değişiklikler kaynak dosyada yapılmalıdır.

`default_nettype none

// 8-bit yukarı sayaç
// enable yüksekken her saat kenarında artar
module Counter (
    input  logic       clk,
    input  logic       rst,
    input  logic       enable,
    output logic [7:0] count
);

    logic [7:0] count_r;

    always_ff @(posedge clk) begin
```

The reset port and reset block are generated automatically; bare `always`,
`reg`, `initial` and `#` delays are never emitted. CI lints this output
with `verilator --lint-only -Wall` and requires zero warnings.

## Status

Pre-1.0, under active development. Syntax may change without notice.

| Works today | Not yet |
|---|---|
| Full-grammar parser with error recovery, fuzzed | Formal verification (F4) |
| Name resolution and const evaluation | Language server / LSP (F5) |
| Type system with overflow widening on arithmetic | Reset-domain (RDC) checks — error codes reserved, not enforced |
| Domain inference and CDC checking (E3001, ambiguous-domain, multi-domain writes) | Standard library is thin |
| `sync()` / `sync3()` generation — source-capture register plus two/three-stage synchronizer in the target domain | Simulation on Windows needs WSL or Docker |
| SystemVerilog output, single- and multi-clock modules, Verilator-lint-clean | |
| CLI: `volt build` / `volt check`, contracted exit codes, `--lang en\|tr`, JSON output | |

## Installation

```console
$ git clone https://github.com/volt-hdl/volt
$ cd volt
$ cargo build --release
$ ./target/release/volt build tests/fixtures/counter.volt
```

Requires stable Rust; no other dependencies.

## Why not an existing HDL?

- **Clash** has encoded clock domains in its type system for 16 years and
  does it rigorously — if you are at home in Haskell, use it.
- **Spade** is a solid standalone HDL with excellent tooling; CDC safety
  is on its roadmap rather than in its type system today.
- **Veryl** makes SystemVerilog substantially nicer to write, by design
  without adding new semantics.
- **Arch** also checks CDC at compile time; CIRCT lowering and formal
  verification are absent there, and on Volt's roadmap (F3/F4) —
  admittedly not built yet here either.

## Documentation

- [docs/spec/](docs/spec/) — the binding language specification
  (grammar, type inference, domain inference, SV mapping)
- [docs/adr/](docs/adr/) — architecture decision records
- [docs/README.md](docs/README.md) — full document index

## License

Licensed under either of the Apache License, Version 2.0 or the MIT
license, at your option.
