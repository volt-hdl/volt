// Fourth referee canary (#309, Verilator #9256): signed `>>>` of a wide
// composite operand by an amount that is a port tied to a constant.
// Reported upstream: https://github.com/verilator/verilator/issues/9256
//
// Correct SystemVerilog, in the form Volt emits for #309: the amount `k` is
// a run-time value in Volt (a port), so Volt writes `(a + b) >>> k`; the
// instance ties it to a constant, and Verilator inlines the module and
// propagates the constant into the shift. IEEE 1800-2017 §11.4.10: the
// arithmetic right shift fills the vacated bits with the sign bit; with
// an amount >= the width every bit is the sign bit. Expected values,
// computed by hand, for the one input (a = -1, b = 0, so a + b = -1):
//
//   y65  = (a + b) >>> 130 on 65 bits    65 ones    1ffffffffffffffff
//   y97  = (a + b) >>> 130 on 97 bits    97 ones    1 and 24 f digits
//   y129 = (a + b) >>> 200 on 129 bits   129 ones   1 and 32 f digits
//   y66  = (a + b) >>> 132 on 66 bits    66 ones    3ffffffffffffffff
//
// Every amount is at or past the C++ word width (cppw = 32*ceil(w/32):
// 96, 128, 160, 96). y65, y97 and y129 have w > 64 and w mod 32 = 1, the
// class of Verilator #9256; y66 is the control, w mod 32 = 2.
//
// Measured with Volt's options (volt_tools::VERILATOR_BUILD_FLAGS, that is
// -fno-dfg): 5.052 prints 0 for all four (the oversized constant shift of
// #302, fixed upstream by commit 84dec259c8 in 5.054); 5.054 prints y66
// correctly and ...0001 for y65, y97 and y129 (the sign bit is extended as
// a 32-bit value). Volt cannot avoid this form (the amount is not a
// constant for Volt). The test is in
// crates/volt-driver/tests/verilator_canary_tests.rs.
module W65 (
    input  logic signed [64:0] a,
    input  logic signed [64:0] b,
    input  logic        [31:0] k,
    output logic signed [64:0] y
);
    assign y = (a + b) >>> k;
endmodule

module W97 (
    input  logic signed [96:0] a,
    input  logic signed [96:0] b,
    input  logic        [31:0] k,
    output logic signed [96:0] y
);
    assign y = (a + b) >>> k;
endmodule

module W129 (
    input  logic signed [128:0] a,
    input  logic signed [128:0] b,
    input  logic         [31:0] k,
    output logic signed [128:0] y
);
    assign y = (a + b) >>> k;
endmodule

module W66 (
    input  logic signed [65:0] a,
    input  logic signed [65:0] b,
    input  logic        [31:0] k,
    output logic signed [65:0] y
);
    assign y = (a + b) >>> k;
endmodule

module tb;
    logic signed [64:0]  a65,  b65,  y65;
    logic signed [96:0]  a97,  b97,  y97;
    logic signed [128:0] a129, b129, y129;
    logic signed [65:0]  a66,  b66,  y66;
    W65  w65  (.a(a65),  .b(b65),  .k(32'd130), .y(y65));
    W97  w97  (.a(a97),  .b(b97),  .k(32'd130), .y(y97));
    W129 w129 (.a(a129), .b(b129), .k(32'd200), .y(y129));
    W66  w66  (.a(a66),  .b(b66),  .k(32'd132), .y(y66));
    initial begin
        a65 = '1;  b65 = '0;
        a97 = '1;  b97 = '0;
        a129 = '1; b129 = '0;
        a66 = '1;  b66 = '0;
        #1;
        $display("R9256 %h %h %h %h", y65, y97, y129, y66);
        $finish;
    end
endmodule
