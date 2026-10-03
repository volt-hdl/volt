// suggestion: W0025
// A misspelt section in Volt.toml: the fix renames it to [package].
module Blink {
    in  a : bool
    out y : bool

    y = a
}
