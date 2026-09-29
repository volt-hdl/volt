# A clock domain crossing error

<div class="chapter-goal">

In this chapter you will write a design with two clocks, pass a signal
from one to the other the wrong way, and read the error Volt reports. Then
you will fix it with `sync()` and see what the fix becomes in
SystemVerilog.

</div>

## Two clocks

Real designs often have more than one clock: a fast one for the core, a
slower one for a peripheral. Each clock, with the registers it drives, is
a *clock domain*.

In the `blinky` project, create a file `crossing.volt`. A button is
sampled by the fast clock; a LED is driven by the slow clock:

```volt,file=crossing.volt,should_fail=E3001
// A button sampled in one clock domain drives a LED in another.

domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

pub module Crossing {
    in  fast_clk : clock @Fast
    in  slow_clk : clock @Slow
    in  rst      : reset(sync, active_high)
    in  button   : bool  @Fast
    out led      : bool  @Slow

    reg pressed_r : bool = false
    on fast_clk {
        pressed_r <= button
    }

    reg led_r : bool = false
    on slow_clk {
        led_r <= pressed_r
    }

    led = led_r
}
```

The two `domain` blocks name the clock domains. `@Fast` and `@Slow` put
each port into one of them. The reset `rst` is shared: Volt synchronizes it
to each clock by itself. The register `pressed_r` belongs to `Fast`,
because `on fast_clk` updates it; `led_r` belongs to `Slow`.

This design is wrong on purpose. Check it:

```console
$ volt check crossing.volt
    Checking crossing.volt
error[E3001]: direct assignment between clock domains
   ┌─ crossing.volt:27:9
   │
 3 │ domain Fast {
   │        ---- source @Fast defined here
   ·
 8 │ domain Slow {
   │        ---- destination @Slow defined here
   ·
27 │         led_r <= pressed_r
   │         ^^^^^    --------- @Fast
   │         │         
   │         @Slow
   │
   = reason: the destination register may sample the source signal during an unstable window (metastability)
   = note: for multi-bit data, AsyncFifo may be safer
   = help: synchronize into the target domain with sync(): dest = sync(src, <clock of Slow>)
   = for more: volt explain E3001


    Finished 0.01s
      Result 1 error(s), 0 warning(s)
```

The error says which line crosses (27), where each domain comes from (lines
3 and 8), why this is a problem and how to fix it. `volt explain E3001`
prints a longer explanation. The command exits with code 1, and `volt
build` would write no SystemVerilog.

<div class="box new-to-hw">

**New to hardware?** Why a crossing needs a synchronizer

A flip-flop samples its input at the clock edge. If the input changes
right at that moment, the flip-flop can hang between 0 and 1 for a while
before it settles; this is called *metastability*. Two clocks that are not
related drift against each other, so sooner or later a change lands right
on an edge. A *synchronizer*, two flip-flops in a row on the receiving
clock, gives the front one a full clock period to settle before anyone
reads the value. The bug is rare and random, which is why it is hard to
find in the lab and why Volt rejects it at compile time.

</div>

## The fix

`sync()` builds the synchronizer. Add a line that brings `pressed_r` into
the `Slow` domain, and read that value under `slow_clk`:

```volt,file=crossing.volt
// A button sampled in one clock domain drives a LED in another.

domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

pub module Crossing {
    in  fast_clk : clock @Fast
    in  slow_clk : clock @Slow
    in  rst      : reset(sync, active_high)
    in  button   : bool  @Fast
    out led      : bool  @Slow

    reg pressed_r : bool = false
    on fast_clk {
        pressed_r <= button
    }

    let pressed_s = sync(pressed_r, slow_clk)

    reg led_r : bool = false
    on slow_clk {
        led_r <= pressed_s
    }

    led = led_r
}
```

`sync()` goes on a line of its own at module level, as in
`let pressed_s = sync(pressed_r, slow_clk)`; inside an `on` block it is not
supported yet. Check again:

```console
$ volt check crossing.volt
    Checking crossing.volt
    Finished 0.00s
      Result 0 error(s), 0 warning(s)
       Next: volt build crossing.volt   (emit SystemVerilog)
```

`volt build crossing.volt` writes `build/rtl/Crossing.sv`. The `sync()`
line became two flip-flops on `slow_clk` (output trimmed):

```systemverilog
    // CDC synchronizer: pressed_r -> slow_clk
    logic sync_pressed_r_stage0;
    logic sync_pressed_r_stage1;

    always_ff @(posedge slow_clk) begin
        if (rst_sync_slow_clk_stage1) begin
            sync_pressed_r_stage0 <= 1'b0;
            sync_pressed_r_stage1 <= 1'b0;
        end else begin
            sync_pressed_r_stage0 <= pressed_r;
            sync_pressed_r_stage1 <= sync_pressed_r_stage0;
        end
    end

    assign pressed_s = sync_pressed_r_stage1;
```

The file also holds one reset synchronizer per clock
(`rst_sync_fast_clk_stage1`, `rst_sync_slow_clk_stage1`): the shared `rst`
is asserted at once but released in step with each clock.

<div class="box from-sv">

**Coming from SystemVerilog?** The same bug compiles there

```systemverilog
always_ff @(posedge slow_clk)
    led_r <= pressed_r;   // pressed_r is clocked by fast_clk
```

SystemVerilog accepts this without a warning; a CDC tool or a review has
to catch it later. In Volt every signal carries its clock domain in its
type, the way a Rust value carries its type, so a crossing is a type error.
`sync()` is a compiler primitive, not a library module, so the compiler
knows the crossing is safe. It is meant for one bit; a multi-bit `sync()`
is a warning (`W3003`), and words cross through the built-in `AsyncFifo`.
A synchronizer you write by hand out of two registers is not recognized.

</div>

Next: [tests and waveforms](tests-and-waveforms.md).
