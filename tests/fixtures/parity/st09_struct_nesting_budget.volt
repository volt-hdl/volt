// parity: E4010
struct L0 { v : u1 }
struct L1 { x : L0 }
struct L2 { x : L1 }
struct L3 { x : L2 }
struct L4 { x : L3 }
struct L5 { x : L4 }
struct L6 { x : L5 }
struct L7 { x : L6 }
struct L8 { x : L7 }
module M {
    in  x : u4
    out y : u4
    y = x
}
