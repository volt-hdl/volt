# Wrapping SystemVerilog with extern

<div class="chapter-goal">

Use an existing SystemVerilog module from Volt: declare it with `extern`, give its ports clock domains, and ship its source with `@source` so that `volt test` can simulate it.

</div>

> **This chapter is planned.** It will be written after feedback on the Tour and the counter chapter. Until then, these show the same topic:
>
> - [Using Volt in an existing project](existing-project.md): an `extern` module with `@source`, checked, tested and built
> - [ADR-0047: domains on extern modules](https://github.com/volt-hdl/volt/blob/main/docs/adr/ADR-0047-extern-domain-anotasyonu.md)
> - [ADR-0076: extern sources](https://github.com/volt-hdl/volt/blob/main/docs/adr/ADR-0076-extern-kaynaklari.md)
> - [tests/fixtures/extern_source/](https://github.com/volt-hdl/volt/tree/main/tests/fixtures/extern_source)
