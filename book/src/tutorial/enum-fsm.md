# A state machine with enum

<div class="chapter-goal">

You will turn a sequence of steps into a finite state machine: an `enum` for the states, a `match` for the transitions, and the checks Volt adds for you. The compiler requires every state to be handled, and generates covers and a state-valid invariant for the machine.

</div>

> **This chapter is planned.** It will be written after feedback on the Tour and the counter chapter. Until then, these show the same topic:
>
> - [examples/uart_tx.volt (a four-state machine)](https://github.com/volt-hdl/volt/blob/main/examples/uart_tx.volt)
> - [ADR-0074: enums](https://github.com/volt-hdl/volt/blob/main/docs/adr/ADR-0074-enum-destegi.md)
> - [ADR-0066: generated FSM and counter contracts](https://github.com/volt-hdl/volt/blob/main/docs/adr/ADR-0066-otomatik-fsm-sayac-kontratlari.md)
