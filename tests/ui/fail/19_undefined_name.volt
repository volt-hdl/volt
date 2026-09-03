//~ E1001
// Tanımsız isme referans.

module UndefinedName {
    in  a : u8
    out r : u8

    r = a + undefined_signal
    //~^ ERROR tanımsız isim: 'undefined_signal'
}
