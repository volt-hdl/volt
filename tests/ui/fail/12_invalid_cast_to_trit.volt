//~ E2009
// Casting from a numeric type to Trit is forbidden.
// It would silently lose information.

module InvalidCast {
    in  value : i8
    out t     : Trit

    t = value as Trit
    //~^ ERROR cast 'i8' -> 'Trit' is invalid
}
