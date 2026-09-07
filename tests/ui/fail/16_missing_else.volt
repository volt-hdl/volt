//~ E0008
// Missing else in an if expression -- latch risk.

module MissingElse {
    in  cond : bool
    in  a    : u8
    out r    : u8

    r = if cond { a }
    //~^ ERROR 'if' expression requires an 'else' branch
}
