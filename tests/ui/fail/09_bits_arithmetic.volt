//~ E2004
// bits<N> tipinde aritmetik yapılamaz.
// bits ham bit vektörüdür, sayısal değil.

module BitsArithmetic {
    in  a : bits<8>
    in  b : bits<8>
    out r : bits<8>

    r = a + b
    //~^ ERROR bits<N> tipinde aritmetik yapılamaz
}
