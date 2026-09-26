//~ E0014
// ADR-0083 Karar 3: sayısal match ifadesi deyimle aynı kuralı izler —
// bütün değerler yazılmış olsa da `_` kolu zorunlu.

module NumericNoWildcard {
    in  op : u2
    in  a  : u8
    in  b  : u8
    out y  : u8

    y = match op { 0 => a, 1 => b, 2 => a, 3 => b }
    //~^ ERROR 'match' expression has no '_' arm
}
