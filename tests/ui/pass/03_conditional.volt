// Koşullu ifade ve mantıksal operatörler
module Mux {
    in  sel : bool
    in  a   : u8
    in  b   : u8
    out y   : u8

    y = if sel { a } else { b }
}
