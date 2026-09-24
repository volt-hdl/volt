// Hand-written SystemVerilog bodies for the extern modules declared in
// ext_top.volt (ADR-0076). Two modules share one file.
`default_nettype none

module ExtInvert (
    input  logic [7:0] a,
    output logic [7:0] y
);
    assign y = ~a;
endmodule

module ExtDelay (
    input  logic       clk,
    input  logic [7:0] d,
    output logic [7:0] q
);
    logic [7:0] q_r = 8'd0;
    always_ff @(posedge clk) q_r <= d;
    assign q = q_r;
endmodule

`default_nettype wire
