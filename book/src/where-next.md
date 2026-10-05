# Where to go next

<div class="chapter-goal">

Where to look when the book does not answer your question: the
explanations built into the compiler, the command reference, the design
decisions behind the language, and the example designs.

</div>

## Explanations built into `volt`

Every error and warning code has a long explanation:

**Type this:**

```console
volt explain E3001
volt explain --list          # every code
volt explain E3001 --lang=tr # in Turkish
```

Topics cover larger subjects: `getting-started`, `domains`, `contracts`,
`stdlib`, `verify-setup`, `simulation-setup`, `waveforms`
(`volt explain --topics` lists them).

## Commands

`volt --help` lists the commands, and `volt <command> --help` their
options. The full contract of every command, its exit codes and its
output format is in
[docs/spec/cli-contract.md](https://github.com/volt-hdl/volt/blob/main/docs/spec/cli-contract.md).

## Design decisions

Each language feature was decided in an architecture decision record
(ADR). The index, with a "Buradan başla" (start here) reading list, is
[docs/adr/README.md](https://github.com/volt-hdl/volt/blob/main/docs/adr/README.md).
The ADRs are written in Turkish; the language specification in
[docs/spec/](https://github.com/volt-hdl/volt/tree/main/docs/spec) is in
English.

## What is missing and what is planned

[Known limitations](limitations.md) lists what Volt does not do yet, with
workarounds. The
[roadmap](https://github.com/volt-hdl/volt/blob/main/docs/roadmap.md)
lists the planned work and the design decision behind each item.

## Examples and reference

- [examples/README.md](https://github.com/volt-hdl/volt/blob/main/examples/README.md):
  a UART, I²C, a VGA controller, a RISC-V core that runs C, and more, with
  their test and verification results.
- [docs/stdlib.md](https://github.com/volt-hdl/volt/blob/main/docs/stdlib.md):
  the built-in components (FIFOs, RAMs, synchronizers, arbiters).
- `volt new --list`: project templates.
