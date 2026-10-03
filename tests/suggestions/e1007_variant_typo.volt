// suggestion: E1007
// A misspelt enum variant: the closest variant is suggested.
enum State : u2 {
    Idle,
    Busy,
}

module F {
    in  s : State
    out y : bool

    y = s == State::Bsy
}
