//~ E2010
// #213: a negative literal skipped the range check. `-129` in an i8
// context was accepted with no diagnostic and written 8'sh7F, so `b` was
// +127. Right answer: `-K` is a literal whose value -K must fit the
// expected type (type-inference.md §4, "Kontrol Modu ve Literal
// Çözümleme"); i8 is -128..=127, so -129 is E2010. Fixed by #265.
module M {
    out b : i8
    b = -129
}
