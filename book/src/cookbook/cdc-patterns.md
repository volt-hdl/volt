# CDC patterns

<div class="chapter-goal">

Pick the right crossing for a single bit, a pulse, a multi-bit value and a data stream. Each pattern shows the Volt code, what the compiler checks, and the SystemVerilog it becomes.

</div>

> **This chapter is planned.** It will be written after feedback on the Tour and the counter chapter. Until then, these show the same topic:
>
> - [Two clocks and CDC](../tutorial/two-clocks.md): `sync()` for a bit, why a multi-bit `sync()` is warning `W3003`, `AsyncFifo` for a stream
> - [A clock domain crossing error](../tour/cdc-error.md): error `E3001` and its fix
> - [templates/cdc (toggle pulse crossing)](https://github.com/volt-hdl/volt/tree/main/templates/cdc)
> - `volt explain E3001`
> - `volt explain domains`
