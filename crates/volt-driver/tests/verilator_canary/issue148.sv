// Second referee canary (ADR-0107, "Ek: DFG kapalı"; #148).
// Reported upstream: https://github.com/verilator/verilator/issues/9060
//
// Correct SystemVerilog: the design of issue145.sv without the assertion
// ens_0 (the second reader of o150a) and without Dt's clk/rst ports. Every
// input is 0; `assign o151a = 4'(p247) - 4'(p247);` is 0 for every p247.
// With the DFG optimizer on, Verilator 5.052 (the pinned release) and
// 5.053 devel both print o151a = 4'hf here (same mechanism as issue145.sv:
// `(a >= b) << 0x1f` is a signed int in C++ and the word split's right
// shift spreads its sign bit into o151k, o151e/d/b/a). With -fno-dfg both
// print the correct value. Volt passes -fno-dfg to every Verilator build
// (volt_tools::VERILATOR_BUILD_FLAGS); the canary test runs with them.
//
// The bit positions of every field matter. Do not reformat the design
// below; the expected values, computed by hand from the SV semantics, are
// in crates/volt-driver/tests/verilator_canary_tests.rs.
module tb;
    logic clk;
    logic rst;
    logic [12:0] p46;
    logic [5:0] p145;
    logic [11:0] p158;
    logic [7:0] p192;
    logic [5:0] p245;
    logic [14:0] p246;
    logic [2:0] p247;
    logic [24:0] p248;
    logic [11:0] o141b;
    logic o141f;
    logic signed [24:0] o146w;
    logic signed [18:0] o146r;
    logic o147a;
    logic o147b;
    logic o147c;
    logic o147d;
    logic o147e;
    logic o147f;
    logic o147r;
    logic [66:0] o148a;
    logic [66:0] o148b;
    logic [66:0] o148c;
    logic [66:0] o148d;
    logic [66:0] o148e;
    logic o148f;
    logic [66:0] o148r;
    logic [6:0] o149a;
    logic [6:0] o149b;
    logic [6:0] o149c;
    logic [6:0] o149d;
    logic [6:0] o149e;
    logic o149f;
    logic o149k;
    logic [7:0] o149w;
    logic [6:0] o149r;
    logic o150a;
    logic o150b;
    logic o150c;
    logic o150d;
    logic o150e;
    logic o150f;
    logic o150r;
    logic [3:0] o151a;
    logic [3:0] o151b;
    logic [3:0] o151d;
    logic [3:0] o151e;
    logic o151f;
    logic o151k;
    logic [5:0] o151w;
    logic [3:0] o151r;
    logic o152a;
    logic o152b;
    logic o152d;
    logic o152e;
    logic o152f;
    logic o152r;
    Dt dut (.*);
    initial begin
        rst = 1'b1;
        clk = 1'b0;
        #1 clk = 1'b1;
        #1 clk = 1'b0;
        rst = 1'b0;
        for (int i = 0; i < 1; i++) begin
            case (i)
                0: begin p46 = 13'h0; p145 = 6'h0; p158 = 12'h0; p192 = 8'h0; p245 = 6'h0; p246 = 15'h0; p247 = 3'h0; p248 = 25'h0; end
            endcase
            #1;
            $display("R %0d 17 %h", i, {o152r, o152f, o152e, o152d, o152b, o152a, o151r, o151w, o151k, o151f, o151e, o151d, o151b, o151a, o150r, o150f, o150e, o150d, o150c, o150b, o150a, o149r, o149w, o149k, o149f, o149e, o149d, o149c, o149b, o149a, o148r, o148f, o148e, o148d, o148c, o148b, o148a, o147r, o147f, o147e, o147d, o147c, o147b, o147a, o146r, o146w, o141f, o141b});
            clk = 1'b1;
            #1 clk = 1'b0;
            #1;
        end
        $display("DONE");
        $finish;
    end
endmodule
module Dt (
    input  logic [12:0]        p46,
    input  logic [5:0]         p145,
    input  logic [11:0]        p158,
    input  logic [7:0]         p192,
    input  logic [5:0]         p245,
    input  logic [14:0]        p246,
    input  logic [2:0]         p247,
    input  logic [24:0]        p248,
    output logic [11:0]        o141b,
    output logic               o141f,
    output logic signed [24:0] o146w,
    output logic signed [18:0] o146r,
    output logic               o147a,
    output logic               o147b,
    output logic               o147c,
    output logic               o147d,
    output logic               o147e,
    output logic               o147f,
    output logic               o147r,
    output logic [66:0]        o148a,
    output logic [66:0]        o148b,
    output logic [66:0]        o148c,
    output logic [66:0]        o148d,
    output logic [66:0]        o148e,
    output logic               o148f,
    output logic [66:0]        o148r,
    output logic [6:0]         o149a,
    output logic [6:0]         o149b,
    output logic [6:0]         o149c,
    output logic [6:0]         o149d,
    output logic [6:0]         o149e,
    output logic               o149f,
    output logic               o149k,
    output logic [7:0]         o149w,
    output logic [6:0]         o149r,
    output logic               o150a,
    output logic               o150b,
    output logic               o150c,
    output logic               o150d,
    output logic               o150e,
    output logic               o150f,
    output logic               o150r,
    output logic [3:0]         o151a,
    output logic [3:0]         o151b,
    output logic [3:0]         o151d,
    output logic [3:0]         o151e,
    output logic               o151f,
    output logic               o151k,
    output logic [5:0]         o151w,
    output logic [3:0]         o151r,
    output logic               o152a,
    output logic               o152b,
    output logic               o152d,
    output logic               o152e,
    output logic               o152f,
    output logic               o152r
);
    logic u147r_o;
    logic u150r_o;
    logic [3:0] u151r_o;
    logic u152r_o;
    assign o141b = '0;
    assign o141f = '0;
    assign o146w = '0;
    assign o146r = '0;
    wire t147b = 6'({{53{1'b0}}, p192}) <= ~p145;
    assign o147a = 6'({{53{1'b0}}, p192}) <= ~p145;
    assign o147b = t147b;
    wire [5:0] s147c = ~p145;
    assign o147c = 6'({{53{1'b0}}, p192}) <= s147c;
    assign o147d = 6'({{53{1'b0}}, p192}) <= ~p145;
    assign o147e = ~p145 >= 6'({{53{1'b0}}, p192});
    assign o147f = ~p145 >= 6'({{53{1'b0}}, p192}) == t147b;
    R147r_0_2 u147r (
        .p145(p145),
        .p192(p192),
        .o   (u147r_o)
    );
    assign o147r = u147r_o;
    assign o148a = '0;
    assign o148b = '0;
    assign o148c = '0;
    assign o148d = '0;
    assign o148e = '0;
    assign o148f = '0;
    assign o148r = '0;
    assign o149a = '0;
    assign o149b = '0;
    assign o149c = '0;
    assign o149d = '0;
    assign o149e = '0;
    assign o149f = '0;
    assign o149k = '0;
    assign o149w = '0;
    assign o149r = '0;
    wire t150b = 13'({{6{1'b0}}, p245}) - 13'(p158) <= 13'((14'(p246[12:0]) - 14'(p46)));
    assign o150a = 13'({{6{1'b0}}, p245}) - 13'(p158) <= 13'((14'(p246[12:0]) - 14'(p46)));
    assign o150b = t150b;
    wire [12:0] s150c = 13'((14'(p246[12:0]) - 14'(p46)));
    assign o150c = 13'({{6{1'b0}}, p245}) - 13'(p158) <= s150c;
    assign o150d = 13'({{6{1'b0}}, p245}) - 13'(p158) <= 13'((14'(p246[12:0]) - 14'(p46)));
    assign o150e = 13'((14'(p246[12:0]) - 14'(p46))) >= 13'({{6{1'b0}}, p245}) - 13'(p158);
    assign o150f = 13'((14'(p246[12:0]) - 14'(p46))) >= 13'({{6{1'b0}}, p245}) - 13'(p158) == t150b;
    R150r_0_2 u150r (
        .p46 (p46),
        .p158(p158),
        .p245(p245),
        .p246(p246),
        .o   (u150r_o)
    );
    assign o150r = u150r_o;
    wire [3:0] t151b = 4'(p247) - 4'(p247);
    assign o151a = 4'(p247) - 4'(p247);
    assign o151b = t151b;
    assign o151d = 4'(p247) - 4'(p247);
    assign o151e = 4'(p247) - 4'(p247);
    assign o151f = 4'(p247) - 4'(p247) == t151b;
    assign o151k = 4'(p247) - 4'(p247) >= 4'd12;
    assign o151w = 6'(p247) - 6'(p247);
    R151r_0_2 u151r (
        .p247(p247),
        .o   (u151r_o)
    );
    assign o151r = u151r_o;
    wire t152b = p248 < 25'h1FFFFFF;
    assign o152a = p248 < 25'h1FFFFFF;
    assign o152b = t152b;
    assign o152d = p248 < 25'h1FFFFFF;
    assign o152e = 25'h1FFFFFF > p248;
    assign o152f = 25'h1FFFFFF > p248 == t152b;
    R152r_0_2 u152r (
        .p248(p248),
        .o   (u152r_o)
    );
    assign o152r = u152r_o;
endmodule
module R147r_0_2 (
    input  logic [5:0] p145,
    input  logic [7:0] p192,
    output logic       o
);
    assign o = 6'({{53{1'b0}}, p192}) <= ~6'd0;
endmodule
module R150r_0_2 (
    input  logic [12:0] p46,
    input  logic [11:0] p158,
    input  logic [5:0]  p245,
    input  logic [14:0] p246,
    output logic        o
);
    always_comb begin : comb_0
        o = 13'({{6{1'b0}}, p245}) - 13'(p158) <= 13'((14'(p246[12:0]) - 14'(13'd0)));
    end
endmodule
module R151r_0_2 (
    input  logic [2:0] p247,
    output logic [3:0] o
);
    assign o = 4'(3'd0) - 4'(3'd0);
endmodule
module R152r_0_2 (
    input  logic [24:0] p248,
    output logic        o
);
    always_comb begin : comb_0
        o = 25'd0 < 25'h1FFFFFF;
    end

endmodule
