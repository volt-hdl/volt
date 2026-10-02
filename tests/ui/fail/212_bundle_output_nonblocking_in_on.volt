//~ E0020
// A bundle field that is an output of the module (here the flipped `ready`
// of an `in` bundle port) written with '<=' in an 'on' block: the same
// flip-flop with an empty reset branch, after bundle flattening.

struct port Bus {
    out valid : bool
    in  ready : bool
}

module BundleOutputNonblockingInOn {
    in  clk : clock
    in  x   : bool
    in  r   : Bus

    on clk { r.ready <= x && r.valid }
    //~^ ERROR output port 'r.ready' cannot be assigned with '<=' in an 'on' block
}
