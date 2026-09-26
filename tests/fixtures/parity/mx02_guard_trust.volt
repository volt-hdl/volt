// parity: E3009
// ADR-0083 Karar 8: muhafızın güven etiketi sonuca katılır.
domain Secret { clock = posedge, reset = sync active_high, trust_level = secret }
domain Pub    { clock = posedge, reset = sync active_high, trust_level = public }

module GuardLeak {
    in  clk : clock @Pub
    in  a   : u2 @Pub
    in  key : u8 @Secret
    out dbg : bool @Pub

    dbg = match a { 0 if key[0] => true, _ => false }
}
