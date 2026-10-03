// suggestion: W1001
// A wire that is driven but never read: the fix renames it with a '_'
// prefix everywhere it appears.
module U {
    in  a : u8
    out y : u8

    wire t : u8
    t = a + 1
    y = a
}
