// Bit ve aralık seçimi
module BitSelect {
    in  data : u8
    out msb  : bool
    out lsb  : bool
    out upper: bits<4>
    out lower: bits<4>

    msb   = data[7]
    lsb   = data[0]
    upper = data[7:4]
    lower = data[3:0]
}
