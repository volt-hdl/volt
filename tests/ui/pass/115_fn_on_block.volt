// ADR-0081: on bloğunda çağrı. Argümanları modül düzeyi adlarsa tel kipi
// (tel always_ff'ten önce, koşulsuz); blok içi for'da döngü değişkenine
// başvuran çağrı ikame kipinde açılır.

fn step(pc: u32, off: u32) -> u32 {
    pc + (off << 2)
}

module FnOnBlock {
    in  clk  : clock
    in  off  : u32
    in  ens  : [bool; 2]
    out pc   : u32

    reg pc_r : u32 = 0

    on clk {
        pc_r <= step(pc_r, off)
        for i in 0..2 {
            if ens[i] {
                pc_r <= step(pc_r, i)
            }
        }
    }
    pc = pc_r
}
