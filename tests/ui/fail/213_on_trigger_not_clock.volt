//~ E3002
// ADR-0099: an 'on' block triggered by a data signal. Before, only the
// SystemVerilog generator stopped it ("compiler bug" note) and the editor
// showed nothing; now the domain check reports it.

module Toggle {
    in  _clk : clock
    in  d    : bool
    out q    : bool

    reg r : bool = false
    on d {
    //~^ ERROR E3002
        r <= !r
    }
    q = r
}
