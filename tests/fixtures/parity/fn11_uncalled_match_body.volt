// parity: E0003
// ADR-0081 Karar 12.4: hiç çağrılmayan fn de tanımda doğrulanır (muhafız: ADR-0083 Karar 1).
fn unused(a: u8) -> u8 { match a { 0 if a == 0 => 1, _ => a } }
module M { in a : u8  out y : u8  y = a }
