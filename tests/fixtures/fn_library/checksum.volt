// Output net fixture (ADR-0079, ADR-0042 addendum): a library file with
// no module — pure fns, a const and a struct. `volt build` writes no
// SystemVerilog for it (a module-less .sv is rejected by Verilator and
// Yosys `--top-module`); `sum_top.volt` uses it.

pub const SEED : u8 = 0x5A

pub struct Pair {
    hi : u8
    lo : u8
}

pub fn fold(x: u16) -> u8 {
    (x[15:8] as u8) ^ (x[7:0] as u8) ^ SEED
}

pub fn add_sat(a: u8, b: u8) -> u8 {
    let s : u9 = (a as u9) + (b as u9)
    if s > 255 { 255 } else { s[7:0] as u8 }
}
