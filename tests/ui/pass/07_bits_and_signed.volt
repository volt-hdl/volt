// bits<N> tipi ve işaretli aritmetik
module BitsAndSigned {
    in  clk   : clock
    in  flags : bits<4>
    in  sval  : i8
    out sel   : bool
    out sacc  : i16

    reg acc : i16 = 0

    // bits<N> üzerinde bit seçimi
    sel = flags[2]

    on clk {
        acc <= acc + (sval as i16)
    }

    sacc = acc
}
