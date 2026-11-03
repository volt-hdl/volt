# Reset synchronizers

<div class="chapter-goal">

Take a raw asynchronous reset into a design with several clocks, and let the compiler generate one synchronizer per clock. The recipe explains the reset warnings `W3009` and `W3010`.

</div>

<div class="before-you-start">

**Before you start**

- **Window:** a terminal ([how to open one](../setup.md#open-a-terminal))
  and your editor.
- **Folder:** your `volt-projects` folder. This recipe makes a new folder
  there, `resets`. **Type this:**

  ```console
  cd ~/volt-projects
  ```

- **Running:** nothing else. This page uses `volt check` and `volt build`,
  which do not need Docker.

</div>

The output on this page was recorded with Volt 0.1.0 in PowerShell on
Windows 11. On macOS and Linux, paths are written with `/` instead of
`\`. The recipe assumes you know clock domains from
[Two clocks and CDC](../tutorial/two-clocks.md).

## The rule

A reset has two moments: when it starts (*assert*) and when it ends
(*release*). Starting at any time is harmless: every register goes to
its reset value. Ending is not. A register that sees the release too
close to a clock edge can go metastable, and two registers of the same
domain can leave reset in different cycles. So the release has to come
in step with the clock of the registers it resets.

A reset button, a power-on reset chip or a pin of the board is in step
with no clock of your design. Volt calls such an input a *raw* reset,
and you declare it as a port with the type `reset(...)`, for example
`in rst_n : reset(async, active_low)`.

For a raw reset port, the compiler builds a *reset synchronizer* for
each clock the reset reaches: two flip-flops that start the reset at
once and end it in step with that clock. Each domain is reset from its
own synchronizer. Without a raw port, Volt adds a reset port by itself
(`rst` or `rst_n`) and assumes that whoever drives it releases it in
step with the clock; with several clocks it cannot be in step with all
of them, and Volt says so.

<div class="box new-to-hw">

**New to hardware?** Synchronous and asynchronous resets

A *synchronous* reset is an input like any other: a register reads it at
the clock edge, so the reset acts at the next edge. An *asynchronous*
reset acts at once, without waiting for an edge, which also works while
a clock is stopped. Either way the release must be in step with the
clock: for an asynchronous reset the flip-flop has timing rules for it
(*recovery* and *removal*), and a synchronous reset is sampled like data,
so a release close to the edge can be caught in either cycle.

</div>

Make a folder `resets` and go into it:

**Type this:**

```console
mkdir resets
```

**Type this:**

```console
cd resets
```

## Two clocks and a reset button

The design below has a core clock and a slower clock for a serial port.
Both domains use an asynchronous reset that is active low (0 means
reset), as the reset button of many boards is. Create the file
`board.volt` (as [Setup](../setup.md#create-a-file) shows):

**Create this file:** `board.volt`

```volt,file=board.volt,should_fail=E3003
// Two clocks reset by one push button.

domain Core {
    clock = posedge
    reset = async active_low
}

domain Uart {
    clock = posedge
    reset = async active_low
}

pub module Board {
    in  core_clk : clock @Core
    in  uart_clk : clock @Uart
    out beat     : bool  @Core
    out tick     : bool  @Uart

    reg beat_r : u4 = 0
    on core_clk {
        beat_r <= beat_r + 1
    }

    reg tick_r : u4 = 0
    on uart_clk {
        tick_r <= tick_r + 1
    }

    beat = beat_r == 15
    tick = tick_r == 15
}
```

The design has no reset port yet, so Volt adds one, `rst_n`, and both
domains would use it. Check it:

**Type this:**

```console
volt check board.volt
```

**You should see:**

```text,output
    Checking board.volt
error[E3003]: asynchronous reset 'rst_n' is shared by 2 clock domains without synchronization
   ┌─ board.volt:14:9
   │
 5 │     reset = async active_low
   │     ------------------------ asynchronous reset → port 'rst_n'
   ·
10 │     reset = async active_low
   │     ------------------------ same port 'rst_n'
   ·
14 │     in  core_clk : clock @Core
   │         ^^^^^^^^ 'rst_n' can be released synchronously to at most one of 'core_clk', 'uart_clk'
15 │     in  uart_clk : clock @Uart
   │         -------- also reset by 'rst_n'
   │
   = reason: a reset release that is synchronous to one clock is asynchronous to the other; registers of the second domain can leave reset in different cycles or go metastable (recovery/removal violation)
   = help: declare the reset as a raw input port; the compiler then adds a reset synchronizer for each clock:
         in rst_n : reset(async, active_low)
   = for more: volt explain E3003


    Finished 0.02s
      Result 1 error(s), 0 warning(s)
```

An asynchronous reset shared by two clocks is error `E3003`: its release
can be in step with one of them at most. The help line has the fix.
Add the line `in rst_n : reset(async, active_low)` below the clock
ports:

**Replace this file:** `board.volt`

```volt,file=board.volt
// Two clocks reset by one push button.

domain Core {
    clock = posedge
    reset = async active_low
}

domain Uart {
    clock = posedge
    reset = async active_low
}

pub module Board {
    in  core_clk : clock @Core
    in  uart_clk : clock @Uart
    in  rst_n    : reset(async, active_low)
    out beat     : bool  @Core
    out tick     : bool  @Uart

    reg beat_r : u4 = 0
    on core_clk {
        beat_r <= beat_r + 1
    }

    reg tick_r : u4 = 0
    on uart_clk {
        tick_r <= tick_r + 1
    }

    beat = beat_r == 15
    tick = tick_r == 15
}
```

The kind of reset in parentheses, `(async, active_low)`, must match the
`reset` line of the domains it feeds; a mismatch is `E3003` too. Check
again:

**Type this:**

```console
volt check board.volt
```

**You should see:**

```text,output
    Checking board.volt
    Finished 0.01s
      Result 0 error(s), 0 warning(s)
       Next: volt build board.volt   (emit SystemVerilog)
```

## The generated synchronizers

**Type this:**

```console
volt build board.volt
```

**You should see:**

```text,output
   Compiling board.volt
    Finished 0.01s (1 source file(s), 1 SV file(s))
     Output build\rtl\Board.sv (69 lines)
       Next: volt run board.volt      (simulate)
             volt verify board.volt   (prove contracts)
```

Open `build/rtl/Board.sv` (in the `resets` folder) in your editor. The
port `rst_n` feeds two synchronizers, one per clock, and the registers
of each domain are reset by the output of their own. This is a part of
the file (output trimmed):

**You should see:**

```systemverilog,output
    // reset synchronizer: rst_n -> core_clk (async assert, sync release)
    logic rst_sync_core_clk_stage0;
    logic rst_sync_core_clk_stage1;
    always_ff @(posedge core_clk or negedge rst_n) begin
        if (!rst_n) begin
            rst_sync_core_clk_stage0 <= 1'b0;
            rst_sync_core_clk_stage1 <= 1'b0;
        end else begin
            rst_sync_core_clk_stage0 <= 1'b1;
            rst_sync_core_clk_stage1 <= rst_sync_core_clk_stage0;
        end
    end

    // reset synchronizer: rst_n -> uart_clk (async assert, sync release)
    ...

    always_ff @(posedge core_clk or negedge rst_sync_core_clk_stage1) begin
        if (!rst_sync_core_clk_stage1) begin
            beat_r <= 4'd0;
        end else begin
            beat_r <= beat_r + 4'd1;
        end
    end
```

Read it like this. While `rst_n` is 0, both stages are 0 at once, without
waiting for a clock edge: the reset starts immediately. After `rst_n`
goes to 1, stage 0 becomes 1 at the next edge of `core_clk`, and stage 1
one edge later. `beat_r` is reset while stage 1 is 0, so it leaves reset
on an edge of `core_clk`, two edges after the button was let go. The
`uart_clk` side does the same with its own two stages.

<div class="box from-sv">

**Coming from SystemVerilog?** The synchronizer you would write by hand

The chain above is the usual "asynchronous assert, synchronous release"
reset synchronizer that a design team keeps in a library and
instantiates once per clock. In Volt you do not instantiate it: a raw
reset port is enough, and the compiler adds one chain for each clock the
port reaches, names it after the clock (`rst_sync_<clock>_stage1`) and
resets that domain's registers from it. A clock you add later gets its
chain without a code change. The chain has two stages; there is no
setting for more yet.

</div>

## One clock: W3009

A module with a single clock and an asynchronous reset, but no raw reset
port, gets warning `W3009`. Create `blinker.volt`:

**Create this file:** `blinker.volt`

```volt,file=blinker.volt,should_warn=W3009
// One clock with an asynchronous reset.

domain Core {
    clock = posedge
    reset = async active_low
}

pub module Blinker {
    in  clk : clock @Core
    out led : bool  @Core

    reg count_r : u4 = 0
    on clk {
        count_r <= count_r + 1
    }

    led = count_r[3]
}
```

**Type this:**

```console
volt check blinker.volt
```

**You should see:**

```text,output
    Checking blinker.volt
warning[W3009]: asynchronous reset 'rst_n' is assumed to be released synchronously to 'clk'
  ┌─ blinker.volt:9:9
  │
5 │     reset = async active_low
  │     ------------------------ asynchronous reset → port 'rst_n'
  ·
9 │     in  clk : clock @Core
  │         ^^^ 'rst_n' must be released synchronously to this clock
  │
  = reason: 'Blinker' is not instantiated in this unit, so nothing in Volt synchronizes its reset release
  = help: if 'rst_n' comes from a pad or a power-on reset, declare it as raw and the compiler adds the synchronizer:
        in rst_n : reset(async, active_low)
  = for more: volt explain W3009


    Finished 0.01s
      Result 0 error(s), 1 warning(s)
       Next: volt build blinker.volt   (emit SystemVerilog)
```

The reset port `rst_n` that Volt adds here carries a promise: it is
released in step with `clk`. Inside a Volt design the promise is kept,
because a parent module connects its own synchronized reset to the
child (see [Submodules](#submodules) below). `Blinker` is not used by
another module in this file, so nothing in Volt keeps it, and `W3009`
says so. What to do depends on where the reset comes from:

- **From a button, a pin or a power-on reset:** add
  `in rst_n : reset(async, active_low)`, as for `Board`. The warning
  goes away and the synchronizer is built.
- **From a design that already synchronizes it,** for example when
  `Blinker` is handed to a team that puts it into their SystemVerilog
  design: keep the module as it is. The warning documents what their
  reset must do.

## Two clocks with synchronous resets: W3010

`reset = sync` (the default) has the same problem in a milder form. A
synchronous reset is sampled like data, and when two clocks share the
reset port that Volt adds, its release is out of step with at least one
of them: warning `W3010`. The chapter
[Two clocks and CDC](../tutorial/two-clocks.md#reset-with-two-clocks)
shows the message.

The fix is the same one line, with the kind of reset of those domains:
`in rst : reset(sync, active_high)`. It is a warning and not an error
because of submodules: a module that is used inside a parent with the
same clocks cannot take a raw reset port of its own (the next section
shows why), and its one automatic reset port can carry the synchronized
reset of a single clock. There is no form of such a module without the
warning yet. At the top of a design, add the raw port.

## Submodules

A submodule does not synchronize its reset again. Its automatic reset
port is connected to the parent's synchronized reset for its clock.
Create `board_led.volt`, where the core of `Board` blinks a LED through
a submodule:

**Create this file:** `board_led.volt`

```volt,file=board_led.volt
// A board whose core blinks a LED through a submodule.

domain Core {
    clock = posedge
    reset = async active_low
}

domain Uart {
    clock = posedge
    reset = async active_low
}

module Blinker {
    in  clk : clock @Core
    out led : bool  @Core

    reg count_r : u4 = 0
    on clk {
        count_r <= count_r + 1
    }

    led = count_r[3]
}

pub module Board {
    in  core_clk : clock @Core
    in  uart_clk : clock @Uart
    in  rst_n    : reset(async, active_low)
    out led      : bool  @Core
    out tick     : bool  @Uart

    let blink = Blinker { clk: core_clk }

    reg tick_r : u4 = 0
    on uart_clk {
        tick_r <= tick_r + 1
    }

    led  = blink.led
    tick = tick_r == 15
}
```

**Type this:**

```console
volt build board_led.volt
```

**You should see:**

```text,output
   Compiling board_led.volt
    Finished 0.02s (1 source file(s), 2 SV file(s))
     Output build\rtl\Blinker.sv (30 lines)
     Output build\rtl\Board.sv (67 lines)
       Next: volt run board_led.volt      (simulate)
             volt verify board_led.volt   (prove contracts)
```

`Blinker` gets no `W3009` here, because `Board` uses it. In
`build/rtl/Board.sv`, the instance is connected to the synchronizer of
`core_clk` (output trimmed):

**You should see:**

```systemverilog,output
    Blinker blink (
        .clk  (core_clk),
        .rst_n(rst_sync_core_clk_stage1),
        .led  (blink_led)
    );
```

Do not give `Blinker` a raw reset port and pass `rst_n` to it: when the
parent synchronizes the same reset to the same clock, the two chains
release independently, and `volt check` reports `E3003` ("synchronized
twice").

## More than one reset

A design can have one raw reset per domain, for example a separate
reset for the serial port. Write the domain after the type:

- `in core_rst_n : reset(async, active_low) @Core`
- `in uart_rst_n : reset(async, active_low) @Uart`

Each port then feeds the clocks of its domain. When a module has more
than one raw reset port, every one needs its `@Domain`; otherwise Volt
cannot tell which clocks a port feeds, and reports `E3010`.

## In simulation and formal verification

You do not drive a raw reset port in a test. `volt test` and `volt run`
hold it active for two cycles, release it, and wait until the
synchronizers have passed the release on, so a test starts with the
design out of reset. `volt verify` assumes the design starts in reset
and checks the contracts from the release on.

You are in the `resets` folder.

## What Volt does not check

- **The order of resets.** Volt checks that each release is in step with
  its clock. It does not check that one domain leaves reset before
  another, and it does not model a reset that depends on a condition.
  The domain keys `reset_cycles` and `reset_sequence` are reserved for
  this; writing one is `E0003` ("not supported yet").
- **The reset of an `extern` module.** An `extern` instance connected to
  a raw reset port gets the raw reset, not the synchronized copy
  (warning `W3011`); see
  [Known limitations](../limitations.md#clock-and-reset-domains).
- **The length of the chain.** Each synchronizer has two stages.

## Summary

| Your design | What Volt does |
|---|---|
| One clock, `reset = sync` (the default), no raw port | adds `rst`, assumed in step with the clock; no warning |
| One clock, `reset = async`, no raw port, not used by another module | adds `rst_n`, warning `W3009` |
| Several clocks, `reset = sync`, no raw port | adds `rst`, warning `W3010` |
| Several clocks, `reset = async`, no raw port | error `E3003` |
| A raw port, `in rst_n : reset(...)` | one synchronizer for each clock the port reaches |
| A submodule | gets its parent's synchronized reset |
| Several raw ports | each needs `@Domain`, otherwise `E3010` |

`volt explain W3009`, `volt explain W3010` and `volt explain E3003` print
the reasons in more detail.
