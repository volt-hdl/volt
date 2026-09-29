# Introduction

Volt is a hardware description language. You describe digital circuits in
a Rust-like syntax, and the compiler writes readable SystemVerilog for your
FPGA or ASIC tools. Clock domains are part of the type system: a signal
that crosses from one clock domain to another without a synchronizer is a
compile error.

This book teaches Volt by building things. It has three parts:

- **Part I, the Tour**, takes about 15 minutes. You install Volt, create a
  project, meet a clock domain crossing error, fix it, run tests and look
  at a waveform.
- **Part II, the Tutorial**, builds one design per chapter: a counter, a
  state machine, a UART, a design with two clocks, and more.
- **Part III, the Cookbook**, is a set of recipes for common hardware
  tasks: FIFOs, crossings, reset synchronizers, register maps, pipelines.

> **Status.** Volt is an early-stage project (version 0.1). The Tour and
> the counter chapter are written; the other chapters list what they will
> cover and point to working examples in the repository.

## Two kinds of readers

This book is written for two groups at once:

- people who are **new to hardware** and have written software before, and
- **FPGA and ASIC engineers** who know SystemVerilog.

The main text is for both. It explains what to type and what happens. Two
kinds of side boxes add detail for one group. You can skip either kind;
the main text reads the same without them.

<div class="box new-to-hw">

**New to hardware?**

Boxes like this one explain a hardware idea the main text uses: a clock, a
flip-flop, a reset. If you have designed digital logic before, skip them.

</div>

<div class="box from-sv">

**Coming from SystemVerilog?**

Boxes like this one show the SystemVerilog you would write for the same
thing, what Volt does differently, and why. If SystemVerilog is new to you,
skip them.

</div>

## Code in this book

Every Volt example in this book is checked by `volt check` in continuous
integration, and the tests in it run in Verilator. Some examples are
wrong on purpose, to show an error; the text says so each time. Command
output is copied from real runs; the chapter says where it ran.

The source of this book is in the
[`book/`](https://github.com/volt-hdl/volt/tree/main/book) directory of the
Volt repository. Corrections are welcome.
