//~ E1001
// ADR-0085: enum variants are named with their enum (ADR-0074); a bare
// variant name is an undefined name, never a catch-all binding. The
// fix-it writes 'State::Idle'.

enum State { Idle, Run }

module BareVariant {
    in  s   : State
    out y   : u8

    y = match s { Idle => 1, _ => 2 }
    //~^ ERROR undefined name: 'Idle'
}
