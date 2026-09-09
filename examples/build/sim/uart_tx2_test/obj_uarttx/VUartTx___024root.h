// Verilated -*- C++ -*-
// DESCRIPTION: Verilator output: Design internal header
// See VUartTx.h for the primary calling header

#ifndef VERILATED_VUARTTX___024ROOT_H_
#define VERILATED_VUARTTX___024ROOT_H_  // guard

#include "verilated.h"


class VUartTx__Syms;

class alignas(VL_CACHE_LINE_BYTES) VUartTx___024root final {
  public:

    // DESIGN SPECIFIC STATE
    VL_IN8(clk,0,0);
    VL_IN8(rst,0,0);
    VL_IN8(start,0,0);
    VL_IN8(data,7,0);
    VL_OUT8(tx,0,0);
    VL_OUT8(busy,0,0);
    CData/*1:0*/ UartTx__DOT__state_r;
    CData/*3:0*/ UartTx__DOT__bit_count_r;
    CData/*7:0*/ UartTx__DOT__data_r;
    CData/*0:0*/ UartTx__DOT__tx_r;
    CData/*0:0*/ UartTx__DOT__busy_r;
    CData/*0:0*/ __VstlFirstIteration;
    CData/*0:0*/ __VstlPhaseResult;
    CData/*0:0*/ __Vtrigprevexpr___TOP__clk__0;
    CData/*0:0*/ __VactPhaseResult;
    CData/*0:0*/ __VnbaPhaseResult;
    SData/*9:0*/ UartTx__DOT__clk_count_r;
    IData/*31:0*/ __VactIterCount;
    VlUnpacked<QData/*63:0*/, 1> __VstlTriggered;
    VlUnpacked<QData/*63:0*/, 1> __VactTriggered;
    VlUnpacked<QData/*63:0*/, 1> __VnbaTriggered;

    // INTERNAL VARIABLES
    VUartTx__Syms* vlSymsp;
    const char* vlNamep;

    // CONSTRUCTORS
    VUartTx___024root(VUartTx__Syms* symsp, const char* namep);
    ~VUartTx___024root();
    VL_UNCOPYABLE(VUartTx___024root);

    // INTERNAL METHODS
    void __Vconfigure(bool first);
};


#endif  // guard
