// Trust levels (ADR-0052): the fourth dimension of a domain. Information
// may only flow to the same or a higher level; constants fit everywhere.
// Every flow below is legal, so this file compiles without an E3009.
// `Debug` and `Secure` carry no clock port of the module: they do not
// open a second clock domain (K11), they only classify the signals.
domain Secure {
    clock       = posedge
    reset       = sync active_high
    trust_level = secret
}

domain Internal {
    clock       = posedge
    reset       = sync active_high
    trust_level = confidential
}

domain Debug {
    clock       = posedge
    reset       = sync active_high
    trust_level = public
}

module TrustFlows {
    in  clk        : clock
    in  key        : u8   @Secure
    in  cfg        : u8   @Internal
    in  ctl        : bool @Debug
    out secret_out : u8   @Secure
    out cfg_out    : u8   @Internal
    out public_out : bool @Debug

    reg acc_r : u8 = 0            // unclassified: takes the highest level written

    on clk {
        if ctl {
            acc_r <= key ^ cfg    // secret ⊔ confidential = secret
        }
    }

    let mixed = acc_r & cfg       // still secret
    secret_out = mixed            // secret → secret: same level, fine
    cfg_out    = cfg + 1          // confidential → confidential
    public_out = ctl && true      // public → public; the constant fits everywhere
}

// A public source may always flow upwards into a secret sink (a
// clockless module: the annotations only classify, K11).
module Upward {
    in  ctl  : bool @Debug
    out gate : bool @Secure

    gate = !ctl                   // public → secret: free
}
