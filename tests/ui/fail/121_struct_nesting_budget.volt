//~ E4010
// ADR-0077 Karar 5 rule 14: flattening stays within the ADR-0067 budget
// (at most 8 levels of nested structs).
struct L0 { v : u1 }
struct L1 { x : L0 }
struct L2 { x : L1 }
struct L3 { x : L2 }
struct L4 { x : L3 }
struct L5 { x : L4 }
struct L6 { x : L5 }
struct L7 { x : L6 }
struct L8 { x : L7 }
//~^ ERROR nests deeper than 8 levels

module M {
    in  a : u8
    out y : u8

    y = a
}
