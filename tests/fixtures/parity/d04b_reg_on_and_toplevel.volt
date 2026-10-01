// parity: E0019
// drivers: ok
// A register assigned with '=' outside an 'on' block is E0019 (ADR-0098),
// reported before driver analysis; until ADR-0098 this class was E4001.
module M {
    in  clk : clock
    in  a : u8
    in  b : u8
    out y : u8

    reg r : u8 = 0
    on clk { r <= a }
    r = b
    y = r
}
