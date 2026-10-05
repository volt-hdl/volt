<!-- expect: fail -->
# A SystemVerilog file block whose path leaves the workspace

```systemverilog,file=../Inv.sv
module Inv (input logic a, output logic y);
    assign y = ~a;
endmodule
```
