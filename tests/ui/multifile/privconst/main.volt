//~ E1001
use lib::WIDTH;

module Top {
    in  rx : [Handshake<u8>; 2]
    out y  : u8
    rx[0].ready = true
    rx[1].ready = true
    y = rx[PICK].data
    //~^ ERROR undefined name: 'PICK'
}
