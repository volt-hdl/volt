// parity: E1013
struct M {
    ondetect : u8
}
module T {
    in  pulsestyle : M
    out y : u8
    y = pulsestyle.ondetect
}
