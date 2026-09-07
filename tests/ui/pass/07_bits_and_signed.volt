// bits<N> type and signed arithmetic
module BitsAndSigned {
    in  clk   : clock
    in  flags : bits<4>
    in  sval  : i8
    out sel   : bool
    out sacc  : i16

    reg acc : i16 = 0

    // Bit select on bits<N>
    sel = flags[2]

    on clk {
        acc <= acc + (sval as i16)
    }

    sacc = acc
}
