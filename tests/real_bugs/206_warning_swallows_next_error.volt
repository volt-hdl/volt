//~ W0010
//~ E0001
// #206: the W0010 warning on `a & b | c` opened the parser's
// cascade-suppression window, so the stray ')' one token later was
// swallowed: the file was accepted with only the warning (and a contract
// after it vanished from the generated .sva). operator-precedence.md
// (mixing '&' and '|' without parentheses is W0010) and error-recovery.md
// (a token that cannot continue the statement is E0001; the suppression
// window follows an error, not a warning): both are reported. Fixed in
// PR #243.
module W {
    in  a : u8
    in  b : u8
    in  c : u8
    out r : u8
    r = a & b | c )
}
