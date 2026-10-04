// suggestion: E2001
// An 8-bit sum does not fit a 4-bit output: the fix keeps the low four
// bits explicitly; 'as u4' alone would narrow with warning W2010.
module Nib {
    in  a : u8
    out y : u4

    y = a + 1
}
