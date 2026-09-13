//~ E1006
package a;

use b::B;
//~^ ERROR cyclic import: 'b'

pub module A {
    in  clk : clock
    in  x   : bool
    out y   : bool
    y = x
}
