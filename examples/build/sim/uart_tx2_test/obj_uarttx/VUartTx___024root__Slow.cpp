// Verilated -*- C++ -*-
// DESCRIPTION: Verilator output: Design implementation internals
// See VUartTx.h for the primary calling header

#include "VUartTx__pch.h"

void VUartTx___024root___ctor_var_reset(VUartTx___024root* vlSelf);

VUartTx___024root::VUartTx___024root(VUartTx__Syms* symsp, const char* namep)
 {
    vlSymsp = symsp;
    vlNamep = strdup(namep);
    // Reset structure values
    VUartTx___024root___ctor_var_reset(this);
}

void VUartTx___024root::__Vconfigure(bool first) {
    (void)first;  // Prevent unused variable warning
}

VUartTx___024root::~VUartTx___024root() {
    VL_DO_DANGLING(std::free(const_cast<char*>(vlNamep)), vlNamep);
}
