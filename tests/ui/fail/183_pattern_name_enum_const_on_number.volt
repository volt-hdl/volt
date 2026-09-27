//~ E2003
// ADR-0085: an enum-typed const cannot match a number (same rule as the
// path pattern State::Run, ADR-0074).

enum State { Idle, Run }
const START : State = State::Run

module EnumConstOnNumber {
    in  x : u8
    out y : u8

    y = match x { START => 1, _ => 2 }
    //~^ ERROR pattern of enum 'State' cannot match a value of type 'u8'
}
