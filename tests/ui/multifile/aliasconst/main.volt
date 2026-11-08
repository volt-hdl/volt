//~ PASS
// An imported const under an alias selects the bundle element: the
// parser resolves `SEL` like the resolver does (#139).
use lib::PICK as SEL;

module Top {
    in  rx : [Handshake<u8>; 2]
    out y  : u8
    rx[0].ready = true
    rx[1].ready = true
    y = rx[SEL].data
}
