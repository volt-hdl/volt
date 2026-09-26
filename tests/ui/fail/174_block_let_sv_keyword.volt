//~ E1013
// ADR-0083 Karar 11 / ADR-0078: blok let'i SV'ye yerel değişken adıyla
// iner — SystemVerilog anahtar sözcüğü olamaz.

module LetKeyword {
    in  clk : clock
    in  a   : u8
    out q   : u8

    reg r : u8 = 0
    on clk {
        let begin = a + 1
        //~^ ERROR 'begin' is a SystemVerilog keyword
        r <= begin
    }
    q = r
}
