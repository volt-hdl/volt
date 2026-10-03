// suggestion: E2007
// The high bit of a range is written first.
module R {
    in  a : u8
    out y : bits<5>

    y = a[3:7]
}
