// ADR-0069: the recursive-type check (E4009) must not reject finite types.
// A diamond (Inner reached twice), generic nesting (Pair<Pair<u8>>), an
// alias chain and a generic argument that its struct never stores
// (Tag<Node>: Tag ignores T) are all finite.

struct Inner {
    a : u8
    b : bool
}

struct Outer {
    x : Inner
    y : Inner
}

type Word = Outer
type Word2 = Word

struct Pair<T> {
    l : T
    r : T
}

type Nested = Pair<Pair<u8>>

struct Tag<T> {
    v : u8
}

struct Node {
    t : Tag<Node>
    n : u8
}

enum Cmd {
    Nop
    Load(Inner)
}

module M {
    in  clk  : clock
    in  rx   : Handshake<Outer>
    out seen : bool

    rx.ready = true
    seen = rx.valid && rx.data.x.b
}
