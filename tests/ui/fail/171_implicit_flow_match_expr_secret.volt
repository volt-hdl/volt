//~ E3009
// ADR-0083 §2.1: sonucu sabit olsa da match seçimi sınananın bilgisini
// taşır (örtük akış, ADR-0052 K7).

domain Secret { clock = posedge, reset = sync active_high, trust_level = secret }
domain Pub    { clock = posedge, reset = sync active_high, trust_level = public }

module SelectLeak {
    in  clk : clock @Pub
    in  key : u8 @Secret
    out dbg : u8 @Pub

    dbg = match key[0] { true => 1, _ => 0 }
    //~^ ERROR secret data flows to a public output
}
