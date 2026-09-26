//~ E2003
// ADR-0083 Karar 2: beklenen tip yokken (tipsiz let) kollar tek tipte
// olmalı — `if` ifadesinin kuralı.

module ArmTypes {
    in  op : u2
    in  a  : u8
    out y  : u8

    let r = match op { 0 => a, _ => true }
    //~^ ERROR match arms have different types: 'u8' and 'bool'
    y = a
}
