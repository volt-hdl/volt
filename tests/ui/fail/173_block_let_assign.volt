//~ E4001
// ADR-0083 Karar 6: blok let'i bir değerin adıdır, register ya da
// değişken değildir — ona atama ikinci sürücüdür.

module LetAssign {
    in  a : u8
    in  b : u8
    out y : u8

    comb {
        let t = a
        t = b
        //~^ ERROR 't' is already driven
        y = t
    }
}
