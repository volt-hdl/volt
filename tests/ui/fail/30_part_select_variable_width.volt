//~ E2008
// The width of an indexed part-select must be a compile-time constant
// (IEEE 1800 §11.5.1); only the start index may vary (ADR-0035).

module PartSelectVarWidth {
    in  data : u32
    in  i    : bits<5>
    in  w    : bits<5>
    out o    : bits<8>

    o = data[i +: w]
    //~^ ERROR part-select width must be a compile-time constant
}
