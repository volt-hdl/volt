// ADR-0097: a module without a clock port gives its contracts no edge to
// be checked on. They used to vanish from `volt verify` ("no contracts
// found", exit 0); now the run stops with E5005 (exit 1).

module Comb {
    in  a : u8
    out b : u8

    requires: a < 10
    ensures:  b == 77

    b = a + 1
}
