# VGA controller — the first two-clock example

640x480 @ 60 Hz sync generator, an 80x60 1-bit frame buffer written
from a system clock and read from the pixel clock, and a top level
that draws a checkerboard. Every earlier example was single-clock;
this one exists to see what Volt's clock-domain story looks like on a
design that genuinely needs two clocks.

```
VgaTop                                (vga_top.volt)
 ├── sys_clk @SysDomain   checkerboard writer FSM, `invert`, `fill_done`
 │      │  AsyncFifo<u14,16>  (x, y, bit) commands   ─┐
 │      │  sync()             invert, done_r         │ sys → pix
 │      │  sync()             frame_tick             │ pix → sys
 ├── FrameBuffer          (frame_buffer.volt)        ◄┘
 │      DualPortRam<bool, 8192> in PixDomain, port A = FIFO pop, port B = scan-out
 ├── VgaTiming            (vga_timing.volt)   pix_clk only, both domains declared here
 └── pix_clk @PixDomain   read address, 1-cycle alignment, grid, fill bar, RGB
```

| File | Lines | Content |
|---|---|---|
| `vga_timing.volt` | 85 | `SysDomain`/`PixDomain`, 800x525 counters, active-low hsync/vsync, `visible`, 5 invariants + 3 covers |
| `frame_buffer.volt` | 99 | sys-side write port → `AsyncFifo<u14,16>` → pix-side `DualPortRam<bool,8192>`; 2 covers |
| `vga_top.volt` | 130 | writer FSM, three `sync()` crossings, aligned RGB; 3 invariants + 1 cover |
| `vga_top_test.volt` | 136 | 7 simulation tests, 1.22 M cycles in total |

## Building, linting, testing, verifying

```
volt build examples/vga/vga_top.volt                   # 3 files -> VgaTiming.sv FrameBuffer.sv VgaTop.sv
docker run --rm -v "$PWD/build/rtl:/work" -w /work verilator/verilator:latest \
    --lint-only -Wall --top-module VgaTop VgaTop.sv FrameBuffer.sv VgaTiming.sv   # clean
volt test examples/vga/vga_top_test.volt               # 7 tests (Verilator; Docker recipe in ../README.md)

VOLT_SBY=build/sby-docker.cmd volt verify --mode bmc   --depth 12 examples/vga/vga_timing.volt   # 8 props, 1.5 s
VOLT_SBY=build/sby-docker.cmd volt verify --mode prove --depth 3 --engine boolector examples/vga/vga_timing.volt
VOLT_SBY=build/sby-docker.cmd volt verify --mode cover --depth 700 --engine boolector examples/vga/vga_timing.volt
VOLT_SBY=build/sby-docker.cmd volt verify --mode bmc   --depth 24 examples/vga/frame_buffer.volt  # 15 props, 212 s
```

Results: Verilator `-Wall` clean (3 modules), 7/7 tests, VgaTiming 8/8
in `bmc 12` and `prove 3`, FrameBuffer 15/15 in `bmc 24`. The two
multi-clock runs that do NOT pass, and why, are in §6 below.

Yosys `synth_xilinx` (hdlc/formal image): FrameBuffer = 1 RAMB18E1 +
3 RAM32M (the FIFO) + 53 FDRE; whole VgaTop = 211 cells, 97 FDRE, the
same single RAMB18E1.

---

## Findings

### 1. Two clock domains

- **Declaring** the domains was trivial (`pub domain X { clock = posedge, reset = sync active_high }`).
  In a multi-file unit the root scope is shared (ADR-0042), so both live
  in `vga_timing.volt` and are imported with `use vga::vga_timing::{SysDomain, PixDomain}`.
  Domains import like any other item; nothing special was needed.
- **Annotation count:** 24 `@Domain` annotations across the three
  files — 2 in `vga_timing.volt` (one is the required `pix_clk`
  annotation, the other is the same on the port list), 11 in
  `frame_buffer.volt`, 11 in `vga_top.volt`. Every non-clock port of a
  two-clock module needs one; registers, `let`s and `wire`s never do
  (K4/K5 infer them from the `on` block / operands). That ratio felt
  right: ports are the contract, internals are inferred.
- **K2 in the multi-clock modules:** VgaTiming has one clock and uses
  the single-clock rule — only `pix_clk` carries `@PixDomain`, the
  five other ports say nothing. In FrameBuffer/VgaTop K2 simply
  switches off and K3 takes over; there is no partial mode ("default
  to the first clock"), which is the right call.
- **E3010 count:** once, and only in the deliberate experiment (an
  unannotated `in a : bool` in a two-clock module). While writing the
  real design it never fired, because the port lists were written
  with the domain in mind from the start. The message lists both
  candidate clocks with their domain names and proposes the exact
  annotation — good.
- **Message quality:** E3001 shows both domain definitions, both
  operand spans with their domain, a reason and a `sync()` fix-it that
  names the destination clock's domain. The instance-binding form
  (`a_addr: wa` in a `DualPortRam`) underlines the whole binding and
  labels both sides. All good. Two rough edges: an E3001 raised
  *inside a contract* uses the combinational-glitch explanation
  ("signals meet at a gate"), which is nonsense for an assertion; and
  every experiment module that has no sequential logic gets two W1001
  "unused input port" warnings for its clocks, so the real diagnostics
  drown in noise for tiny modules.

### 2. CDC bridges

Four crossings in the final design:

| Crossing | Direction | Width | Mechanism |
|---|---|---|---|
| pixel write commands | sys → pix | 14 bits | `AsyncFifo<u14, 16>` inside FrameBuffer |
| `invert` control | sys → pix | 1 | `sync(invert, pix_clk)` |
| `done_r` (fill finished) | sys → pix | 1 | `sync(done_r, pix_clk)` |
| `frame_tick` (vsync level) | pix → sys | 1 | `sync(vs_active, sys_clk)` |

- **`sync()` was enough** for the three single-bit control signals;
  the multi-bit path needed `AsyncFifo`. `HandshakeSync` would also
  have worked for the (rare) write commands, but the FIFO gives
  back-pressure for free (`wr_full` is the only sys-side status).
- **Multi-bit data:** the (x, y, bit) triple is packed into one `u14`
  word. There is no concatenation operator, so packing is
  `(bit << 13) | ((y as u14) << 7) | (x as u14)` and unpacking is
  `cmd[12:0]` / `cmd[13]`. Verilator flagged the two spare bits of a
  first `u16` attempt (UNUSEDSIGNAL); the word was narrowed to 14.
- **W3003** appeared only in the experiment (`sync(a : u8, ...)`).
  Its help text lists the three alternatives (AsyncFifo, HandshakeSync,
  gray coding) — that is exactly the decision table a designer needs.
- **`sync()` placement restriction:** `let s = sync(x, clk)` is
  accepted by the checker and rejected by the emitter with E0003
  "function calls (including sync) is not supported in F0". The
  synchronizer output must be a `wire` or an output port assignment.
  The same expression in a `let` and in an assignment should not
  differ; this cost one build iteration.
- **Deliberate violations:** ten experiment modules (scratch file, not
  in the repo). All ten were caught:

  | Experiment | Result |
  |---|---|
  | unannotated port in a two-clock module | E3010 |
  | `b = a` across domains | E3001 (assignment form) |
  | `b = a && c` across domains | E3001 (combinational form) |
  | `on pix_clk { r <= a }` with `a @SysDomain` | E3001 |
  | `reg mem : [bool; 16]` written on `sys_clk`, `mem[ra]` read on `pix_clk` | E3001 (the array read joins @SysDomain with the @PixDomain index) |
  | `DualPortRam` with `a_addr/a_wr_data/a_wr_en` from the other domain | 3× E3001 on the bindings |
  | user-module instance with a port bound from the wrong domain (K8) | E3001 |
  | `invariant: a -> c` with `a`, `c` in different domains | E3001 |
  | `sync(a : u8, ...)` | W3003 |
  | `sync()` inside one domain | W3002 |

  The one thing that is not caught is a *logical* CDC mistake behind
  a legal bridge, e.g. a pulse carried by `sync()` that the other
  clock is too slow to see; `PulseSync` (W3005) exists for that, but
  nothing stops a designer from using `sync()` instead.

### 3. Frame buffer

- **stdlib `DualPortRam` could not be used as a dual-clock memory.**
  It has one `clk` port (ADR-0029: "two independent ports on the
  same clock"). Binding the write port from `@SysDomain` gives three
  E3001s. A hand-written `reg` array is caught the same way (the read
  in the pixel `on` block joins the array's domain with the index's).
  So Volt currently offers no way to express a true dual-clock RAM —
  not in stdlib, not by hand. An `extern module` wrapper pushes the
  body into an SV file the compiler does not check, but since ADR-0047
  the BOUNDARY is checked: `@Src`/`@Dst` symbolic domains on the
  extern ports are bound by the clock connections and every other
  port is verified against them (E3001 on a wrong-domain binding,
  E3014 on two clocks for one domain). What remains unchecked is the
  inside of the black box, and SV emission of extern instances is
  still E0003.
- **What was done instead:** move the crossing off the memory and onto
  the write commands (`AsyncFifo<u14, 16>`), keep the RAM single-clock
  in `PixDomain` with port A for the FIFO drain and port B for the
  scan-out. This is a legitimate design (it is what many real VGA
  cores do), and it is the only shape the type system accepts. Cost:
  ~6 pix_clk of write latency, a 16-entry FIFO, and the writer sees
  `wr_full` instead of a free-running port.
- **Port domains:** the two sides are written as two port groups in
  one module, `@SysDomain` on the six write-side ports and
  `@PixDomain` on the four read-side ports. That was clear enough; a
  bundle (ADR-0039) per side would shorten it, but bundles carry one
  domain per field anyway.
- **Address types:** `DualPortRam` addresses are `bits<13>` while
  arithmetic wants `u13`; `((rd_y as u13) << 7) | (rd_x as u13)` then
  `as bits<13>` (same width, allowed). The FIFO word slice
  `cmd[12:0]` is already `bits<13>` and binds directly. Two E2003s
  ("expected 'bits<13>', found 'u13'") on the way.
- **W3006** (write-write collision) fires even though `b_wr_en` is
  the constant `false`; the checker does not fold constants for that
  warning.
- **Synthesis:** the `DualPortRam<bool, 8192>` maps to exactly one
  RAMB18E1 (Yosys `synth_xilinx`), the FIFO storage to 3 RAM32M
  (distributed), 53 FDRE for pointers, synchronizers and the aligned
  read. Yosys prints six "Resizing cell port … DIADI 64→16" warnings
  from its own BRAM mapping of a 1-bit-wide memory; harmless.
- **Lint detail:** port A's `a_rd_data` is always emitted and was
  unused → UNUSEDSIGNAL. It is now exported as `wr_prev` (the
  read-first value of the overwritten cell) and surfaces at the top
  as `fb_dbg`. There is no lint-off pragma in Volt, so unused
  primitive outputs have to be consumed by design.

### 4. Timing constraints

- **`@timing(pixel_clock >= 25.175.mhz)` does not parse:** there is
  no float literal, so `25.175` lexes as `25` `.` `175` and the
  parser reports E0001 "expected field name after '.'" — an
  unhelpful message for what is really "no decimal literals".
- **`@timing(pix_clk = 25175000)` parses and is NOT enforced -- and
  since ADR-0048 the compiler says so.** The attribute is in
  `KNOWN_ATTRIBUTES` (no W0020), its arguments are parsed as
  expressions and then dropped: `@timing(pixel_clock >= 25175000)`
  with a name that exists nowhere also compiles. Before ADR-0048 this
  was silent; now every unenforced attribute (`@timing`, `@budget`,
  `@false_path`, `@multicycle`, `@version`, `@abi_version`, `@dft`,
  `@debug_visible`, `@debug_trace`, `@synthesis_target`, `@domain`)
  produces **W0021** with the reason ("SDC generation is not
  implemented yet") and the opt-out (`@allow(unenforced)` on the item,
  or `[lint] unenforced_attributes = "allow"` in Volt.toml). This
  example keeps the attribute and the warning on purpose: `volt check
  examples/vga/vga_top.volt` reports `0 error(s), 2 warning(s)` (W0021
  here, W3006 in the frame buffer). Nothing downstream reads `attrs`
  except `@strict_timing` (`volt-hir/src/timing.rs`) and the W0021
  pass itself (`volt-hir/src/attrs.rs`).
- **No SDC/XDC output:** `build/` contains only `.sv`; the domain
  declaration's `frequency` key is parsed (`DomainKey::Frequency`)
  and equally ignored.
- **How the pixel-clock guarantee is expressed today:** as a comment.
  The natural home would be the domain declaration
  (`domain PixDomain { clock = posedge, frequency = 25175000 }`)
  with the compiler emitting `create_clock -period 39.72` per clock
  port and `set_clock_groups -asynchronous` between domains — the
  latter is the one constraint Volt already *knows* from the domain
  analysis and could generate with zero user input.

### 5. Simulation performance

| Test | Cycles | DUT |
|---|---|---|
| hsync timing | 1,456 | VgaTiming |
| vsync timing | 420,000 (a full frame) | VgaTiming |
| visible region | 384,000 | VgaTiming |
| frame buffer write read | 17 | FrameBuffer |
| checkerboard fill | 24,015 | VgaTop |
| fill bar until done | 2 | VgaTop |
| frame tick reaches system side | 393,610 | VgaTop |

Total 1.22 M cycles. `volt test` wall time 14.4 s, of which the
compiled binaries account for **82 ms** (VgaTiming 805 k cycles in
49 ms, VgaTop 418 k cycles in 33 ms, measured by running
`build/sim/vga_top_test/obj_*/V*` directly); everything else is
Verilator's C++ compile of the three DUTs. A full frame is therefore
not a problem at all — 420 k cycles cost less than the process
start-up — and the "keep the windows small" advice in the task does
not apply to designs of this size. `volt test` is usable for
multi-million-cycle runs; what would hurt is the per-DUT compile
(~4-5 s each), which is paid once per test file.

Two harness properties matter for multi-clock designs:
- **All clock ports toggle together.** `run_cycle` sets every
  `clock` port high, evals, sets every one low (`volt-sv-emit/src/sim.rs`).
  sys_clk and pix_clk are the same clock in simulation; synchronizer
  latencies are exercised, real asynchrony (ratio, phase, metastable
  windows) is not. There is no `step_sys(n)` / clock-ratio control in
  the test language.
- **One implicit `rst` for every domain**, applied for two cycles of
  the common clock. The generated RTL also has one `rst` input that
  both `always_ff` blocks use directly — no per-domain reset
  synchronizer is generated, which a real two-clock design needs.

The sibling rule (`X_test.volt` ↔ `X.volt`) also bit: the file was
first named `vga_test.volt` (as the task said) and produced E8501 "no
module with this name" for every test plus 65 cascading E8506s; it
must be `vga_top_test.volt`. One test file may instantiate several
DUTs (`VgaTiming`, `FrameBuffer`, `VgaTop` here) — one Verilator
model per DUT.

### 6. Formal

- **Contracts across domains cannot be written:** `invariant: a -> c`
  with operands in different domains is E3001 (§2 table). That is
  correct in principle, but there is no escape hatch for what a
  designer really wants to say ("after fill_done, every visible pixel
  matches the pattern"); such properties have to be split into
  single-domain pieces or left to simulation.
- **Single-clock module in a multi-clock design:** VgaTiming's 8
  properties are verified in `bmc 12` (1.5 s) and `prove 3`
  (1.2 s, boolector). Cover is the VGA-specific problem: `!hsync` is
  first reachable at step 656 and `!vsync` at step 392,000. With
  `--depth 700` boolector reaches the hsync cover in 5 s; the vsync
  and last-pixel covers are unreachable by BMC in practice (there is
  no free-initial-state cover mode).
- **Multi-clock modules run under `multiclock on` (clk2fflogic).**
  FrameBuffer `bmc 24` takes 212 s for 15 properties (the same bmc on
  VgaTiming takes 1.5 s) — clk2fflogic plus an 8192-bit memory is
  expensive, and 24 global steps are only ~12 edges per clock.
- **`prove` on VgaTop is currently unsound, and it is not the
  design's fault.** In multiclock mode the wrapper keeps
  `initial assume (rst)` from the single-clock flow, but registers
  only reset on a *sampled* posedge of their own clock. The
  counterexample (`build/formal/vgatop_cex.vcd`) has `rst = 1` at
  step 0 with `sys_clk` already high, `rst` low at step 1, and the
  first rising edge of `sys_clk` at step 2 — `wx_r` starts at 104 and
  `invariant: wx_r < 80` "fails at cycle 2". The multi-clock wrapper
  needs to hold `rst` until every clock has seen a rising edge (or
  constrain the initial state). This affects every user contract in a
  two-clock module; the stdlib primitives' own contracts include a
  "no mid-trace reset" assumption but not this.
- **FrameBuffer `prove 3` fails induction on the `AsyncFifo`'s own
  `(wbin - rbin) <= 16` invariant** (the stdlib doc already calls the
  FIFO contracts weakened). The driver reports it as "SymbiYosys
  reported a tool error (exit code 4)" instead of "induction failed
  at framebuffer.sv:167" — the UNKNOWN status is not mapped.
- **Cover diagnostics point at the wrong contract:** an unreached
  cover in `--mode cover` is reported as `error[E5001]: contract
  violated` on the module's *first* invariant (or on the AsyncFifo
  instance in FrameBuffer). The SBY log says "Unreached cover
  statement at line N"; that line, not the first invariant, should be
  in the message.
- **Contract clock in a multi-clock module is the first clock port**,
  whatever the property's domain: FrameBuffer's `cover: fifo_valid_r`
  (a PixDomain register) is emitted as `always @(posedge sys_clk)`.
  With a common clock in simulation nobody notices; in formal it
  samples a pixel-domain register on the system clock.

### 7. What is missing

Language features that would have made this easy, roughly by impact:

1. **A dual-clock memory primitive** (`DualClockRam<T, DEPTH>` with
   `wr_clk`/`rd_clk`), or a way to declare a `reg` array with two
   port domains. Every video/DMA design wants one, and today the only
   type-checked route is "FIFO the writes".
2. **Per-domain reset** (`reset = sync active_high` is declared per
   domain but a single `rst` is emitted) plus a generated reset
   synchronizer; and the matching formal wrapper fix (hold reset until
   each clock has ticked).
3. **`@timing` / `frequency` semantics** with SDC output; at minimum
   `set_clock_groups -asynchronous` from the domain analysis. The
   attribute now warns (W0021) instead of being dropped silently; the
   SDC mapping itself is planned in ADR-0048.
4. **Test-language clock control:** a clock ratio per test or
   `step_pix(n)` / `step_sys(n)`, so a 2-clock design is simulated with
   two clocks.
5. **Cross-domain contracts** with an explicit sampling clock
   (`invariant @PixDomain: sync-free property over synced copies`), or
   at least a clearer E3001 text for contracts.
6. Smaller: a concatenation operator (`{y, x}`), float/unit literals
   (`25.175.mhz`), `sync()` allowed in `let`, constant folding for
   W3006, a lint-off pragma for unused primitive outputs, a warning
   when a Volt identifier is an SV keyword (`cell` silently produced
   invalid SV — Verilator "syntax error, unexpected cell").

### 8. Comparison with SystemVerilog

Volt: 314 lines of design (85 + 99 + 130) for 375 lines of generated
SV (49 + 135 + 191). Hand-written SV for the same thing would be
roughly 420-480 lines: the two `always_ff` blocks and comparators of
the timing module are the same size; the async FIFO (gray pointers,
two 2-flop synchronizers, full/empty) is ~90 lines that Volt
generates from one instantiation; each 2-flop synchronizer is ~10
lines ×3; the dual-port RAM template ~20 lines; and the SV version
would additionally need a reset synchronizer per domain (~15 lines)
that Volt does not write either.

Did Volt pay off on the CDC front? Yes, on three counts: (1) every
crossing is visible in the source as `sync()`/`AsyncFifo`, and every
attempt to cross without one — including the ones that look innocent
in SV, like reading a `reg` array from the other clock or binding an
instance port from the wrong domain — was rejected with a message
that names both domains and the fix; (2) the multi-bit warning turned
a class of subtle bugs into a compile-time decision; (3) the async
FIFO and synchronizers are generated, not copied. The price is that
the type system also rejects the one thing that is legitimately dual-
clock (a true dual-port RAM), the formal flow is not yet ready for two
clocks (reset ordering, contract clock choice), simulation runs both
clocks as one, and the timing side (`@timing`, `frequency`, SDC) is
parsed but does nothing -- since ADR-0048 it at least says so (W0021).
