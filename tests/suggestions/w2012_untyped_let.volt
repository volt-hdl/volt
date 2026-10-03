// suggestion: W2012
// An unsuffixed literal let defaults to i32; the fix writes exactly that
// type (a narrower one would not hold every value of the literal's type).
module Lit {
    in  x : i32
    out y : i32

    let k = 5
    y = x + k
}
