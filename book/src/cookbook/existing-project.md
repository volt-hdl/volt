# Using Volt in an existing project

<div class="chapter-goal">

In this chapter you will take a SystemVerilog module you already have and
use it from a Volt design with `extern`. You will check the design, test
it together with the SystemVerilog module, and write it out as
SystemVerilog that the rest of your project compiles like any other file.

</div>

<div class="before-you-start">

**Before you start**

- **Window:** a terminal ([how to open one](../setup.md#open-a-terminal))
  and your editor.
- **Folder:** your `volt-projects` folder. This chapter makes a new folder
  there, `existing`. **Type this:**

  ```console
  cd ~/volt-projects
  ```

- **Running:** Docker Desktop, with **Engine running** in its window, for
  `volt test`. If Verilator is installed, Docker is not needed; `volt
  doctor` says which one `volt test` uses.

</div>

The output on this page was recorded with Volt 0.1.0 in PowerShell on
Windows 11, with Docker Desktop.

## The module you already have

Make the folder `existing`, go into it, and make a folder `rtl` inside it
for the SystemVerilog file:

**Type this:**

```console
mkdir existing
```

**Type this:**

```console
cd existing
```

**Type this:**

```console
mkdir rtl
```

The SystemVerilog module below stands for a module of your project. It
gives a pulse one clock cycle long each time its input goes from 0 to 1.
Create it in the `rtl` folder (as [Setup](../setup.md#create-a-file)
shows; in the clipboard command, write `rtl/RisePulse.sv` as the name):

**Create this file:** `rtl/RisePulse.sv`

```systemverilog,file=rtl/RisePulse.sv
// An existing SystemVerilog module: a one-cycle pulse on each rising
// edge of `level`.
module RisePulse (
    input  logic clk,
    input  logic level,
    output logic rise
);
    logic level_q;

    always_ff @(posedge clk) level_q <= level;

    assign rise = level & ~level_q;
endmodule
```

## Declare it with extern

A Volt design uses a SystemVerilog module through an `extern module`
declaration: the ports of the module, with their names, directions and
types, and the file that holds its body. This design counts button
presses with `RisePulse`. Create it in the `existing` folder:

**Create this file:** `press_counter.volt`

```volt,file=press_counter.volt
// Counts button presses. The rising-edge detector is an existing
// SystemVerilog module, RisePulse, in rtl/RisePulse.sv.

@source("rtl/RisePulse.sv")
extern module RisePulse {
    in  clk   : clock
    in  level : bool
    out rise  : bool
}

pub module PressCounter {
    in  clk    : clock
    in  button : bool
    out count  : u8

    let det = RisePulse { clk: clk, level: button }

    reg count_r : u8 = 0
    on clk {
        if det.rise {
            count_r <= count_r + 1
        }
    }

    count = count_r
}
```

`@source` names the SystemVerilog file, relative to the `.volt` file.
`let det = RisePulse { ... }` places one instance of the module, and
`det.rise` reads its output, as with a Volt module. Check the design:

**Type this:**

```console
volt check press_counter.volt
```

**You should see:**

```text,output
    Checking press_counter.volt
    Finished 0.05s
      Result 0 error(s), 0 warning(s)
       Next: volt build press_counter.volt   (emit SystemVerilog)
```

Volt does not read the SystemVerilog file: it checks that the file is
there, and it trusts the `extern` declaration for the ports. If the two
disagree, for example a port that the SystemVerilog module does not have,
`volt check` still passes; the mistake shows up as a Verilator error when
`volt test` compiles the two files together.

## Test the two together

A test file for this design looks like the one of
[Tests and waveforms](../tour/tests-and-waveforms.md). Create it:

**Create this file:** `press_counter_test.volt`

```volt,file=press_counter_test.volt
// Simulation tests for PressCounter. The design and the SystemVerilog
// body of RisePulse are simulated together.

test "a held button counts once" {
    let dut = PressCounter { };
    dut.button = true;
    step(5);
    assert_eq(dut.count, 1);
}

test "two presses count twice" {
    let dut = PressCounter { };
    dut.button = true;
    step(2);
    dut.button = false;
    step(2);
    dut.button = true;
    step(2);
    assert_eq(dut.count, 2);
}
```

**Type this:**

```console
volt test press_counter_test.volt
```

**You should see:**

```text,output
   Compiling press_counter_test.volt
running 2 tests
note: Verilator not found locally; running it in Docker (verilator/verilator:v5.052)
test a_held_button_counts_once ... ok
test two_presses_count_twice ... ok

test result: ok. 2 passed; 0 failed
```

`volt test` hands `rtl/RisePulse.sv` to Verilator together with the
SystemVerilog it generates, so the test runs the real body of
`RisePulse`. Change the line `assign rise = level & ~level_q;` in
`rtl/RisePulse.sv` to `assign rise = level;` and both tests fail; change
it back before you go on.

## Write SystemVerilog for your project

**Type this:**

```console
volt build press_counter.volt
```

**You should see:**

```text,output
   Compiling press_counter.volt
    Finished 0.01s (1 source file(s), 1 SV file(s))
     Output build\rtl\PressCounter.sv (41 lines)
       Next: volt run press_counter.volt      (simulate)
             volt verify press_counter.volt   (prove contracts)
```

Open `build/rtl/PressCounter.sv` in your editor. This is a part of it
(output trimmed):

**You should see:**

```systemverilog,output
module PressCounter (
    input  logic       clk,
    input  logic       rst,
    input  logic       button,
    output logic [7:0] count
);

    logic det_rise;

    RisePulse det (
        .clk  (clk),
        .level(button),
        .rise (det_rise)
    );
    ...
```

The module and its ports keep the names of `press_counter.volt`. Volt
adds a reset port, `rst`, which sets `count_r` back to 0. `RisePulse` is
placed with its ports connected by name, and gets no reset port: an
`extern` module has exactly the ports it declares.

`volt build` writes `PressCounter.sv` and nothing else; it does not copy
`rtl/RisePulse.sv`. The rest of your project compiles both files, as it
compiles its own.

<div class="box from-sv">

**Coming from SystemVerilog?** Placing the generated module

```systemverilog
PressCounter u_counter (
    .clk   (clk),
    .rst   (rst),
    .button(button),
    .count (count)
);
```

The generated file is plain SystemVerilog with no Volt package or
include. We placed `PressCounter` like this in a hand-written testbench and
compiled it with Verilator 5.052 from `build/rtl/PressCounter.sv` and
`rtl/RisePulse.sv` alone: `verilator --lint-only -Wall` reported nothing,
and the testbench counted two presses. If you stop using Volt, the
generated files are what you keep; the header says they were generated,
and the names in them are the names of your Volt source.

</div>

## What extern does not do

- **Volt does not read the SystemVerilog file.** The ports of the
  `extern` declaration are trusted; a mismatch is reported by Verilator
  during `volt test`, not by `volt check`.
- **No parameters.** Volt rejects a generic `extern module`. A
  parameterized SystemVerilog module is used with its default parameter
  values. For other values, write a small SystemVerilog wrapper module
  that sets them, declare the wrapper as the `extern`, and list both files
  in `@source("rtl/wrapper.sv", "rtl/module.sv")`.
- **No reset port is added.** If the SystemVerilog module has a reset,
  declare it as a port of the `extern` and connect it yourself. The
  module gets the reset you connect; when Volt puts a reset synchronizer
  in front of its own registers, the `extern` does not get the
  synchronized copy ([issue #93](https://github.com/volt-hdl/volt/issues/93)).
- **`volt build` does not copy the SystemVerilog file** into `build/`.
- **Clock domains stop at the ports.** The ports of an `extern` can carry
  clock domains, and Volt checks what you connect to them, but it cannot
  see a crossing inside the SystemVerilog module.
- **No standard library names.** An `extern` cannot be named like a
  module of the standard library (`EdgeDetect`, `SyncFifo`, `Counter`,
  ...; `volt explain stdlib` lists them): `volt check` reports `E1016`.
  The name of an `extern` is the name of its SystemVerilog module, so a
  SystemVerilog module called `Counter` goes behind a small SystemVerilog
  wrapper module with another name, and the `extern` declares the
  wrapper.

You are in the `existing` folder.

Next: [Wrapping SystemVerilog with extern](extern-sv.md).
