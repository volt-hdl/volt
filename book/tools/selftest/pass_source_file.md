<!-- expect: pass -->
# A SystemVerilog file block that a later extern reads

The block is saved into the workspace; without it, `volt check` would fail
with E1012 (cannot read SystemVerilog source).

```systemverilog,file=rtl/Inv.sv
module Inv (input logic a, output logic y);
    assign y = ~a;
endmodule
```

```volt,file=top.volt
@source("rtl/Inv.sv")
extern module Inv {
    in  a : bool
    out y : bool
}

pub module Top {
    in  a : bool
    out y : bool

    let i = Inv { a: a }
    y = i.y
}
```
