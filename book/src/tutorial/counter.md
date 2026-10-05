# A counter

<div class="chapter-goal">

In this chapter you will write a counter from a blank file: a module
with ports, a register, a clocked `on` block and a continuous
assignment. You will learn how Volt decides the bit width of a result and
why it refuses to drop bits silently, and you will test the counter in
simulation.

</div>

The output in this chapter was recorded with Volt 0.1.0 on Windows 11,
with Verilator running in Docker (see [Install Volt](../tour/install.md)).

## A file of its own

A project with a `Volt.toml` is handy, but a single `.volt` file works too.
Make a new directory and create a file named `counter.volt` in it.

## A module, ports and a register

```volt,file=counter.volt
// A free-running 8-bit counter.

pub module Counter {
    in  clk   : clock
    out count : u8

    reg count_r : u8 = 0

    on clk {
        count_r <= count_r + 1
    }

    count = count_r
}
```

Line by line:

- `pub module Counter { ... }` declares a circuit named `Counter`. `pub`
  lets other files use it; the [multi-file chapter](multi-file.md) comes
  back to that.
- `in clk : clock` and `out count : u8` are the ports: the wires that
  connect the module to the outside. Each port has a direction (`in` or
  `out`), a name and a type. `clock` is a clock input; `u8` is an unsigned
  number of 8 bits (0 to 255). `bool` is a single bit that is `true` or
  `false`.
- `reg count_r : u8 = 0` declares a register: 8 bits of storage. The value
  after `=` is its reset value.
- `on clk { ... }` says what happens at each rising edge of `clk`.
  `count_r <= count_r + 1` gives the register its next value.
- `count = count_r` is a continuous assignment: the output `count` always
  shows the current value of `count_r`.

The `_r` suffix is a naming habit for registers, not a rule.

<div class="box new-to-hw">

**New to hardware?** Clocks, flip-flops and reset

A *clock* is a signal that switches between 0 and 1 at a steady rate, for
example 50 million times a second. A *flip-flop* stores one bit and changes
it at the moment the clock rises, and at no other time. A register is a
row of flip-flops: `count_r` is eight of them. So the line
`count_r <= count_r + 1` does not run in a loop; it describes an adder
whose output is stored at every clock edge. *Reset* is a separate input
that puts every register back to a known value, here 0, for example after
power-up.

</div>

Check the file:

**Type this:**

```console
volt check counter.volt
```

**You should see:**

```text,output
    Checking counter.volt
    Finished 0.00s
      Result 0 error(s), 0 warning(s)
       Next: volt build counter.volt   (emit SystemVerilog)
```

`volt build counter.volt` writes `build/rtl/Counter.sv`. The module part
of it:

```systemverilog
module Counter (
    input  logic       clk,
    input  logic       rst,
    output logic [7:0] count
);

    logic [7:0] count_r;

    always_ff @(posedge clk) begin
        if (rst) begin
            count_r <= 8'd0;
        end else begin
            count_r <= count_r + 8'd1;
        end
    end

    assign count = count_r;

endmodule
```

The reset input `rst` was added for you: a module with a single clock gets
a synchronous, active-high reset, and every register returns to its reset
value while `rst` is high.

<div class="box from-sv">

**Coming from SystemVerilog?** `reg`, `on` and the two kinds of assignment

A `reg` with its `on` block becomes one `always_ff` with the reset branch
generated from the initial value; there is no separate reset code to keep
in sync. `<=` is allowed inside an `on` block and nowhere else, and `=`
outside it: writing `o = r` inside `on clk` is error `E0006`. There is no
`always_comb` and no `always @*`, so there are no inferred latches. Unlike
Verilog's `reg` keyword, a Volt `reg` is always a flip-flop.

</div>

## Enable and step inputs

A counter that always runs is not very useful. Add an `enable` input, a
`step` input for the amount to add, and an output `low` with the lower four
bits of the count:

```volt,file=counter.volt,should_fail=E2001
// An 8-bit counter that adds `step` on every clock edge while
// `enable` is high.

pub module Counter {
    in  clk    : clock
    in  enable : bool
    in  step   : u4
    out count  : u8
    out low    : u4

    reg count_r : u8 = 0

    on clk {
        if enable {
            count_r <= count_r + step
        }
    }

    count = count_r
    low   = count_r
}
```

`if enable { ... }` inside the `on` block means: at a clock edge, update
the register when `enable` is high, and keep its value otherwise.

This version has a mistake on purpose. Check it:

**Type this:**

```console
volt check counter.volt
```

**You should see:**

```text,output
    Checking counter.volt
error[E2001]: an 8-bit value does not fit in a 4-bit target
   ┌─ counter.volt:20:13
   │
20 │     low   = count_r
   │             ^^^^^^^ implicit narrowing is not allowed
   │
   = reason: narrowing drops the upper bits; in hardware that truncation must be visible
   = help: keep the low 4 bits explicitly: (expr)[3:0] as u4
   = for more: volt explain E2001


    Finished 0.00s
      Result 1 error(s), 0 warning(s)
```

## Bit widths

Every value in Volt has a width, and Volt never drops bits without being
told. The rules you need for now:

- **An addition can grow by one bit.** `count_r + step` adds an 8-bit and a
  4-bit number; the exact sum needs up to 9 bits. Assigned to the 8-bit
  register, the sum keeps its lower 8 bits: the counter wraps from 255 back
  to a small number, which is what a counter should do. Assigned to a
  9-bit target, the sum keeps its carry. A `let` without a type keeps
  the carry too: with `a` and `b` both `u8`, `let sum = a + b` is a 9-bit
  value.
- **A wider target is fine.** A 12-bit output assigned `count_r + step`
  gets the operands widened to 12 bits before the addition.
- **A narrower target is an error.** `low` has 4 bits and `count_r` has 8.
  Copying one into the other would lose the upper 4 bits, so Volt asks you
  to say so (`E2001`).
- **Literals must fit.** `reg r : u4 = 20` is an error (`E2010`), because 4
  bits hold at most 15.

The help line shows the fix: name the bits you want. `count_r[3:0]`
selects bits 3 down to 0; the result is `bits<4>`, four raw bits with no
number meaning, and `as u4` reads them as a number. Nothing is dropped by
the cast, so there is no warning. (A plain `count_r as u4` compiles too,
but warns with `W2010`: a narrowing cast can hide a mistake as well as
state an intent.)

```volt,file=counter.volt
// An 8-bit counter that adds `step` on every clock edge while
// `enable` is high.

pub module Counter {
    in  clk    : clock
    in  enable : bool
    in  step   : u4
    out count  : u8
    out low    : u4

    reg count_r : u8 = 0

    on clk {
        if enable {
            count_r <= count_r + step
        }
    }

    count = count_r
    low   = count_r[3:0] as u4
}
```

**Type this:**

```console
volt check counter.volt
```

**You should see:**

```text,output
    Checking counter.volt
    Finished 0.00s
      Result 0 error(s), 0 warning(s)
       Next: volt build counter.volt   (emit SystemVerilog)
```

The generated SystemVerilog makes each width explicit:

```systemverilog
    always_ff @(posedge clk) begin
        if (rst) begin
            count_r <= 8'd0;
        end else begin
            if (enable) begin
                count_r <= count_r + 8'(step);
            end
        end
    end

    assign count = count_r;
    assign low = count_r[3:0];
```

<div class="box from-sv">

**Coming from SystemVerilog?** No silent truncation or extension

SystemVerilog sizes an expression from its context and truncates or
zero-extends to fit: `assign low = count_r;` compiles, and a lint tool may
or may not warn. Volt makes narrowing an error and widening explicit in the
output (`8'(step)`), so `verilator -Wall` has nothing to say about
`WIDTHEXPAND` or `WIDTHTRUNC`. The sum of an addition may be assigned at
its operand width (it wraps, as a counter needs) or wider (it keeps the
carry); a narrower target needs a cast. Signed and unsigned never mix
implicitly (`E2002`).

</div>

## Test it

Create `counter_test.volt` next to `counter.volt`. A file named
`X_test.volt` sees the modules of `X.volt`:

```volt,file=counter_test.volt
// Simulation tests for Counter.

test "counts by step while enabled" {
    let dut = Counter { };
    dut.enable = true;
    dut.step = 3;
    step(4);
    assert_eq(dut.count, 12);
    assert_eq(dut.low, 12);
}

test "holds while disabled" {
    let dut = Counter { };
    dut.step = 1;
    step(5);
    assert_eq(dut.count, 0);
}

test "wraps around after 255" {
    let dut = Counter { };
    dut.enable = true;
    dut.step = 15;
    step(17);
    assert_eq(dut.count, 255);
    assert_eq(dut.low, 15);
    step(1);
    assert_eq(dut.count, 14);
    assert_eq(dut.low, 14);
}
```

Each test starts a fresh counter with its reset applied (`Counter { }`).
Inputs not set by the test are 0, which is why the second test sees no
change: `enable` stays `false`. `step(n)` lets `n` clock edges pass. In the
last test, 17 steps of 15 reach 255, and one more step gives 270, which
wraps to 270 − 256 = 14.

**Type this:**

```console
volt test counter_test.volt
```

**You should see:**

```text,output
   Compiling counter_test.volt
running 3 tests
note: Verilator not found locally; running it in Docker (verilator/verilator:v5.052)
test counts_by_step_while_enabled ... ok
test holds_while_disabled ... ok
test wraps_around_after_255 ... ok

test result: ok. 3 passed; 0 failed
```

<div class="box new-to-hw">

**New to hardware?** Why test with numbers that wrap

Hardware counters, timers and addresses have a fixed number of bits, and
most bugs hide at the edges: the step from the largest value back to 0,
or a value that is one too large for its field. A good test drives the
design to those edges on purpose, as the last test does.

</div>

## What you learned

- A `module` has typed `in` and `out` ports; `clock`, `bool`, `u8` and
  `bits<4>` are types.
- `reg name : type = value` is a register with its reset value; `on clk`
  updates registers with `<=`; `=` outside it connects outputs.
- Additions may wrap at the operand width or keep the carry one bit wider;
  narrowing is an error until you say which bits you want.
- `test` blocks drive inputs, advance the clock with `step()` and check
  outputs; `volt test` runs them in Verilator.

Next: [a state machine with enum](enum-fsm.md) turns a sequence of steps
into states.
