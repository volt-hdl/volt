//~ E2009
// ADR-0074: a number may hold a code that is no variant, so 'uN as Enum'
// is rejected. Decode explicitly with a match and choose what the
// invalid codes become.
enum State { Idle, Run, Stop }

module M {
    in  clk : clock
    in  raw : u2
    out run : bool

    reg s : State = State::Idle

    on clk {
        s <= raw as State
//~^ ERROR a number may hold a code that is no variant
    }

    run = s == State::Run
}
