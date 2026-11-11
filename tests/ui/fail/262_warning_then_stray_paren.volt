//~ E0001
// #206: the W0010 warning on `a & b | c` used to open the parser's
// cascade-suppression window, and the stray ')' right after it was
// swallowed: the file was accepted and the second invariant was missing
// from the generated .sva. A token that cannot start a statement is
// E0001 (error-recovery.md); a warning opens no window.
module W {
    in  clk : clock
    in  a : u8
    in  b : u8
    in  c : u8
    out r : u8
    out s : u8
    out t : bool
    reg cnt : u8 = 0
    invariant: cnt < 200
    r = a & b | c )
    //~^ ERROR unexpected ')'
    invariant: cnt == 255
    on clk { cnt <= cnt + 1 }
    s = cnt
    t = true
}
