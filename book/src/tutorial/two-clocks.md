# Two clocks and CDC

<div class="chapter-goal">

You will declare clock domains, see why a signal cannot simply cross from one to another, and use `sync()` and `AsyncFifo` to cross safely. The chapter also covers resets in a design with several clocks.

</div>

> **This chapter is planned.** It will be written after feedback on the Tour and the counter chapter. Until then, these show the same topic:
>
> - [volt new --template cdc](https://github.com/volt-hdl/volt/tree/main/templates/cdc)
> - [ADR-0002: domain semantics](https://github.com/volt-hdl/volt/blob/main/docs/adr/ADR-0002-domain-semantigi.md)
> - [ADR-0065: reset domain crossings](https://github.com/volt-hdl/volt/blob/main/docs/adr/ADR-0065-rdc-ve-hedefli-sdc.md)
> - `volt explain domains`
