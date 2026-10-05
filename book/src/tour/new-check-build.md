# A project in three commands

<div class="chapter-goal">

In this chapter you will create a project with `volt new`,
check it with `volt check` and turn it into SystemVerilog with
`volt build`. You will also take a short look at the generated code.

</div>

<div class="before-you-start">

**Before you start**

- **Window:** a terminal: PowerShell on Windows, Terminal on macOS and
  Linux ([how to open one](../setup.md#open-a-terminal)). Keep your editor
  at hand too.
- **Folder:** your `volt-projects` folder. **Type this:**

  ```console
  cd ~/volt-projects
  ```

  No such folder? [Setup](../setup.md#make-a-folder-for-your-projects)
  makes it.
- **Running:** nothing else. This page does not need Docker.

</div>

The output on this page was recorded with Volt 0.1.0 in PowerShell on
Windows 11. On macOS and Linux, paths are written with `/` instead of `\`;
everything else looks the same.

## Create a project

**Type this:**

```console
volt new blinky
```

**You should see:**

```text,output
    Created minimal project 'blinky' in blinky
             Volt.toml, counter.volt, counter_test.volt, README.md, .gitignore
       Next: cd blinky
             volt check
             volt test
```

`volt new` made a folder named `blinky` with five files in it. (`volt new
--list` shows the other templates: two clock domains, a FIFO, a register
map. The default template is a small counter.) Go into the new folder:

**Type this:**

```console
cd blinky
```

The rest of the Tour, except the next chapter, works in this folder.

| File | What it holds |
|---|---|
| `Volt.toml` | the project manifest: its name and its top module, `Counter` |
| `counter.volt` | the design |
| `counter_test.volt` | simulation tests for the design |
| `README.md` | what to try next |
| `.gitignore` | tells Git, the version control tool, not to track the `build` folder that Volt writes. You can ignore it if you do not use Git. macOS and Linux hide names that start with `.`; `ls -a` lists them |

## The design

Open `counter.volt` in your editor (in VS Code: **File → Open Folder**,
choose `blinky`, then click `counter.volt`).

**You should see:**

```volt,file=counter.volt,from=templates/minimal/counter.volt,output
// An 8-bit counter with an enable input and a wrap-around value.
//
//   volt check     check the design (no output files)
//   volt build     write SystemVerilog to build/rtl/
//   volt test      run counter_test.volt in Verilator
//   volt verify    prove the contracts with SymbiYosys

// The counter goes back to 0 after this value.
const MAX : u8 = 9

// The next count. A `fn` is pure combinational logic, expanded where
// it is called.
fn next(count: u8) -> u8 {
    if count == MAX { 0 } else { count + 1 }
}

pub module Counter {
    in  clk    : clock
    in  enable : bool
    out count  : u8
    out wrap   : bool

    // Contracts. `volt verify` proves them for every input sequence;
    // `volt test` also checks them on every cycle of every test.
    invariant: count_r <= MAX
    cover: count_r == MAX

    reg count_r : u8 = 0

    on clk {
        if enable {
            count_r <= next(count_r)
        }
    }

    count = count_r
    wrap  = enable && count_r == MAX
}
```

You do not need to follow every line yet; the [counter
chapter](../tutorial/counter.md) builds a design like this one step by step.
In short: `Counter` has a clock input, an `enable` input and two outputs.
The register `count_r` counts from 0 to 9 and wraps back to 0. The two
lines starting with `invariant` and `cover` are contracts: rules about the
design that tools can check.

<div class="box new-to-hw">

**New to hardware?** A module is a circuit, not a function

A `module` describes a piece of hardware that exists all the time. Its
`in` and `out` ports are wires. A `reg` is a storage element that takes a
new value at each clock tick; everything outside `on clk { ... }` is
wiring that always shows the current result. Nothing "runs" line by line.

</div>

## Check it

`volt check` reads every source file and test file of the project and
reports errors: each `.volt` file under `src`, whether the top module uses
it or not, and each `*_test.volt` file that `volt test` would run,
subdirectories included (`build/` is skipped). It writes no files, so it
is quick enough to run after every change:

**Type this:**

```console
volt check
```

**You should see:**

```text,output
    Checking counter.volt
    Checking counter_test.volt
    Finished 0.00s
      Result 0 error(s), 0 warning(s)
       Next: volt build   (emit SystemVerilog)
```

## Build it

`volt build` writes SystemVerilog for the top module named in `Volt.toml`.
It reads the files that module uses and no others, so an error in a file
the design does not use shows up in `volt check`, not here:

**Type this:**

```console
volt build
```

**You should see:**

```text,output
   Compiling counter.volt
    Finished 0.00s (1 source file(s), 1 SV file(s))
     Output build\rtl\Counter.sv (37 lines)
       Next: volt run      (simulate)
             volt verify   (prove contracts)
```

`volt build` made a folder `build` inside `blinky`, and in it the folder
`rtl` with the file `Counter.sv`. Open `build/rtl/Counter.sv` in your
editor.

**You should see:**

```systemverilog,output
// This file was generated by Volt.
// Source:  counter.volt
// Module:  Counter
// Version: 0.1.0 (63a1c76)
//
// DO NOT EDIT — make changes in the source file instead.

`default_nettype none

module Counter (
    input  logic       clk,
    input  logic       rst,
    input  logic       enable,
    output logic [7:0] count,
    output logic       wrap
);

    logic [7:0] count_r;
    // next(count_r) — counter.volt:32
    wire [7:0] next_0 = (count_r == 8'd9) ? 8'd0 : (count_r + 8'd1);

    always_ff @(posedge clk) begin
        if (rst) begin
            count_r <= 8'd0;
        end else begin
            if (enable) begin
                count_r <= next_0;
            end
        end
    end

    assign count = count_r;
    assign wrap = enable && count_r == 8'd9;

endmodule

`default_nettype wire
```

Three things to notice:

- The module has a `rst` port that the source does not mention. A design
  with one clock gets a synchronous, active-high reset named `rst`, and
  every register goes back to its initial value (`= 0`) when `rst` is high.
- The call `next(count_r)` became a named wire, `next_0`, with a comment
  pointing back to the source line.
- The names of ports and registers are the names you wrote. The file is
  meant to be read, linted and fed to any SystemVerilog tool.
- The `Version` line names the Volt that wrote the file: the version and,
  in parentheses, the commit it was built from. Yours shows other letters
  and digits there.

<div class="box from-sv">

**Coming from SystemVerilog?** What the generator promises

Volt emits plain synthesizable SystemVerilog: `always_ff` with a
synchronous reset, `assign` for everything combinational,
`` `default_nettype none `` around each module, one file per module named
after it. The constants are folded (`MAX` became `8'd9`), and there are no
`function`s, `interface`s or `generate` blocks in the output. The
compiler's CI lints the SystemVerilog generated for its test designs and
examples with `verilator --lint-only -Wall` and expects zero warnings.

</div>

You are in the `blinky` folder. The next chapter uses a folder of its own.

Next: [a clock domain crossing error](cdc-error.md).
