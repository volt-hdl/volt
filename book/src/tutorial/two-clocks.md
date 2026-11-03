# Two clocks and CDC

<div class="chapter-goal">

You will declare clock domains, see why a signal cannot simply cross from one to another, and use `sync()` and `AsyncFifo` to cross safely. The chapter also covers resets in a design with several clocks.

</div>

<div class="before-you-start">

**Before you start**

- **Window:** a terminal ([how to open one](../setup.md#open-a-terminal))
  and your editor.
- **Folder:** your `volt-projects` folder. This chapter makes a new folder
  there, `clocks`. **Type this:**

  ```console
  cd ~/volt-projects
  ```

- **Running:** for the section [Test it](#test-it), Docker Desktop with
  **Engine running** in its window
  ([Setup](../setup.md#install-and-start-docker-desktop) shows how): `volt
  test` runs the Verilator simulator in Docker. If Verilator is installed
  on your computer, Docker is not needed; `volt doctor` says which one
  `volt test` uses. The other sections need nothing but Volt.

</div>

The output on this page was recorded with Volt 0.1.0 in PowerShell on
Windows 11, with Docker Desktop running Verilator. On macOS and Linux,
paths are written with `/` instead of `\`.

## Two clocks, two domains

The [Tour](../tour/cdc-error.md) moved a single bit, a button, from one
clock to another. This chapter moves numbers. A sensor chip, here an
analog-to-digital converter (ADC), delivers 8-bit samples with a clock of
its own; the rest of the design runs on the system clock. Each clock,
with the registers it drives, is a *clock domain*. The samples have to
cross from the ADC domain into the system domain.

Make a folder `clocks` and go into it:

**Type this:**

```console
mkdir clocks
```

**Type this:**

```console
cd clocks
```

In this folder, create the file `sample_bridge.volt` (as
[Setup](../setup.md#create-a-file) shows). This version keeps the latest
sample in a register of the ADC domain and copies it into a register of
the system domain:

**Create this file:** `sample_bridge.volt`

```volt,file=sample_bridge.volt,should_fail=E3001
// Samples from a sensor clock reach the system clock.

domain Adc {
    clock = posedge
    reset = sync active_high
}

domain Sys {
    clock = posedge
    reset = sync active_high
}

pub module SampleBridge {
    in  adc_clk      : clock @Adc
    in  sys_clk      : clock @Sys
    in  rst          : reset(sync, active_high)
    in  sample       : u8    @Adc
    in  sample_valid : bool  @Adc
    out data         : u8    @Sys

    reg last_r : u8 = 0
    on adc_clk {
        if sample_valid {
            last_r <= sample
        }
    }

    reg data_r : u8 = 0
    on sys_clk {
        data_r <= last_r
    }

    data = data_r
}
```

What is new compared with the [counter](counter.md):

- A `domain` block names a clock domain and says how its registers work.
  `clock = posedge` means they update on the rising edge of the clock.
  `reset = sync active_high` means their reset is *synchronous*, read at
  a clock edge like any other input, and *active high*: 1 means reset.
- `@Adc` after a port type puts the port into the domain `Adc`. The
  clock port `adc_clk` is the clock of that domain.
- A register belongs to the domain of the `on` block that writes it:
  `last_r` to `Adc`, `data_r` to `Sys`.
- `in rst : reset(sync, active_high)` is the reset input of the design.
  [Reset with two clocks](#reset-with-two-clocks) below explains why it
  is written out here, while the counter got its reset without asking.

A design with a single clock needs no `domain` at all; Volt puts
everything into one domain by itself.

This design is wrong on purpose. Check it:

**Type this:**

```console
volt check sample_bridge.volt
```

**You should see:**

```text,output
    Checking sample_bridge.volt
error[E3001]: direct assignment between clock domains
   ┌─ sample_bridge.volt:30:9
   │
 3 │ domain Adc {
   │        --- source @Adc defined here
   ·
 8 │ domain Sys {
   │        --- destination @Sys defined here
   ·
30 │         data_r <= last_r
   │         ^^^^^^    ------ @Adc
   │         │          
   │         @Sys
   │
   = reason: the destination register may sample the source signal during an unstable window (metastability)
   = note: for multi-bit data, AsyncFifo may be safer
   = help: synchronize at module level, outside this block: let s = sync(<signal>, <clock of Sys>), then read s here
   = for more: volt explain E3001


    Finished 0.01s
      Result 1 error(s), 0 warning(s)
```

Line 30 reads `last_r`, a register of `Adc`, in an `on sys_clk` block of
`Sys`. That is a *clock domain crossing* (CDC) without a synchronizer,
error `E3001`. The note line already points to the way out for a number:
`AsyncFifo`. The next section shows why the `sync()` of the Tour is not
enough here.

## Why sync() is not enough for a number

`sync()` builds a two-flop synchronizer for each bit. Do what the help
line suggests: synchronize `last_r` at module level and read the
synchronized name in the `on sys_clk` block. Replace the file:

**Replace this file:** `sample_bridge.volt`

```volt,file=sample_bridge.volt,should_warn=W3003
// Samples from a sensor clock reach the system clock.

domain Adc {
    clock = posedge
    reset = sync active_high
}

domain Sys {
    clock = posedge
    reset = sync active_high
}

pub module SampleBridge {
    in  adc_clk      : clock @Adc
    in  sys_clk      : clock @Sys
    in  rst          : reset(sync, active_high)
    in  sample       : u8    @Adc
    in  sample_valid : bool  @Adc
    out data         : u8    @Sys

    reg last_r : u8 = 0
    on adc_clk {
        if sample_valid {
            last_r <= sample
        }
    }

    let last_s = sync(last_r, sys_clk)

    reg data_r : u8 = 0
    on sys_clk {
        data_r <= last_s
    }

    data = data_r
}
```

**Type this:**

```console
volt check sample_bridge.volt
```

**You should see:**

```text,output
    Checking sample_bridge.volt
warning[W3003]: two-flop synchronization does not guarantee bit coherence for 8-bit signals
   ┌─ sample_bridge.volt:28:18
   │
28 │     let last_s = sync(last_r, sys_clk)
   │                  ^^^^^^^^^^^^^^^^^^^^^ bits may be captured on different edges
   │
   = reason: a two-flop synchronizer synchronizes each bit independently; when bits are captured on different clock edges an invalid intermediate value appears
   = help: for multi-bit data use one of:
                AsyncFifo<T, N>   — data streams
                HandshakeSync<T>  — single transfers
                gray coding       — counters
                AsyncDualPortRam<T, N> — random-access data
   = for more: volt explain W3003


    Finished 0.01s
      Result 0 error(s), 1 warning(s)
       Next: volt build sample_bridge.volt   (emit SystemVerilog)
```

The design compiles now, with warning `W3003`: eight synchronizers side
by side do not keep the eight bits together.

<div class="box new-to-hw">

**New to hardware?** Why the bits of a number fall apart

Each synchronizer settles on its own. When `last_r` goes from 127
(`0111_1111`) to 128 (`1000_0000`), all eight bits change at once. If
the system clock samples during the change, some synchronizers pass the
new bit one cycle before the others, and for that cycle the system
domain sees a mix of old and new bits: 255, 0, or any other value that
`last_r` never held. A single bit has no such problem: it is either old
or new.

</div>

It is a warning and not an error because some values survive it: a
counter in Gray code changes one bit at a time, and a setting that is
written once and read much later has long settled. For a stream of
samples, use a FIFO.

## A FIFO between the clocks

`AsyncFifo` is a built-in component: a small memory written on one clock
and read on the other, with the synchronizers it needs inside. Every
sample goes in on the ADC side and comes out on the system side, in the
same order. This version of the design also says when `data` holds a new
sample (`data_valid`), and when a sample was lost because the FIFO was
full (`dropped`). Replace the file:

**Replace this file:** `sample_bridge.volt`

```volt,file=sample_bridge.volt
// Samples from a sensor clock reach the system clock.

domain Adc {
    clock = posedge
    reset = sync active_high
}

domain Sys {
    clock = posedge
    reset = sync active_high
}

pub module SampleBridge {
    in  adc_clk      : clock @Adc
    in  sys_clk      : clock @Sys
    in  rst          : reset(sync, active_high)
    in  sample       : u8    @Adc
    in  sample_valid : bool  @Adc
    out data         : u8    @Sys
    out data_valid   : bool  @Sys
    out dropped      : bool  @Adc

    wire take : bool @Sys

    let fifo = AsyncFifo<u8, 4> {
        wr_clk:  adc_clk,
        wr_data: sample,
        wr_en:   sample_valid,
        rd_clk:  sys_clk,
        rd_en:   take,
    }

    take = !fifo.rd_empty

    reg valid_r : bool = false
    on sys_clk {
        valid_r <= take
    }

    data       = fifo.rd_data
    data_valid = valid_r
    dropped    = sample_valid && fifo.wr_full
}
```

**Type this:**

```console
volt check sample_bridge.volt
```

**You should see:**

```text,output
    Checking sample_bridge.volt
    Finished 0.01s
      Result 0 error(s), 0 warning(s)
       Next: volt build sample_bridge.volt   (emit SystemVerilog)
```

How it works:

- `AsyncFifo<u8, 4>` holds up to 4 values of type `u8`. The size must be
  a power of two.
- The ports that start with `wr_` belong to the domain of `wr_clk`, here
  `Adc`; the ports that start with `rd_` belong to the domain of
  `rd_clk`, `Sys`. Volt checks what you connect: `sample` is `@Adc` and
  goes to `wr_data`. Connecting `sample_valid` to `rd_en` would be error
  `E3001` again.
- `fifo.rd_empty` is true when there is nothing to read, and
  `fifo.wr_full` is true when there is no room. The read side reads
  whenever the FIFO is not empty: `take = !fifo.rd_empty`.
- `wire take : bool @Sys` declares the wire, with its domain, above the
  FIFO, because a name must be declared before it is used (error
  `E1002`). Its value is given further down. The `@Sys` is a promise
  that Volt checks: assigning it a value of `Adc` would be an error.
- `fifo.rd_data` is a register: a value read with `rd_en` appears on it
  one clock cycle later. `valid_r` delays `take` by the same cycle, so
  `data_valid` is true exactly when `data` holds a new sample.
- `dropped` is in the `Adc` domain, because both of its inputs are.

<div class="box from-sv">

**Coming from SystemVerilog?** A dual-clock FIFO without an IP core

In SystemVerilog you would instantiate a vendor FIFO generator or a
hand-written FIFO with Gray-coded pointers, and a CDC tool would be told
to trust it. `AsyncFifo` is generated into the module itself: Gray-coded
read and write pointers, a two-flop synchronizer for each pointer, and a
comment that names the crossing. The compiler knows which side belongs
to which domain, so the crossing is checked like any other. Other
built-in bridges: `HandshakeSync` for a single word now and then,
`PulseSync` for a one-cycle pulse, `AsyncDualPortRam` for a memory
(`volt explain stdlib`).

</div>

## Reset with two clocks

A reset has to be released in step with the clock of every register it
resets. With one clock that is easy. With two, the moment the reset
goes away may be right on an edge of one clock while it is far from the
edges of the other.

The line `in rst : reset(sync, active_high)` declares the reset as a
*raw* input: it comes from outside and is in step with neither clock.
For a raw reset, Volt builds one *reset synchronizer* for each clock that
it reaches, two flip-flops that pass the release on in step with that
clock. Each domain is then reset from its own copy.

Without that line, Volt still adds a reset port named `rst`, as for the
counter, but it cannot know which clock the reset is in step with, and
`volt check` warns:

**You should see (if the `rst` line is missing):**

```text,output

```

Keep the line. The recipe [Reset synchronizers](../cookbook/reset-sync.md)
shows the generated synchronizers, asynchronous resets and the related
errors.

## Test it

Create `sample_bridge_test.volt` next to `sample_bridge.volt`:

**Create this file:** `sample_bridge_test.volt`

```volt,file=sample_bridge_test.volt
// Simulation tests for SampleBridge. The test bench drives every clock
// of the design together, one edge per step().

test "samples cross in order" {
    let dut = SampleBridge { };
    dut.sample_valid = true;
    dut.sample = 10;
    step(1);
    dut.sample = 20;
    step(1);
    dut.sample = 30;
    step(1);
    dut.sample_valid = false;
    assert_false(dut.data_valid);
    step(1);
    assert_true(dut.data_valid);
    assert_eq(dut.data, 10);
    step(1);
    assert_eq(dut.data, 20);
    step(1);
    assert_eq(dut.data, 30);
    step(1);
    assert_false(dut.data_valid);
}

test "a full FIFO drops samples" {
    let dut = SampleBridge { };
    dut.sample_valid = true;
    step(3);
    assert_false(dut.dropped);
    step(1);
    assert_true(dut.dropped);
}
```

The test bench drives both clocks together: `step(1)` gives one rising
edge to `adc_clk` and one to `sys_clk`. The tests cannot run the two
clocks at different speeds, so they do not show the timing problems a
synchronizer protects against. What they show is the order of the
samples and how long a crossing takes. In the earliest test, the sample
10 is written on edge 1 and is on `data` after edge 4: the write pointer
has to pass a synchronizer before the read side sees it, and the read
itself takes a cycle. In the second test a sample arrives on every edge.
After four of them the write side sees the FIFO as full, because it
learns about the reads through a synchronizer too, and `dropped` turns
on: the next sample would be lost.

Check that Docker Desktop says **Engine running**, then run the tests:

**Type this:**

```console
volt test sample_bridge_test.volt
```

**You should see:**

```text,output
   Compiling sample_bridge_test.volt
running 2 tests
note: Verilator not found locally; running it in Docker (verilator/verilator:v5.052)
test samples_cross_in_order ... ok
test a_full_FIFO_drops_samples ... ok

cover summary:
  SampleBridge.fifo_cov_0 (sample_bridge.volt:25)  hit 1 time
  SampleBridge.fifo_cov_1 (sample_bridge.volt:25)  hit 10 times

test result: ok. 2 passed; 0 failed
```

The cover summary lists two covers that you did not write: `AsyncFifo`
brings them along, which is why both point to line 25, the `let fifo`
line. `fifo_cov_0` counts the cycles in which the FIFO was full,
`fifo_cov_1` those in which it was empty. Both were reached, so the
tests drove the FIFO to both of its limits.

## The SystemVerilog

**Type this:**

```console
volt build sample_bridge.volt
```

**You should see:**

```text,output
   Compiling sample_bridge.volt
    Finished 0.01s (1 source file(s), 1 SV file(s))
     Output build\rtl\SampleBridge.sv (127 lines)
       Next: volt run sample_bridge.volt      (simulate)
             volt verify sample_bridge.volt   (prove contracts)
```

`build/rtl/SampleBridge.sv` holds one module, with the FIFO and the reset
synchronizers inside. These comment lines in it name each part (output
trimmed):

**You should see:**

```systemverilog,output
    // reset synchronizer: rst -> adc_clk (async assert, sync release)
    // reset synchronizer: rst -> sys_clk (async assert, sync release)
    // AsyncFifo 'fifo': adc_clk -> sys_clk, depth 4 (gray-code pointers)
```

You are in the `clocks` folder.

## What you learned

- A `domain` names a clock domain; `@Name` puts a port or a wire into it,
  and a register belongs to the domain of its `on` block.
- Reading a signal of one domain in another is error `E3001`.
- `sync()` is for a single bit. On a number it is warning `W3003`,
  because the bits can arrive in different cycles.
- `AsyncFifo` carries a stream of values between two clocks; its `wr_`
  and `rd_` ports belong to the two domains.
- A raw reset port, `in rst : reset(...)`, gets one reset synchronizer
  per clock.
- `volt test` drives every clock together, one edge per `step()`.

Next: [contracts and formal verification](contracts-formal.md) prove
properties of a design for every input, not just the ones a test tries.
