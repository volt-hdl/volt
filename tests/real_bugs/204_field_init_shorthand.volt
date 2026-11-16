//~ E3001
//~ W3010
// #204: the field-init shorthand `S { d }` skipped the type, clock-domain,
// trust, handshake and timing checks: this design had no diagnostic, while
// the long form `S { d: d, x: true }` had E3001. The shorthand means exactly
// `d: d` (grammar-full.ebnf, FieldInit and PortBind), so `d @clk_a` read in
// `on clk_b` without sync() is a clock-domain crossing (domain-inference.md,
// E3001). Fixed by #238 (the parser desugars the shorthand to `d: d`).
// W3010: the implicit reset is shared by the two clocks (ADR-0065), in
// both forms.
struct S { d : u8, x : bool }

module M {
    in  clk_a : clock
    in  clk_b : clock
    in  d : u8 @clk_a
    out q : u8 @clk_b
    reg rb : u8 = 0
    let s = S { d, x: true }
    on clk_b { rb <= s.d }
    q = rb
}
