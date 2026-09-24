// ADR-0074: an enum-typed state register. The match names every
// variant, so no '_' arm is needed; the last arm becomes the SV
// 'default' (it also takes code 3, which no variant uses). The
// generated contracts include the state-valid invariant (F1): 3
// variants in 2 bits leave one code unused.
enum State { Idle, Data, Stop }
enum Phase { Wait, Go, Hold }

module EnumFsm {
    in  clk   : clock
    in  start : bool
    in  last  : bool
    out busy  : bool
    out phase : Phase

    reg state_r : State = State::Idle
    reg phase_r : Phase = Phase::Wait
    wire next   : Phase

    on clk {
        match state_r {
            State::Idle => { if start { state_r <= State::Data } }
            State::Data => { if last { state_r <= State::Stop } }
            State::Stop => { state_r <= State::Idle }
        }
        phase_r <= next
    }

    comb {
        match phase_r {
            Phase::Wait => { next = if start { Phase::Go } else { Phase::Wait } }
            Phase::Go => { next = Phase::Hold }
            Phase::Hold => { next = Phase::Wait }
        }
    }

    busy  = state_r != State::Idle
    phase = phase_r
}

test "enum fsm walks through its states" {
    let dut = EnumFsm { };
    dut.start = true;
    dut.last = false;
    step(1);
    assert_eq(dut.phase, Phase::Go);
    dut.start = false;
    step(1);
    assert_eq(dut.phase, Phase::Hold);
    assert_true(dut.busy);
    step(1);
    assert_eq(dut.phase, Phase::Wait);
    assert_ne(dut.phase, Phase::Hold);
}
