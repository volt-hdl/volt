// ADR-0081: kontratta fn çağrısı — saf olduğu için anlamı bağlamdan
// bağımsız; kontratta tel üretilmez (ikame kipi), --emit=sva RTL'yi
// değiştirmez.

fn is_aligned(addr: u32) -> bool {
    (addr & 3) == 0
}

fn next(addr: u32) -> u32 {
    addr + 4
}

module FnContract {
    in  clk  : clock
    out addr : u32

    reg a_r : u32 = 0

    invariant: is_aligned(a_r)
    cover:     next(a_r) == 16

    on clk {
        a_r <= next(a_r)
    }
    addr = a_r
}
