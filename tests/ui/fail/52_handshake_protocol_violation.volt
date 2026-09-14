//~ E4007
// Handshake protocol violation (ADR-0050): the producer derives 'valid'
// from 'ready' through a combinational path ('let go' in between). A
// producer must raise valid without waiting for ready — otherwise two
// well-behaved parties deadlock. The decision belongs in a register.
module Producer {
    in  clk  : clock
    in  have : bool
    out tx   : Handshake<u8>

    reg count : u8 = 0
    on clk {
        if tx.fired {
            count <= count + 1
        }
    }
    let go : bool = have && tx.ready
    tx.valid = go
    //~^ ERROR E4007
    tx.data  = count
}
