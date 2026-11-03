# A state machine with enum

<div class="chapter-goal">

You will turn a sequence of steps into a finite state machine: an `enum` for the states, a `match` for the transitions, and the checks Volt adds for you. The compiler requires every state to be handled, and generates a cover for every transition, so a test run shows which transitions your tests never took.

</div>

<div class="before-you-start">

**Before you start**

- **Window:** a terminal ([how to open one](../setup.md#open-a-terminal))
  and your editor.
- **Folder:** your `volt-projects` folder. This chapter makes a new folder
  there, `fsm`. **Type this:**

  ```console
  cd ~/volt-projects
  ```

- **Running:** for the sections from [Test it](#test-it) on, Docker
  Desktop with **Engine running** in its window
  ([Setup](../setup.md#install-and-start-docker-desktop) shows how):
  `volt test` runs the Verilator simulator in Docker. If Verilator is
  installed on your computer, Docker is not needed; `volt doctor` says
  which one `volt test` uses.

</div>

The output on this page was recorded with Volt 0.1.0 in PowerShell on
Windows 11, with Docker Desktop running Verilator. On macOS and Linux,
paths are written with `/` instead of `\`.

## A button that bounces

A push button does not switch cleanly. When you press it, the metal
contacts bounce for a few milliseconds, and the input goes on and off
many times before it stays on. A design that counts presses would count
every bounce. A *debouncer* waits until the input has been steady for a
while before it believes it.

The debouncer is a *state machine*: at any moment it is in one of a few
named situations, its *states*, and at each clock edge it decides from
its inputs whether to move to another state. Ours has four:

| State | Means | Moves on |
|---|---|---|
| `Up` | the button is released | to `GoingDown` when the input is on |
| `GoingDown` | the input is on, but not for long enough yet | to `Down` after a steady stretch, back to `Up` if the input drops |
| `Down` | the button is pressed | to `GoingUp` when the input is off |
| `GoingUp` | the input is off, but not for long enough yet | to `Up` after a steady stretch, back to `Down` if the input comes back |

Make a folder `fsm` and go into it:

**Type this:**

```console
mkdir fsm
```

**Type this:**

```console
cd fsm
```

## States as an enum

In this folder, create the file `debounce.volt` (as
[Setup](../setup.md#create-a-file) shows). This version has a mistake on
purpose: one state is missing from the `match`.

**Create this file:** `debounce.volt`

```volt,file=debounce.volt,should_fail=E0014
// Cleans up a bouncing push button.

const STABLE : u4 = 4

enum Button { Up, GoingDown, Down, GoingUp }

pub module Debouncer {
    in  clk     : clock
    in  raw     : bool
    out pressed : bool
    out click   : bool

    reg state_r : Button = Button::Up
    reg count_r : u4     = 0

    on clk {
        match state_r {
            Button::Up => {
                if raw {
                    count_r <= 0
                    state_r <= Button::GoingDown
                }
            }
            Button::GoingDown => {
                if !raw {
                    state_r <= Button::Up
                } else if count_r == STABLE - 1 {
                    state_r <= Button::Down
                } else {
                    count_r <= count_r + 1
                }
            }
            Button::Down => {
                if !raw {
                    count_r <= 0
                    state_r <= Button::GoingUp
                }
            }
        }
    }

    pressed = state_r == Button::Down || state_r == Button::GoingUp
    click   = state_r == Button::GoingDown && raw && count_r == STABLE - 1
}
```

What is new compared with the [counter](counter.md):

- `const STABLE : u4 = 4` names a constant: how many clock cycles the
  input must stay the same. A real button needs a few milliseconds,
  hundreds of thousands of cycles; 4 keeps the tests short.
- `enum Button { Up, GoingDown, Down, GoingUp }` declares a type whose
  values are the four states. `Button::Up` is one of them.
- `reg state_r : Button = Button::Up` is the *state register*. It holds
  one of the four values, and starts in `Up` after reset.
- `match state_r { ... }` picks the arm for the current state. Each arm,
  `Button::Up => { ... }`, says what happens at the clock edge while the
  machine is in that state. A state that does not write `state_r` stays
  where it is.
- `count_r` counts how long the input has been steady. Each "going"
  state sets it to 0 on the way in and counts up; when it reaches
  `STABLE - 1`, the machine moves on.
- `pressed` and `click` are outputs computed from the state. `==`
  compares two enum values. `click` is true for the one cycle in which
  `GoingDown` decides that the press is real: a single pulse per press,
  however long the button is held.

<div class="box new-to-hw">

**New to hardware?** A state machine is a register and a decision

In software a sequence of steps is code that runs from top to bottom. In
hardware nothing runs from top to bottom: at every clock edge all the
logic is evaluated at once. To do one thing after another, the circuit
keeps a register that says which step it is at, and logic that picks the
next step from that register and the inputs. That pair is a *finite
state machine* (FSM). Naming the steps with an `enum` makes the register
readable: `Button::GoingDown` instead of the number 1.

</div>

## Every state needs an arm

Check the file:

**Type this:**

```console
volt check debounce.volt
```

**You should see:**

```text,output
    Checking debounce.volt
error[E0014]: 'match' on enum 'Button' does not cover every variant: missing Button::GoingUp
   ┌─ debounce.volt:17:9
   │  
17 │ ╭         match state_r {
18 │ │             Button::Up => {
19 │ │                 if raw {
20 │ │                     count_r <= 0
   · │
38 │ │             }
39 │ │         }
   │ ╰─────────^ not every variant is covered
   │  
   = note: an enum match that names every variant needs no '_' arm; its last arm also takes the codes no variant uses
   = help: add an arm for each missing variant (Button::GoingUp => { }) or a final '_ => { }' arm
   = for more: volt explain E0014


    Finished 0.01s
      Result 1 error(s), 0 warning(s)
```

The `match` handles three of the four states, and error `E0014` names
the one that is missing. In SystemVerilog, a `case` without that branch
would compile, and the machine would freeze in `GoingUp` the earliest
time it got there. Volt rejects it at compile time.

The help line offers two fixes: an arm for `Button::GoingUp`, or a final
`_ => { }` arm that catches every state not named above it. Prefer the
arm: with `_`, a state you add to the enum later is silently caught by
it, and the compiler can no longer tell you that you forgot it. Add the
missing arm:

**Replace this file:** `debounce.volt`

```volt,file=debounce.volt
// Cleans up a bouncing push button.

const STABLE : u4 = 4

enum Button { Up, GoingDown, Down, GoingUp }

pub module Debouncer {
    in  clk     : clock
    in  raw     : bool
    out pressed : bool
    out click   : bool

    reg state_r : Button = Button::Up
    reg count_r : u4     = 0

    on clk {
        match state_r {
            Button::Up => {
                if raw {
                    count_r <= 0
                    state_r <= Button::GoingDown
                }
            }
            Button::GoingDown => {
                if !raw {
                    state_r <= Button::Up
                } else if count_r == STABLE - 1 {
                    state_r <= Button::Down
                } else {
                    count_r <= count_r + 1
                }
            }
            Button::Down => {
                if !raw {
                    count_r <= 0
                    state_r <= Button::GoingUp
                }
            }
            Button::GoingUp => {
                if raw {
                    state_r <= Button::Down
                } else if count_r == STABLE - 1 {
                    state_r <= Button::Up
                } else {
                    count_r <= count_r + 1
                }
            }
        }
    }

    pressed = state_r == Button::Down || state_r == Button::GoingUp
    click   = state_r == Button::GoingDown && raw && count_r == STABLE - 1
}
```

**Type this:**

```console
volt check debounce.volt
```

**You should see:**

```text,output
    Checking debounce.volt
    Finished 0.02s
      Result 0 error(s), 0 warning(s)
       Next: volt build debounce.volt   (emit SystemVerilog)
```

## Test it

Create `debounce_test.volt` next to `debounce.volt`:

**Create this file:** `debounce_test.volt`

```volt,file=debounce_test.volt
// Simulation tests for Debouncer.

test "a steady press is seen once" {
    let dut = Debouncer { };
    dut.raw = true;
    step(4);
    assert_false(dut.pressed);
    assert_true(dut.click);
    step(1);
    assert_true(dut.pressed);
    assert_false(dut.click);
    step(10);
    assert_true(dut.pressed);
    assert_false(dut.click);
}

test "a short bounce is ignored" {
    let dut = Debouncer { };
    dut.raw = true;
    step(2);
    dut.raw = false;
    step(1);
    dut.raw = true;
    step(2);
    dut.raw = false;
    step(5);
    assert_false(dut.pressed);
}

test "a release needs a steady low too" {
    let dut = Debouncer { };
    dut.raw = true;
    step(5);
    dut.raw = false;
    step(2);
    assert_true(dut.pressed);
    step(3);
    assert_false(dut.pressed);
}
```

The earliest test holds the button down. After 4 cycles the machine has
been in `GoingDown` long enough: `click` is on for that cycle, and from
the next one on the button counts as pressed. The second test presses
for two cycles, lets go, presses again and lets go: no stretch is long
enough, so `pressed` never turns on. The third test releases the button
and checks that `pressed` stays on until the release has been steady.

Check that Docker Desktop says **Engine running**, then run the tests:

**Type this:**

```console
volt test debounce_test.volt
```

**You should see:**

```text,output
   Compiling debounce_test.volt
running 3 tests
note: Verilator not found locally; running it in Docker (verilator/verilator:v5.052)
test a_steady_press_is_seen_once ... ok
test a_short_bounce_is_ignored ... ok
test a_release_needs_a_steady_low_too ... ok

cover summary:
  Debouncer.cov_0 (debounce.volt:21, auto FSM transition)  hit 4 times
  Debouncer.cov_1 (debounce.volt:26, auto FSM transition)  hit 2 times
  Debouncer.cov_2 (debounce.volt:28, auto FSM transition)  hit 2 times
  Debouncer.cov_3 (debounce.volt:36, auto FSM transition)  hit 1 time
  Debouncer.cov_4 (debounce.volt:41, auto FSM transition)  NEVER HIT
  Debouncer.cov_5 (debounce.volt:43, auto FSM transition)  hit 1 time
  Debouncer.cov_6 (debounce.volt:27, auto counter wrap)    hit 16 times

test result: ok. 3 passed; 0 failed
```

The three tests pass. Below them is a *cover summary* with seven lines,
although the design has no `cover` line. Volt found the state machine in
the `match` and generated a cover for every transition the code can
make, plus one for the counter. Each line names where the transition is
written, and how many cycles of the test run took it:

- `debounce.volt:21` is the line `state_r <= Button::GoingDown` in the
  `Up` arm: the transition from `Up` to `GoingDown`. It was taken 4
  times, at least once in each test.
- `debounce.volt:41` is the line `state_r <= Button::Down` in the
  `GoingUp` arm: a bounce while the button is being released. `NEVER
  HIT` means that no test made that transition, so it was never tested.
- `auto counter wrap` counts the cycles in which `count_r` holds its
  largest value, `STABLE - 1`.

## A transition no test took

Add a test for the bounce on release: hold the button, let go for a
single cycle, press again. Replace the test file:

**Replace this file:** `debounce_test.volt`

```volt,file=debounce_test.volt
// Simulation tests for Debouncer.

test "a steady press is seen once" {
    let dut = Debouncer { };
    dut.raw = true;
    step(4);
    assert_false(dut.pressed);
    assert_true(dut.click);
    step(1);
    assert_true(dut.pressed);
    assert_false(dut.click);
    step(10);
    assert_true(dut.pressed);
    assert_false(dut.click);
}

test "a short bounce is ignored" {
    let dut = Debouncer { };
    dut.raw = true;
    step(2);
    dut.raw = false;
    step(1);
    dut.raw = true;
    step(2);
    dut.raw = false;
    step(5);
    assert_false(dut.pressed);
}

test "a release needs a steady low too" {
    let dut = Debouncer { };
    dut.raw = true;
    step(5);
    dut.raw = false;
    step(2);
    assert_true(dut.pressed);
    step(3);
    assert_false(dut.pressed);
}

test "a bounce on release keeps the press" {
    let dut = Debouncer { };
    dut.raw = true;
    step(5);
    dut.raw = false;
    step(1);
    dut.raw = true;
    step(5);
    assert_true(dut.pressed);
    assert_false(dut.click);
}
```

**Type this:**

```console
volt test debounce_test.volt
```

**You should see:**

```text,output
   Compiling debounce_test.volt
running 4 tests
note: Verilator not found locally; running it in Docker (verilator/verilator:v5.052)
test a_steady_press_is_seen_once ... ok
test a_short_bounce_is_ignored ... ok
test a_release_needs_a_steady_low_too ... ok
test a_bounce_on_release_keeps_the_press ... ok

cover summary:
  Debouncer.cov_0 (debounce.volt:21, auto FSM transition)  hit 5 times
  Debouncer.cov_1 (debounce.volt:26, auto FSM transition)  hit 2 times
  Debouncer.cov_2 (debounce.volt:28, auto FSM transition)  hit 3 times
  Debouncer.cov_3 (debounce.volt:36, auto FSM transition)  hit 2 times
  Debouncer.cov_4 (debounce.volt:41, auto FSM transition)  hit 1 time
  Debouncer.cov_5 (debounce.volt:43, auto FSM transition)  hit 1 time
  Debouncer.cov_6 (debounce.volt:27, auto counter wrap)    hit 18 times

test result: ok. 4 passed; 0 failed
```

Every transition is now taken by at least one test. The last assert also
checks that the bounce did not produce a second `click`.

<div class="box new-to-hw">

**New to hardware?** Why count transitions

A test that passes tells you that the design did what the test expected.
It does not tell you what the test left out. In a state machine the rare
paths, a bounce at the wrong moment or an error in the middle of a
transfer, are the ones that break in the lab, and they are easy to
forget. Counting the transitions turns "did I test everything?" into a
list you can read.

</div>

## The SystemVerilog

**Type this:**

```console
volt build debounce.volt
```

**You should see:**

```text,output
   Compiling debounce.volt
    Finished 0.01s (1 source file(s), 1 SV file(s))
     Output build\rtl\Debouncer.sv (72 lines)
       Next: volt run debounce.volt      (simulate)
             volt verify debounce.volt   (prove contracts)
```

Open `build/rtl/Debouncer.sv` (in the `fsm` folder) in your editor. This
is a part of it (output trimmed):

**You should see:**

```systemverilog,output
    localparam logic [1:0] Button_Up        = 2'd0;
    localparam logic [1:0] Button_GoingDown = 2'd1;
    localparam logic [1:0] Button_Down      = 2'd2;
    localparam logic [1:0] Button_GoingUp   = 2'd3;

    logic [1:0] state_r;  // Button
...
            case (state_r)
                Button_Up: begin
                    if (raw) begin
                        count_r <= 4'd0;
                        state_r <= Button_GoingDown;
                    end
                end
...
                default: begin // Button_GoingUp
```

Each state becomes a `localparam` with the state's name, numbered in the
order of the `enum`, and the `match` becomes a `case`. The arm of the
last state is the `default` branch, with its name in a comment: a value
that names no state, which cannot happen here, is handled like
`GoingUp`.

<div class="box from-sv">

**Coming from SystemVerilog?** What the compiler checks for you

In SystemVerilog an FSM is usually a `typedef enum`, a `case` with a
`default`, and a lint rule or a review to catch a missing state. Volt
makes the missing arm a compile error (`E0014`) and writes no
`typedef`: the states are `localparam`s, so the output reads the same in
every tool. The generated transition covers are SVA `cover property`
statements in `volt build --emit=sva`, and `volt test` counts them in
simulation. When the states do not fill the bits of the register, for
example three states in two bits, Volt also generates an invariant that
the register holds a valid state; the four states here use all four
codes. The waveform of a failed test shows the state names, as in
[Tests and waveforms](../tour/tests-and-waveforms.md).

</div>

You are in the `fsm` folder.

## What we learned

- `enum Name { A, B, C }` declares the states; a `reg` of that type,
  `reg state_r : Name = Name::A`, is the state register, with a state as
  its reset value.
- `match` on the state register gives each state an arm; an arm that
  does not write the register keeps the state.
- A `match` must handle every state: a missing one is error `E0014`.
  Prefer an arm per state over a `_` arm.
- `const` names a number, such as how long an input must stay steady.
- Volt generates a cover for every transition, and `volt test` lists
  them: `NEVER HIT` points at a transition no test took.

Next: [two clocks and CDC](two-clocks.md) moves data between parts of a
design that run on different clocks.
