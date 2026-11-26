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

**Wrong results without a diagnostic.** These open bugs of Volt 0.1.4
compile without an error and give a result other than the one the source
says. "Suspected" marks a bug that was found by reading the code and has
not been reproduced by running it yet. The paragraphs below give the
workaround of each.

- [#214](https://github.com/volt-hdl/volt/issues/214): a `const` that does not fit its type is accepted; compile time and the hardware see different values.
- [#372](https://github.com/volt-hdl/volt/issues/372): when the source of a `sync()` is an instance output or is reached through more than 8 `let`s, the constraint files leave the path into the synchronizer untimed (`set_false_path`).
- [#421](https://github.com/volt-hdl/volt/issues/421): a signal annotated with a clock port name (`@a_clk`) loses its source clock in the constraint files, and a `sync()` of such a port gets no capture register.
- [#435](https://github.com/volt-hdl/volt/issues/435) (suspected): the clock of a `clock = none` domain may get a `create_clock` line in the SDC file.
- [#208](https://github.com/volt-hdl/volt/issues/208): nothing clocked by a clock output of an `extern module` (a PLL, a clock buffer) is checked for clock-domain crossings.
- [#209](https://github.com/volt-hdl/volt/issues/209): a register's reset value can be a signal, and it skips the clock-domain and trust checks.
- [#217](https://github.com/volt-hdl/volt/issues/217): `E5010` is missed when values of different delays meet in a `comb` block.
- [#219](https://github.com/volt-hdl/volt/issues/219): a timing constraint on a register `slow` also applies to a register named `slow_reg…`.
- [#226](https://github.com/volt-hdl/volt/issues/226): `@false_path` and `@multicycle` are written to the SDC file without a check, and the timing tool then leaves the path untimed.
- [#210](https://github.com/volt-hdl/volt/issues/210): a secret passes through an unclassified input port of a submodule to a public output without `E3009`.
- [#211](https://github.com/volt-hdl/volt/issues/211): the trust annotation of a `wire` or a `let` is not read.
- [#154](https://github.com/volt-hdl/volt/issues/154): a struct `let`'s flattened field shadows a port or signal of the same name in the generated SystemVerilog.
- [#155](https://github.com/volt-hdl/volt/issues/155): a module-level `let` named like a bundle port or `regs` is accepted, and the name means two things.
- [#161](https://github.com/volt-hdl/volt/issues/161): a constant imported with `use … as` is written into the SystemVerilog under its alias, a name that does not exist there.
- [#162](https://github.com/volt-hdl/volt/issues/162): a test block reads a private constant of another file.
- [#229](https://github.com/volt-hdl/volt/issues/229): a port bound twice in one instance takes the last binding, and `@false_path` or `@multicycle` on a struct port constrains nothing.
- [#220](https://github.com/volt-hdl/volt/issues/220): an `@mmio` module accepts an unaligned base address and a repeated argument, and a `*/` in a `///` comment ends the C comment of the generated header.
- [#222](https://github.com/volt-hdl/volt/issues/222): a malformed `Volt.toml` is ignored, and `volt build` exits with 0.
- [#382](https://github.com/volt-hdl/volt/issues/382): a `$finish` in the SystemVerilog body of an `extern` module does not end a `volt test` run, so a test can pass after the check in it failed.

**Errors, false warnings and failures.** These open bugs of Volt 0.1.4
do not give a wrong result: Volt stops with an error or warns where
nothing is wrong, or a step fails. The paragraphs below describe more
bugs of this kind, each with its workaround.

- [#306](https://github.com/volt-hdl/volt/issues/306): the one-line install failed on Ubuntu 26.04 in the project's tests, because of a dangling symbolic link; if it fails for you, use the [manual install](tour/install.md#manual-install).
- [#342](https://github.com/volt-hdl/volt/issues/342): binding a reset port with a spec to a child's `in rst : reset` (no spec) gets a false `E2003` ("expected 'reset', found 'reset'"); write the same spec on the child's port, `reset(sync, active_high)`.
- [#344](https://github.com/volt-hdl/volt/issues/344): a module without registers that instantiates an `extern` and a Volt child with its own raw reset port gets a false `W3011`, whose label names a synchronizer that is not generated; the warning can be ignored there.
- [#362](https://github.com/volt-hdl/volt/issues/362): `delay<K>` inside a `fn` body has no effect on the timing check, and nothing says so; write `delay<K>` at the call site.
- [#422](https://github.com/volt-hdl/volt/issues/422): a constant written with `as` is checked with its uncut value (a false `W2013` for `x << (9 as u3)`, a false `E2006` for an index `const K : u32 = 5 as u2`), and a width constant that uses `clog2` or `~` stops `volt build` with an `E2005` that calls it not constant; write the value itself (`1`, `4`).
- [#442](https://github.com/volt-hdl/volt/issues/442): `prev()` of a named constant (`invariant: prev(M) == 9`) passes `volt check`, but `volt test` and `volt verify` stop with a Verilator syntax error, because the helper register is named after the constant's value (`past_8'd9_1`); write the constant without `prev()` (`invariant: M == 9`): the earlier value of a constant is the constant.
- [#443](https://github.com/volt-hdl/volt/issues/443): when an array register is assigned from a packed port or wire, or assigned to one, as a whole (`r <= arr`, `all = r`), `volt check` and `volt build` pass, but Verilator rejects the generated SystemVerilog (`SEL is not an unpacked array`, `CASTSIZE`); assign the elements one by one (`r[0] <= arr[0]`, …; `all[0] = r[0]`, …), for example `on clk { for i in 0..4 { r[i] <= arr[i] } }` and `for i in 0..4 { all[i] = r[i] }`.
- [#79](https://github.com/volt-hdl/volt/issues/79): two clock ports of one domain (`in c1 : clock @c0`) get a false `W3010` ("reset 'rst' is shared by 2 clocks"); the warning can be ignored there, and its suggestion, a raw reset port, adds synchronizers the design does not need.
- [#124](https://github.com/volt-hdl/volt/issues/124): a variable part-select whose start always folds to a constant out of range (`s[(p | 31) +: 20]` on a 20-bit `s`) compiles, and `volt test` stops with Verilator's `SELRANGE` warning; such a select reads nothing inside the value, so remove it or write the start you mean.
- [#251](https://github.com/volt-hdl/volt/issues/251): two generic modules with the same name in two packages get a misleading `E1001` ("undefined name: 'M_8'") instead of `E1003`; give one of them another name.
- [#390](https://github.com/volt-hdl/volt/issues/390): a chain of constants in which each one names the one before twice (`const A2 : u64 = A1 + A1`) takes time that doubles with each link, so `volt check`, `volt build` and the editor stop responding on a short file; name the earlier constant once (`const A2 : u64 = 2 * A1`).
- [#448](https://github.com/volt-hdl/volt/issues/448): when a name used in a module-level `for` with thousands of iterations is undefined, `volt check`, `volt build` and the editor take seconds to report the `E1001`, even on a file of a few lines; the delay goes away once the name is declared, so fix the `E1001` it reports (or lower the loop bound while you do).

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

**A `sync()` can lose its source clock in the constraint files.** When
the source of a `sync()` is an instance output (`let so = s.o`), is
reached through more than 8 `let`s, or is a signal annotated with a
clock port name (`in d : bool @a_clk`) instead of a domain name, the XDC
file writes `set_false_path` instead of
`set_max_delay -datapath_only`, so the path into the synchronizer is not
timed; with `@a_clk` a port source also gets no capture register.
Annotate signals with the domain name (`@A`), and look for a `?` in the
`# sync` comments of the constraint files (`# sync 'sync_so': ? -> sclk`):
for each one, add the `set_max_delay -datapath_only` line yourself.
Tracked in [issue #372](https://github.com/volt-hdl/volt/issues/372) and
[issue #421](https://github.com/volt-hdl/volt/issues/421).

**A `clock = none` domain's clock may get a `create_clock` line
(suspected).** The constraint generator turns every clock port into a
clock constraint without looking at the domain's edge. This was found by
reading the code and has not been run yet. If your design has such a
domain, check its SDC file and remove a `create_clock` for that clock.
Tracked in [issue #435](https://github.com/volt-hdl/volt/issues/435).

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

<a id="comb-latch"></a>**An incomplete assignment in a `comb` block infers a latch.**
`comb { if c { y = 1 } else { if d { y = 2 } } }` compiles, and the
generated `always_comb` keeps the old `y` when `c` and `d` are both false
(Verilator reports `LATCH`). A `wire` that is read and never driven
compiles too (Verilator reports `UNDRIVEN`). Assign every output of a
`comb` block on every path, for example with a default assignment at the
top of the block (`y = 0`), and drive every `wire`. Tracked in
[issue #230](https://github.com/volt-hdl/volt/issues/230).

**A port bound twice in one instance takes the last binding.**
`Child { x: 1, x: a }` compiles without a diagnostic and connects `x` to
`a`. Bind each port once. Tracked in
[issue #229](https://github.com/volt-hdl/volt/issues/229).

**`delay<K>` adds no register.** `delay<K>(x)` tells the timing check
that `x` is K cycles older than its inferred delay. It adds no hardware,
and Volt does not check the claim: `let sum = delay<1>(a) + b` compiles,
but `a` and `b` are still of different ages. To align two values, add a
stage register, as the help of `E5010` shows: a
`reg a_d2 : Delayed<u32, 2> = 0` loaded with `a_d2 <= a` in the `on`
block, and `let sum = a_d2 + b`. Making `delay<K>` a marked escape is
tracked in [issue #358](https://github.com/volt-hdl/volt/issues/358).

**`@false_path` and `@multicycle` are not checked.** They are written to
the SDC file for any two registers, also for a real one-cycle path, and
the timing tool then leaves that path untimed. `volt explain E6003` and
`E6004` say that Volt proves these attributes; it does not, and the two
codes are never reported. Treat both attributes as your own claim. On a
port whose type is a `struct` or a `struct port`, neither attribute
constrains anything: `@false_path in cfg : Cfg` writes
`get_ports cfg`, which matches none of the generated ports (`cfg_a`,
`cfg_b`), or no line at all for a `struct port`. Declare such inputs as
plain ports. Tracked in
[issue #226](https://github.com/volt-hdl/volt/issues/226),
[issue #231](https://github.com/volt-hdl/volt/issues/231) and
[issue #229](https://github.com/volt-hdl/volt/issues/229).

**Reading an input port of an instance writes an undeclared name.** With
`let s = Sub { x: a }`, `p = s.x` compiles and is written as
`assign p = s_x;`, which Verilator rejects. Read the value you bound
(`p = a`). Tracked in
[issue #218](https://github.com/volt-hdl/volt/issues/218).

**An array literal assigned to an array `wire` fails in Verilator.**
`xs = [x0, x1]` with `wire xs : [u8; 2]` is written as
`assign xs = '{x0, x1};`. Assign the elements one by one:
`comb { xs[0] = x0  xs[1] = x1 }`. Tracked in
[issue #135](https://github.com/volt-hdl/volt/issues/135).

**A constant index of a wider type than the address stops `volt test`.**
With `const K : u4 = 2`, `a[K]` on a `u8` is written as `a[4'd2]`, and
Verilator stops with `WIDTHTRUNC`. Give the constant the width of the
address (`const K : u3 = 2` for an 8-bit value). Tracked in
[issue #134](https://github.com/volt-hdl/volt/issues/134).

**Two `@mmio` modules in one design do not compile, and some `@mmio`
arguments are not checked.** The second `@mmio` module gets `E1001` for
the AXI bundle names, also in another file. A base address that is not a
multiple of 4 is accepted, a repeated argument takes the last value, and
a `///` comment is copied into the generated C header as it is, so a `*/`
in it ends the C comment. Use one `@mmio` module per design, a base
address that is a multiple of 4, each argument once, and no `*/` in the
comments of an `@mmio` module. Tracked in
[issue #220](https://github.com/volt-hdl/volt/issues/220).

**A struct port whose field has a private struct type of its own file is
rejected** with `E1001` where the port is used. Make the field's struct
`pub` and import it too. Tracked in
[issue #163](https://github.com/volt-hdl/volt/issues/163).

**`declassify()` in a module-level `for` or a generic module gets a
false `E3009`.** Write the assignments without the `for`, one per
element, and keep `declassify()` out of generic modules. Tracked in
[issue #212](https://github.com/volt-hdl/volt/issues/212).

**A submodule output that depends on public inputs alone is taken as
secret** when another input of the same instance is secret, and gets a
false `E3009`. Put the public part in a submodule of its own, whose
inputs are all public. Tracked in
[issue #195](https://github.com/volt-hdl/volt/issues/195).

**A source file that begins with a byte order mark is rejected**
(`E0001: unexpected character: "\u{feff}"`). Some Windows editors write
this mark when they save as "UTF-8 with BOM"; save the file as UTF-8
without it. Tracked in
[issue #229](https://github.com/volt-hdl/volt/issues/229).

**A malformed `Volt.toml` is ignored.** When `Volt.toml` is not valid
TOML, `volt build` compiles and exits with 0, without a diagnostic. If a
setting seems to have no effect, check the syntax of the file. Tracked
in [issue #222](https://github.com/volt-hdl/volt/issues/222).

**A `$finish` in an `extern` body does not end a test.** When the
SystemVerilog body of an `extern` module (`@source`) calls `$finish` to
stop on a failed check, `volt test` prints Verilator's `Verilog $finish`
line and goes on, and the test can pass. Write the check in Volt instead,
as a contract of the module that instantiates the `extern`
(`invariant: a <= 100`) or as an `assert_eq` in the test: a violated
contract fails the test. Tracked in
[issue #382](https://github.com/volt-hdl/volt/issues/382).

**An array register assigned or read as a whole is rejected by
Verilator.** With `reg r : [u6; 4]`, `on clk { r <= arr }` is written as
`r <= 6'(arr);` and `all = r` as `assign all = 24'(r);`: casts to the
width of one element and of the whole port, on an unpacked array. `volt
check` and `volt build` accept the design, and Verilator rejects the
generated SystemVerilog, so `volt test` stops with an error. Assign and
read the elements one by one:
`on clk { for i in 0..4 { r[i] <= arr[i] } }` and
`for i in 0..4 { all[i] = r[i] }`. Tracked in
[issue #443](https://github.com/volt-hdl/volt/issues/443).

**Some explanations in `volt explain` are wrong.** The solution of
`E3012` and `E3011` (`r <= sync(fast_data, slow_clk)` in an `on` block)
gets `E0003`: write `let s = sync(x, clk)` at module level, as the
compiler's own help says. `W0010` says that `==` binds before `&`; Volt
reads `a & b == c` as `(a & b) == c`. `E3004`, `E3005`, `E6001`,
`E6003`, `E6004`, `E7001`, `E7002`, `E9001`, `E9002`, `W2011`, `W2020`
and `W2021` are explained as checks, but the compiler never reports
them. Where an explanation and the compiler disagree, follow the
compiler. Tracked in
[issue #225](https://github.com/volt-hdl/volt/issues/225) and
[issue #226](https://github.com/volt-hdl/volt/issues/226).

**The project that `volt new` creates names its module `Counter`**, a
name of the standard library, so instantiating it from another module
gets `E1016`. Rename the module (and `top` in `Volt.toml`) before you
use it in a larger design. Tracked in
[issue #101](https://github.com/volt-hdl/volt/issues/101).

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

**Integer literals stop at 2¹²⁸ - 1.** A literal holds a value up to
2¹²⁸ - 1 in any base. A larger one, such as
`0x1_0000_0000_0000_0000_0000_0000_0000_0000`, is `E0005` ("value does
not fit in 128 bits"), also when the target is `bits<256>`. Nothing is cut
silently: a literal up to 2¹²⁸ - 1 cast to `bits<256>` keeps its value
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
the subtraction. With a = b = 2²⁰ the result is -2²⁰ instead of
2⁴⁰ - 2²⁰. The rule changes in v0.2.0. Until then Volt warns
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
Docker. With that optimizer on, Verilator 5.052 and 5.054 print wrong
values for some correct SystemVerilog
([#148](https://github.com/volt-hdl/volt/issues/148), reported upstream as
[verilator#9060](https://github.com/verilator/verilator/issues/9060)). On
the example designs the option changed the time of `volt test` by less
than the difference between two runs. If you run Verilator on the
generated SystemVerilog yourself, add `-fno-dfg` too.

**Use Verilator 5.054 for `volt test`.** The project tests every
simulation result with Verilator 5.054, and the Docker image of `volt
test` holds that release. Verilator 5.050, 5.051 and 5.052 give 0 instead
of -1 for a negative signed value shifted right by an amount that is a
constant for Verilator and at least the width of a C++ word (32 or 64
bits, or the width rounded up to 32 bits). Volt writes a constant amount
in a form those releases compute correctly, but it cannot do that when
the amount is a port that a module instance ties to a constant, as
`G { a: a, k: 64 }` with `y = a >> k` in `G`
([#309](https://github.com/volt-hdl/volt/issues/309)). If `volt doctor`
shows a local Verilator 5.050 to 5.052, update it, or set
`VOLT_TOOL_BACKEND=docker`.

**A signed shift of a wide sum can simulate wrong in Verilator 5.054.**
Four conditions together: the value is signed and wider than 64 bits,
its width is one more than a multiple of 32 (65, 97, 129 bits), it is
computed by an expression such as `(e + f)` rather than read from a port
or signal, and it is shifted right by a port that a module instance ties
to a constant at least the width rounded up to 32 bits (96 for 65 bits).
For a negative value Verilator 5.054 then gives `...0001` instead of all
ones; synthesis, Yosys and `volt verify` give all ones. Reported upstream
as [verilator#9256](https://github.com/verilator/verilator/issues/9256),
tracked in [issue #311](https://github.com/volt-hdl/volt/issues/311).
Workaround: make the amount a `const` or a generic parameter of the
module that shifts (`G<130> { ... }` with `y = (e + f) >> N`): Volt then
writes the shift in a form that Verilator computes correctly.

**Every clock runs at the same rate in simulation.** `volt test` and
`volt run` give each clock port of the design one rising edge per cycle,
all at the same moment. A design with several clocks cannot be simulated
with clocks of different speeds or with a phase shift between them, so
a test checks the order and the latency of a crossing, not its timing.
The crossing itself is checked by the compiler (`E3001`). An example is
in [Two clocks and CDC](tutorial/two-clocks.md#test-it).

**Test values are at most 64 bits wide.** The test language cannot write
to a port wider than 64 bits or compare a wider value. A module with an
input wider than 64 bits cannot be tested directly: `volt test` stops
with a C++ error from the Verilator build (`VlWide`), also when the test
does not use that input
([issue #224](https://github.com/volt-hdl/volt/issues/224)).
Workaround: test the design through a small wrapper module whose ports
are 64 bits or narrower, for example `Wide { key: lo as uint<128> as bits<128> }`
with `in lo : u64`. `volt run` does drive and print wide ports.
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
