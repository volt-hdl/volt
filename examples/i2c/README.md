# I2C master — pure Volt with `opendrain` pads (ADR-0051)

An I2C master (standard 100 kHz and fast 400 kHz, 7-bit addressing,
single-byte write and read, ACK/NACK, START / STOP / REPEATED START,
clock stretching). The first version of this example (§8 below, kept
as the record) was written to find out what happens when a Volt design
needs a bidirectional open-drain pin: the port direction compiled, but
there was no high-impedance value in the language, so the tri-state
buffer had to be a hand-written 69-line SystemVerilog wrapper. That
finding became ADR-0051; this version is what the design looks like
with it. There is no hand-written SV any more.

```
I2cMaster   (i2c_master.volt)      opendrain sda / scl, generated: assign sda = sda_drive_low ? 1'b0 : 1'bz
I2cTb       (i2c_test.volt)        wire sda_bus / scl_bus  ->  SV: tri1 (pull-up + wired-AND)
 ├── I2cMaster                     .sda(sda_bus), .scl(scl_bus)
 └── I2cSlaveModel (i2c_test.volt) opendrain sda / scl as well: address match, ACK, read data, stretching
```

| File | Lines | Content |
|---|---|---|
| `i2c_master.volt` | 349 (248 code) | 7-state FSM on an `enum I2cState` register as an exhaustive `match` in the `on clk` block (ADR-0074), 4-phase bit divider from `const CLKS_PER_BIT_*`, both lines read through `sync()`, 5 invariants + 7 covers written by hand (the state-valid invariant is generated from the enum) |
| `i2c_test.volt` | 555 | `I2cSlaveModel`, `I2cTb`, 12 simulation tests; the master comes in through `use i2c::i2c_master::I2cMaster` |

Before ADR-0051: 328 + 541 + `i2c_top.sv` 69 = 938 lines, four pad
ports per device (`sda_in` / `sda_oe` / `scl_in` / `scl_oe`) and a
Volt-side wired-AND bus model in the test bench. After: 898 lines, two
pad ports, no wrapper, no bus model.

## Building, linting, testing, verifying

```
volt build examples/i2c/i2c_master.volt                  # build/rtl/I2cMaster.sv (inout wire sda, 1'bz buffer)
volt build examples/i2c/i2c_test.volt                    # I2cMaster.sv I2cSlaveModel.sv I2cTb.sv (tri1 nets)

docker run --rm -v "$PWD/build/rtl:/work" -w /work verilator/verilator:latest \
    --lint-only -Wall --top-module I2cMaster I2cMaster.sv                       # clean
docker run --rm -v "$PWD/build/rtl:/work" -w /work verilator/verilator:latest \
    --lint-only -Wall --top-module I2cTb  I2cTb.sv I2cMaster.sv I2cSlaveModel.sv # clean, tri1 + 1'bz included

volt test examples/i2c/i2c_test.volt                     # 12 tests (Docker Verilator image, see ../README.md)

VOLT_SBY=build/sby-docker.cmd volt verify --mode bmc   --depth 12  --engine boolector examples/i2c/i2c_master.volt  # 24 props (12 written, 12 generated), 1.4 s
VOLT_SBY=build/sby-docker.cmd volt verify --mode prove --depth 3   --engine boolector examples/i2c/i2c_master.volt  # k-induction, 1.2 s
VOLT_SBY=build/sby-docker.cmd volt verify --mode cover --depth 190 --engine boolector examples/i2c/i2c_master.volt  # 7/7 reached, 13.5 s
```

Results: Verilator `-Wall` clean for both tops, 12/12 tests (all on the
first run against the timing table below), 13 properties in `bmc 12`
and `prove 3`, every cover reached: `stretched_r` step 8, both lines
held step 9 (START phase 3), `ack_seen_r` / `nack_seen_r` step 90, STOP
step 92, a completed read step 101, REPEATED START step 173 (fast
mode, 9 clocks per bit).

## The pad interface

```volt
opendrain sda : bool
opendrain scl : bool

wire sda_s : bool
sda_s = sync(sda.read(), clk)          // the other end is a slave with its own timing

on clk {
    ...
    1 => { sda.drive_low() }           // START: SDA falls while SCL is high
    ...
    _ => { sda.release()  scl.release() ... }
}

invariant: !busy_r -> (sda.released && scl.released)   // drive intent, provable
```

The compiler synthesises `sda_drive_low` / `scl_drive_low` registers
(reset value: released), turns every `drive_low()` / `release()` into a
`<=` to them, rewrites `sda.released` to `!sda_drive_low` and
`sda.read()` to the net itself, and emits

```systemverilog
inout  wire  sda,
...
assign sda = sda_drive_low ? 1'b0 : 1'bz;
```

A direct `sda = ...` or `sda <= ...` is E4008 (it would be a push-pull
driver on a shared line). Reading `sda.read()` straight into the FSM
would be W3007: the pad is by definition driven by an external device,
so both lines go through `sync()` first. `volt explain E4008` /
`W3007` have the full story.

**What the synchroniser costs.** Two flops = two clocks of latency on
every line. Phase 1 of a bit ("SCL released") only ends once the
synchronised SCL reads high, so in fast mode (2 clocks per phase) the
master waits one extra clock per bit: 9 clocks instead of 8 (355 kHz
at 3.2 MHz). In standard mode (8 clocks per phase) the latency is
hidden inside the phase. The `stretched` flag is raised on the second
consecutive stalled clock, i.e. only when a slave holds SCL longer
than the synchroniser explains. The system clock moved from 1.6 MHz
to 3.2 MHz for this version: with one clock per phase the slave's ACK
(driven three clocks after it sees SCL fall) cannot reach the master's
sample point two clocks after its own SCL release.

**Bit timing** (standard mode, 8 clocks per phase): p0 SCL low + SDA
set, p1 SCL released (wait for `scl_s`), p2 SCL high (sample `sda_s`
at the end), p3 SCL pulled low. START at E+16, ADDR E+32..E+288, ACK
slot SCL high E+296..E+312 (sampled at E+312), DATA E+320..E+576,
DATA_ACK E+576..E+608, STOP: SCL rises E+616, SDA rises E+624, idle at
E+640; a NACK on the address goes straight to STOP (idle at E+352).
The slave sees every bus event three clocks late (two synchroniser
clocks + one edge-detector register).

**Contracts** (all on registers, the pad ones on the synthesised drive
registers): `bit_count_r <= 8`, `clk_count_r < CLKS_PER_PHASE_STD`,
the idle-line rule `!busy_r -> (sda.released && scl.released)`, two
induction helpers (`state_r != I2cState::Idle -> busy_r`, `!busy_r ->
phase_r == 0 && clk_count_r == 0 && !stall_r`); covers for ACK, NACK,
stretching, STOP, REPEATED START, a completed read and both lines held
at once. The numeric version's `state_r < 7` is gone from the source:
with `state_r : I2cState` (seven variants in three bits) the compiler
generates the same fact as the state-valid invariant (ADR-0066 F1,
ADR-0074), so the property count is unchanged. In the formal model Yosys reads a released `'z`
net as constant 0, which would freeze phase 1 forever; `volt verify`
therefore models the external device as an unconstrained `(* anyseq *)`
driver that owns the line while it is released (ADR-0051 §4). The
`prove 3` run passed unchanged from the previous version: the
idle-line invariant is about the drive registers, not about `z`.

**Tests.** Scripts are linear, so the slave is a Volt module with the
same two `opendrain` pads; `I2cTb` declares `wire sda_bus` /
`wire scl_bus`, binds both devices to them and re-exports the resolved
levels and the slave's counters as `out` ports (`volt test` cannot
touch an `inout`, E8503/E8504). In SV the wires are `tri1` nets and
each device drives them with its own `1'bz` buffer — Verilator's
`--cc` resolves internal tri-state nets correctly (all 12 tests run on
this), and its `-Wall` lint has nothing to say about `tri1` or `1'bz`.
Tests: idle bus, START, the address byte on the wire bit by bit, ACK
on the line during the slot, NACK (slave disabled / address mismatch),
full write, STOP, read (`0xC3` back, master NACKs the last byte),
clock stretching (the slave holds SCL 40 clocks, the master finishes
22 clocks late with the data intact), fast mode (179 clocks) and
REPEATED START (write then read, 2 STARTs, 1 STOP, idle at E+1248).

**Verilator and tri-state, revisited.** Everything the first version
reported still holds: `--lint-only -Wall` is clean with `1'bz`,
`{8{1'bz}}` and `tri1`; `--cc` drops the buffer of a *top-level*
`inout` unless `--pins-inout-enables` is given. This design never
hits that: the master's pads are internal to `I2cTb`, where the
`tri1` net and the sub-module buffers are resolved by Verilator's
tristate pass. A top with `inout` pins would still need the
`--pins-inout-enables` flow in a C++ harness (ADR-0051 limits).

---

## 8. Findings from the first version (before ADR-0051)

Kept as the record that motivated the ADR. Everything in this section
describes Volt *without* `opendrain`; the numbers refer to the 16 /
4 clocks-per-bit design at 1.6 MHz.

### 8a. `inout` support

`inout sda : bool` parsed, resolved, type-checked and emitted with no
diagnostic in every combination tried (read only, continuous assign
only, both, and even a non-blocking `sda <= ...` inside `on clk`):
`inout logic sda` in the right position. What was missing was the
*value*: **Volt had no `z`**, no `Tristate`/`OpenDrain` type and no
output-enable construct, so the only thing an `inout` could be
assigned was an ordinary 0/1 expression -- a push-pull driver.
`assign sda = oe ? 1'b0 : 1'b1` on an I2C bus would short against the
pull-up and every other device. The way around it: `in sda_in` /
`out sda_oe` per line and a hand-written `i2c_top.sv` adding
`assign sda = sda_oe ? 1'b0 : 1'bz; assign sda_in = sda;`. The
`extern module` route did not work either: sv-emit still rejects any
extern instance with E0003 (ADR-0047 covers only the analysis layer).

ADR-0051 answer: `opendrain sda : bool`, `sda.drive_low()` /
`sda.release()`, compiler-generated buffer; direct assignment E4008.

### 8b. Read / write separation

An `inout` could be both read and assigned in one module and the
compiler said nothing: no driver conflict, no warning that a port is
read and written, and `sda <= !q_r` in an `always_ff` on an `inout
logic` even passed Verilator `-Wall`. ADR-0051 answer: the drive state
is a named register, read-while-driving is legitimate (ACK slot:
`release()` then `read()`), procedural assignment to the pad is E4008.

### 8c. Verilator

- `--lint-only -Wall` on `I2cMaster.sv`, the three-module `I2cTb` and
  the hand-written `I2cTop.sv` with `1'bz`: **clean, no tri-state
  warning at all**.
- `--cc` on `I2cTop` **without** `--pins-inout-enables`: the top-level
  `sda` is a single `VL_INOUT8` and the tri-state driver is
  **silently dropped** (`sda_oe_r` dead-code eliminated). **With** it:
  `sda` becomes `VL_IN8` plus `sda__out` / `sda__en`; the testbench has
  to resolve the bus itself. Still true for a top-level `inout`; the
  ADR-0051 test bench keeps the pads internal.

### 8d. Domain

The `inout` port took the module's only clock domain by K2 like every
other non-clock port; the master sampled `sda_in` directly with no
synchroniser and the compiler could not know the pin was asynchronous
-- the general "input pins are trusted" gap. ADR-0051 answer: a
bidirectional read is external by definition (W3007 unless it is the
source of `sync()` or inside a contract).

### 8e. Contracts

The tri-state *state* could not be expressed; what could be was the
hand-kept enable register (`!busy_r -> !sda_oe_r && !scl_oe_r`), proved
by k-induction after a STOP fix. A formal probe showed Yosys treats a
driven `inout logic` as the driven net (no `z`). ADR-0051 answer:
`sda.released` / `sda.driving` name the drive intent; the formal model
adds a free external driver for the released case.

### 8f. Testability

`volt test` cannot touch an `inout` at all (E8503/E8504, unchanged),
so the bus was modelled in Volt as `!(oe_a || oe_b)` and re-exported.
ADR-0051 answer: two `wire`s bound to both devices' pads, `tri1` in
SV, Verilator resolves them; the wrapper module is still needed for
the `out` re-exports. A `step_until(cond, max)` primitive and
hierarchical reads (`dut.s_i.byte_cnt`) are still missing.

### 8g. What is missing (status)

1. High-impedance value / open-drain port kind — **done** (ADR-0051,
   option (b): a dedicated direction with `drive_low`/`release`/`read`
   and contract fields).
2. Diagnostic for a value-assigned `inout` — **done** (E4008).
3. `extern module` instantiation in sv-emit — still E0003.
4. `volt test`: `step_until` and hierarchical reads — open.
5. `volt verify` reporting an induction failure as a counterexample
   instead of "tool error" — open.
6. Minor: no `bool -> uN` cast (E2009) — open.

### 8h. Comparison with SystemVerilog

A hand-written SV master with the same features is 250-300 lines; the
generated `I2cMaster.sv` is 350 (incl. two synchronisers and the
buffers) from 248 non-comment Volt lines. The one thing the first
exercise was about -- the tri-state buffer -- is now generated, and
the pin interface is one port per line instead of two. Where Volt
still wins: the seven contracts run through bmc / prove / cover in
seconds with no testbench glue, and the slave model, bus and twelve
tests are Volt too.
