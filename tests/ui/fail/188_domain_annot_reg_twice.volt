//~ E0001
// ADR-0088: 'reg(clk)' and '@Domain' both name the register's domain;
// one of them is enough.

domain Slow {}

module RegTwice {
    in  clk : clock @Slow
    in  a   : bool  @Slow
    out y   : bool  @Slow

    reg(clk) r : bool @Slow = false
    //~^ ERROR register 'r' has two domain annotations: 'reg(clk)' and '@Slow'
    on clk { r <= a }
    y = r
}
