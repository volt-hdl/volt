// Third referee canary (#302): signed `>>>` by a constant at or past the
// width. Not a DFG bug: -fno-dfg does not change it.
//
// Correct SystemVerilog. IEEE 1800-2017 §11.4.10: the arithmetic right
// shift fills the vacated bits with the sign bit when the result is
// signed; with an amount >= the width every bit is vacated, so every bit
// is the sign bit. Expected values, computed by hand, for the one input
// (a = -1, e = -1, f = 0):
//
//   y64  = a >>> 64          63 ones   7fffffffffffffff
//   y62  = a >>> 62          63 ones   7fffffffffffffff
//   s130 = (e + f) >>> 130   65 ones   1ffffffffffffffff
//   s64  = (e + f) >>> 64    65 ones   1ffffffffffffffff
//
// y64 and s130 are the forms Volt emitted before #302; y62 and s64 the
// forms it emits now (ADR-0120: a signed `>>` by a constant K >= w is
// written `>>> (w-1)`, the same value).
//
// Measured with Volt's options (volt_tools::VERILATOR_BUILD_FLAGS): the
// pinned release, Verilator 5.052, prints y64 = 0 and s130 = 0: a
// constant K at or past the C++ word width selects the sign from bit
// cppw-1 instead of w-1 (V3Premit, introduced in 5.050, fixed upstream
// by commit 84dec259c8 in 5.054). Release 5.054 prints y64 correctly
// and s130 = 00000000000000001: for a composite operand of width w > 64
// with w mod 32 = 1, the sign is extended with lbits 32 instead of 1. Both
// print y62 and s64 correctly. The test is in
// crates/volt-driver/tests/verilator_canary_tests.rs.
module S (
    input  logic signed [62:0] a,
    input  logic signed [64:0] e,
    input  logic signed [64:0] f,
    output logic signed [62:0] y64,
    output logic signed [62:0] y62,
    output logic signed [64:0] s130,
    output logic signed [64:0] s64
);
    assign y64  = a >>> 64;
    assign y62  = a >>> 62;
    assign s130 = (e + f) >>> 130;
    assign s64  = (e + f) >>> 64;
endmodule

module tb;
    logic signed [62:0] a;
    logic signed [64:0] e;
    logic signed [64:0] f;
    logic signed [62:0] y64;
    logic signed [62:0] y62;
    logic signed [64:0] s130;
    logic signed [64:0] s64;
    S dut (.a(a), .e(e), .f(f), .y64(y64), .y62(y62), .s130(s130), .s64(s64));
    initial begin
        a = '1;
        e = '1;
        f = '0;
        #1;
        $display("R302 %h %h %h %h", y64, y62, s130, s64);
        $finish;
    end
endmodule
