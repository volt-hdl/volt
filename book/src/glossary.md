# Glossary

<div class="chapter-goal">

Short definitions of the hardware and Volt terms used in this book. The
list grows with the book; the specification's terms are in
[docs/spec/GLOSSARY.md](https://github.com/volt-hdl/volt/blob/main/docs/spec/GLOSSARY.md).

</div>

**Clock.** A signal that switches between 0 and 1 at a steady rate.
Registers change their value at its rising edge.

**Clock domain.** A clock together with the registers it drives and the
signals computed from them. In Volt a `domain` also fixes the clock edge
and the reset style, and every signal belongs to one domain.

**CDC (clock domain crossing).** A signal from one clock domain read in
another. Without a synchronizer it can cause metastability; Volt reports it
as `E3001`.

**Combinational logic.** Logic whose output depends on its current inputs
and nothing else: gates and wires, no storage. In Volt, assignments with
`=` outside an `on` block, `let`, and `fn` calls.

**Contract.** A rule about a design written in the design: `invariant`,
`requires`, `ensures`, `cover`. `volt verify` proves it; `volt test`
checks it on every simulated cycle.

**Cover.** A contract that asks "can this happen?". Simulation counts how
often it was reached; formal verification searches for a way to reach it.

**Enum.** A type with a fixed set of named values, such as the states of a
state machine. Volt encodes the names as numbers in the generated
SystemVerilog and shows the names in waveforms.

**Flip-flop.** A storage element for one bit that takes a new value at a
clock edge.

**Formal verification.** Proving a property for every possible input
sequence with a solver, instead of trying some sequences in simulation.
Volt uses SymbiYosys.

**Metastability.** The unstable state of a flip-flop whose input changed
right at the clock edge. It settles to 0 or 1 after an unpredictable time.

**Module.** A circuit with ports. Modules can contain other modules.

**Port.** A named input (`in`) or output (`out`) of a module, with a type.

**Register.** A group of flip-flops holding a value, declared with `reg`
and updated in an `on` block.

**Reset.** An input that puts registers back to a known value. A
*synchronous* reset acts at a clock edge; an *asynchronous* one acts at
once.

**Simulation.** Computing a design's signals cycle by cycle for given
inputs. Volt uses Verilator.

**Synchronizer.** Two or more flip-flops in a row on the receiving clock,
which give a crossing signal time to settle. In Volt: `sync()`.

**SystemVerilog.** The hardware description language that Volt generates
and that FPGA and ASIC tools read.

**Testbench.** Code that drives a design's inputs and checks its outputs
in simulation. Volt generates it from `test` blocks.

**Waveform.** A recording of signals over time, usually a `.vcd` file,
viewed with a tool such as GTKWave.

**Width.** The number of bits of a value: `u8` has 8, `bool` has 1.
