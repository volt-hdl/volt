// suggestion: E1001
// A pattern names a value; a misspelt const is suggested.
const LIMIT : u8 = 9

module P {
    in  a : u8
    out y : bool

    y = match a {
        LIMT => true,
        _ => false,
    }
}
