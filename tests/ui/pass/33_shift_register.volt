// Built-in ShiftRegister primitive (ADR-0029) -- serial-to-parallel
// conversion: each shift_en pushes data_in into the youngest stage,
// data_out is the oldest stage, taps exposes all LEN stages flattened
// as bits<LEN * width(T)>.
module SerialToParallel {
    in  clk    : clock
    in  bit_in : bool
    in  strobe : bool
    out oldest : bool
    out window : bits<8>

    let sr = ShiftRegister<bool, 8> {
        clk: clk,
        data_in: bit_in,
        shift_en: strobe,
    }

    oldest = sr.data_out
    window = sr.taps
}
