# Systolic array example — regular structures (ADR-0056)

`pe_array.volt` is a 4x4 output-stationary systolic array: 16 identical
processing elements, each multiplying the operand arriving from the west
by the one arriving from the north, accumulating the product and
forwarding both operands one hop a cycle later. Fed with a skewed A and B
it computes C = A * B in place.

The point of the example is not the arithmetic but the *shape*: the grid
is written once, as two nested compile-time `for` loops, in 96 lines.

```volt
for y in 0..N {
    for x in 0..N {
        let pe = Pe {
            clk: clk, clr: clr,
            a_in: a_link[y * (N + 1) + x],
            b_in: b_link[y * N + x],
        }
        a_link[y * (N + 1) + x + 1] = pe.a_out
        b_link[(y + 1) * N + x]     = pe.b_out
        c[y * N + x]                = pe.c
    }
}
```

What the compiler does with it (ADR-0056):

- the loops are unrolled in the parser, before name resolution, so each
  iteration is an ordinary instance named `pe_<y>_<x>` and every later
  pass (types, drivers, clock domains, contracts, SV) sees hand-written
  code;
- the loop variables become literals and the index arithmetic folds
  (`(1 + 1) * 4 + 2` -> `10`), so the generated SV reads
  `assign c[160 +: 16] = pe_2_2_c;`;
- array-typed ports and wires (`[i8; N]`, `[i16; N * N]`) are packed
  vectors in SV (`logic [31:0] a`, `logic [255:0] c`) with `+:`
  part-selects for the elements -- Yosys rejects unpacked array ports,
  packed vectors keep both Verilator and the formal flow happy;
- a diagnostic inside the loop body points at the source line and adds
  `= note: in the unrolled 'for' iteration y = 2, x = 1`.

## Build, lint, verify

```
volt build examples/systolic/pe_array.volt        # build/rtl/Pe.sv, PeArray.sv
docker run --rm -v "$PWD/build/rtl:/work" -w /work \
    verilator/verilator:latest --lint-only -Wall --top-module PeArray PeArray.sv Pe.sv
volt verify -j 4 --mode bmc   --depth 8  examples/systolic/pe_array.volt
volt verify -j 4 --mode prove --depth 3 --engine boolector examples/systolic/pe_array.volt
volt verify -j 4 --mode cover --depth 12 examples/systolic/pe_array.volt
```

| Metric | Value |
|---|---|
| Source | 96 lines (a hand-written 4x4 would be ~400) |
| Generated SV | Pe 49 lines, PeArray 336 lines, 16 instances |
| Verilator `-Wall` | clean |
| Contracts | 5 properties (3 in `Pe`, 2 in `PeArray`): bmc 8, prove 3, cover 12 all pass |

The contracts are deliberately small: `prev(clr) -> c == 0` (the
accumulator empties the cycle after `clr`), `prev(a_in) == a_out` (the
operand really is delayed by exactly one hop) and `cover: c != 0`. The
array restates the first one on its corner elements; contracts only see
ports and registers, so the link wires cannot be named in them.
