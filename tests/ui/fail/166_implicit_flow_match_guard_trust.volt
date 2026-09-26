//~ E3009
// ADR-0083 Karar 8: a match arm guard selects the arm, so the result
// carries the guard's trust level (implicit flow, ADR-0052 K7).

domain Secret { clock = posedge, reset = sync active_high, trust_level = secret }
domain Pub    { clock = posedge, reset = sync active_high, trust_level = public }

module GuardLeak {
    in  clk : clock @Pub
    in  a   : u2 @Pub
    in  key : u8 @Secret
    out dbg : bool @Pub

    dbg = match a { 0 if key[0] => true, _ => false }
    //~^ ERROR secret data flows to a public output
}
