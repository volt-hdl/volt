//~ E1001
// Reference to an undefined name.

module UndefinedName {
    in  a : u8
    out r : u8

    r = a + undefined_signal
    //~^ ERROR undefined name: 'undefined_signal'
}
