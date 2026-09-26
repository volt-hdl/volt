//~ E0003
// ADR-0083 Karar 1: match kolu muhafızı deyimde olduğu gibi ifadede de
// henüz SV'ye inmez; akış kuralı (muhafız trust/domain'e katılır) hazır.

module ArmGuard {
    in  op : u2
    in  a  : u8
    in  b  : u8
    out y  : u8

    y = match op { 0 if a > b => a, _ => b }
    //~^ ERROR not supported yet: 'match' arm guards ('if' after a pattern)
}
