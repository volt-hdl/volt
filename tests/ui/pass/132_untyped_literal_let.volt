// An unsuffixed literal `let` is an i32 (type-inference.md §5, W2012) and
// is emitted as one. It used to report W2012 ("i32 assumed") and E2005
// ("cannot determine the width") on the same line (#80).
module UntypedLiteralLet {
    in  x : i32
    out y : i32

    let k = 100
    y = x + k
}
