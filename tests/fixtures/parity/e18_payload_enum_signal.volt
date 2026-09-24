// parity: E0003
enum Cmd { Idle, Load(u8) }
module M {
    in  c : Cmd
    out y : bool
    y = true
}
