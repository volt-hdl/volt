// suggestion: W0025
// A misspelt key in Volt.toml: the fix renames it to the key Volt reads.
module Blink {
    in  a : bool
    out y : bool

    y = a
}
