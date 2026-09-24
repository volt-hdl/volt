//~ E1003
// ADR-0074: variants map to '<Enum>_<Variant>' localparams; a signal of
// the same name in the module would collide in SystemVerilog.
enum State { Idle, Run }

module Clash {
//~^ ERROR clashes with a signal of this module
    in  clk : clock
    in  go  : bool
    out y   : bool

    reg s : State = State::Idle
    wire State_Run : bool
    State_Run = go

    on clk { if State_Run { s <= State::Run } }

    y = s == State::Run
}
