// parity: ok
enum Op : u7 { Load = 0b0000011, Store = 0b0100011, Alu = 0b0110011 }
module M {
    in  clk : clock
    in  st  : bool
    out code : u7

    reg op : Op = Op::Load
    on clk { if st { op <= Op::Store } else { op <= Op::Alu } }
    code = op as u7
}
