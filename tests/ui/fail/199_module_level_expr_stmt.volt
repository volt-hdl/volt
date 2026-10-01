//~ E0001
// An expression on its own is not a statement. At module level `y == a`
// (a typo for `y = a`) used to be dropped without an error.

module ModuleLevelExprStmt {
    in  a : u8
    out y : u8

    y == a
    //~^ ERROR an expression on its own is not a statement
}
