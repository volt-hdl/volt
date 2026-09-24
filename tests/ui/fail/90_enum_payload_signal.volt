//~ E0003
// ADR-0074: enums with data-carrying variants are legal declarations but
// have no hardware mapping yet (tag + union layout is future work).
enum Cmd { Idle, Load(u8) }

module M {
    in  c : Cmd
//~^ ERROR data-carrying variants
    out y : bool

    y = true
}
