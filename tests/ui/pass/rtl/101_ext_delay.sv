// SystemVerilog body of ExtDelay in 101_extern_source.volt (ADR-0076).
`default_nettype none
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
