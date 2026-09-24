// parity: E1013
enum pulsestyle { ondetect, other }
module T {
    in  clk : clock
    in  a   : bool
    out y   : bool
    reg s : pulsestyle = pulsestyle::ondetect
    on clk { if a { s <= pulsestyle::other } }
    y = s == pulsestyle::other
}
