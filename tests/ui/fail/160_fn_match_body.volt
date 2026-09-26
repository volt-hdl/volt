//~ E0003
// ADR-0081 Karar 12.4: gövde çağrılmasa da bir kez doğrulanır; match kolu
// muhafızının SV eşlemesi yok (ADR-0083 Karar 1 — match ifadesinin kendisi
// artık iner) — tanı fn tanımında, çağrı sayısından bağımsız.
fn pick(x: u8) -> u8 {
    match x { 0 if x == 0 => 1, _ => x }
//~^ ERROR E0003 not supported yet: 'match' arm guards ('if' after a pattern)
}

module TwoCalls {
    in  a : u8
    in  b : u8
    out y : u8
    y = pick(a) + pick(b)
}
