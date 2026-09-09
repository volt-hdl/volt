// Verilated -*- C++ -*-
// DESCRIPTION: Verilator output: Design implementation internals
// See VUartTx.h for the primary calling header

#include "VUartTx__pch.h"

bool VUartTx___024root___trigger_anySet__act(const VlUnpacked<QData/*63:0*/, 1> &in) {
    VL_DEBUG_IF(VL_DBG_MSGF("+    VUartTx___024root___trigger_anySet__act\n"); );
    // Locals
    IData/*31:0*/ n;
    // Body
    n = 0U;
    do {
        if (in[n]) {
            return (1U);
        }
        n = ((IData)(1U) + n);
    } while ((1U > n));
    return (0U);
}

void VUartTx___024root___trigger_orInto__act_vec_vec(VlUnpacked<QData/*63:0*/, 1> &out, const VlUnpacked<QData/*63:0*/, 1> &in) {
    VL_DEBUG_IF(VL_DBG_MSGF("+    VUartTx___024root___trigger_orInto__act_vec_vec\n"); );
    // Locals
    IData/*31:0*/ n;
    // Body
    n = 0U;
    do {
        out[n] = (out[n] | in[n]);
        n = ((IData)(1U) + n);
    } while ((0U >= n));
}

#ifdef VL_DEBUG
VL_ATTR_COLD void VUartTx___024root___dump_triggers__act(const VlUnpacked<QData/*63:0*/, 1> &triggers, const std::string &tag);
#endif  // VL_DEBUG

bool VUartTx___024root___eval_phase__act(VUartTx___024root* vlSelf) {
    VL_DEBUG_IF(VL_DBG_MSGF("+    VUartTx___024root___eval_phase__act\n"); );
    VUartTx__Syms* const __restrict vlSymsp VL_ATTR_UNUSED = vlSelf->vlSymsp;
    auto& vlSelfRef = std::ref(*vlSelf).get();
    // Body
    {
        // Inlined CFunc: _eval_triggers_vec__act
        vlSelfRef.__VactTriggered[0U] = (QData)((IData)(
                                                        ((IData)(vlSelfRef.clk) 
                                                         & (~ (IData)(vlSelfRef.__Vtrigprevexpr___TOP__clk__0)))));
        vlSelfRef.__Vtrigprevexpr___TOP__clk__0 = vlSelfRef.clk;
    }
#ifdef VL_DEBUG
    if (VL_UNLIKELY(vlSymsp->_vm_contextp__->debug())) {
        VUartTx___024root___dump_triggers__act(vlSelfRef.__VactTriggered, "act"s);
    }
#endif
    VUartTx___024root___trigger_orInto__act_vec_vec(vlSelfRef.__VnbaTriggered, vlSelfRef.__VactTriggered);
    return (0U);
}

void VUartTx___024root___trigger_clear__act(VlUnpacked<QData/*63:0*/, 1> &out) {
    VL_DEBUG_IF(VL_DBG_MSGF("+    VUartTx___024root___trigger_clear__act\n"); );
    // Locals
    IData/*31:0*/ n;
    // Body
    n = 0U;
    do {
        out[n] = 0ULL;
        n = ((IData)(1U) + n);
    } while ((1U > n));
}

bool VUartTx___024root___eval_phase__nba(VUartTx___024root* vlSelf) {
    VL_DEBUG_IF(VL_DBG_MSGF("+    VUartTx___024root___eval_phase__nba\n"); );
    VUartTx__Syms* const __restrict vlSymsp VL_ATTR_UNUSED = vlSelf->vlSymsp;
    auto& vlSelfRef = std::ref(*vlSelf).get();
    // Locals
    CData/*0:0*/ __VnbaExecute;
    // Body
    __VnbaExecute = VUartTx___024root___trigger_anySet__act(vlSelfRef.__VnbaTriggered);
    if (__VnbaExecute) {
        {
            // Inlined CFunc: _eval_nba
            if ((1ULL & vlSelfRef.__VnbaTriggered[0U])) {
                {
                    // Inlined CFunc: _nba_sequent__TOP__0
                    CData/*1:0*/ __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__state_r;
                    __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__state_r = 0;
                    SData/*9:0*/ __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__clk_count_r;
                    __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__clk_count_r = 0;
                    CData/*3:0*/ __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__bit_count_r;
                    __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__bit_count_r = 0;
                    CData/*7:0*/ __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__data_r;
                    __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__data_r = 0;
                    __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__state_r 
                        = vlSelfRef.UartTx__DOT__state_r;
                    __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__clk_count_r 
                        = vlSelfRef.UartTx__DOT__clk_count_r;
                    __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__bit_count_r 
                        = vlSelfRef.UartTx__DOT__bit_count_r;
                    __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__data_r 
                        = vlSelfRef.UartTx__DOT__data_r;
                    if (vlSelfRef.rst) {
                        __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__state_r = 0U;
                        __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__clk_count_r = 0U;
                        __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__bit_count_r = 0U;
                        __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__data_r = 0U;
                        vlSelfRef.UartTx__DOT__tx_r = 1U;
                        vlSelfRef.UartTx__DOT__busy_r = 0U;
                    } else if ((0U == (IData)(vlSelfRef.UartTx__DOT__state_r))) {
                        vlSelfRef.UartTx__DOT__tx_r = 1U;
                        vlSelfRef.UartTx__DOT__busy_r = 0U;
                        __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__clk_count_r = 0U;
                        __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__bit_count_r = 0U;
                        if (vlSelfRef.start) {
                            __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__data_r 
                                = vlSelfRef.data;
                            vlSelfRef.UartTx__DOT__busy_r = 1U;
                            __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__state_r = 1U;
                        }
                    } else if ((1U == (IData)(vlSelfRef.UartTx__DOT__state_r))) {
                        vlSelfRef.UartTx__DOT__tx_r = 0U;
                        if ((3U == (IData)(vlSelfRef.UartTx__DOT__clk_count_r))) {
                            __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__clk_count_r = 0U;
                            __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__state_r = 2U;
                        } else {
                            __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__clk_count_r 
                                = (0x000003ffU & ((IData)(1U) 
                                                  + (IData)(vlSelfRef.UartTx__DOT__clk_count_r)));
                        }
                    } else if ((2U == (IData)(vlSelfRef.UartTx__DOT__state_r))) {
                        vlSelfRef.UartTx__DOT__tx_r 
                            = (1U & (IData)(vlSelfRef.UartTx__DOT__data_r));
                        if ((3U == (IData)(vlSelfRef.UartTx__DOT__clk_count_r))) {
                            __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__data_r 
                                = ((IData)(vlSelfRef.UartTx__DOT__data_r) 
                                   >> 1U);
                            __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__clk_count_r = 0U;
                            if ((7U <= (IData)(vlSelfRef.UartTx__DOT__bit_count_r))) {
                                __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__bit_count_r = 0U;
                                __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__state_r = 3U;
                            } else {
                                __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__bit_count_r 
                                    = (0x0000000fU 
                                       & ((IData)(1U) 
                                          + (IData)(vlSelfRef.UartTx__DOT__bit_count_r)));
                            }
                        } else {
                            __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__clk_count_r 
                                = (0x000003ffU & ((IData)(1U) 
                                                  + (IData)(vlSelfRef.UartTx__DOT__clk_count_r)));
                        }
                    } else {
                        vlSelfRef.UartTx__DOT__tx_r = 1U;
                        if ((3U == (IData)(vlSelfRef.UartTx__DOT__clk_count_r))) {
                            __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__clk_count_r = 0U;
                            vlSelfRef.UartTx__DOT__busy_r = 0U;
                            __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__state_r = 0U;
                        } else {
                            __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__clk_count_r 
                                = (0x000003ffU & ((IData)(1U) 
                                                  + (IData)(vlSelfRef.UartTx__DOT__clk_count_r)));
                        }
                    }
                    vlSelfRef.UartTx__DOT__state_r 
                        = __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__state_r;
                    vlSelfRef.UartTx__DOT__clk_count_r 
                        = __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__clk_count_r;
                    vlSelfRef.UartTx__DOT__bit_count_r 
                        = __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__bit_count_r;
                    vlSelfRef.UartTx__DOT__data_r = __Vinline_0__eval_nba___Vinline_0__nba_sequent__TOP__0___Vdly__UartTx__DOT__data_r;
                    vlSelfRef.tx = vlSelfRef.UartTx__DOT__tx_r;
                    vlSelfRef.busy = vlSelfRef.UartTx__DOT__busy_r;
                }
            }
        }
        VUartTx___024root___trigger_clear__act(vlSelfRef.__VnbaTriggered);
    }
    return (__VnbaExecute);
}

void VUartTx___024root___eval(VUartTx___024root* vlSelf) {
    VL_DEBUG_IF(VL_DBG_MSGF("+    VUartTx___024root___eval\n"); );
    VUartTx__Syms* const __restrict vlSymsp VL_ATTR_UNUSED = vlSelf->vlSymsp;
    auto& vlSelfRef = std::ref(*vlSelf).get();
    // Locals
    IData/*31:0*/ __VnbaIterCount;
    // Body
    __VnbaIterCount = 0U;
    do {
        if (VL_UNLIKELY(((0x00002710U < __VnbaIterCount)))) {
#ifdef VL_DEBUG
            VUartTx___024root___dump_triggers__act(vlSelfRef.__VnbaTriggered, "nba"s);
#endif
            VL_FATAL_MT("UartTx.sv", 9, "", "DIDNOTCONVERGE: NBA region did not converge after '--converge-limit' of 10000 tries");
        }
        __VnbaIterCount = ((IData)(1U) + __VnbaIterCount);
        vlSelfRef.__VactIterCount = 0U;
        do {
            if (VL_UNLIKELY(((0x00002710U < vlSelfRef.__VactIterCount)))) {
#ifdef VL_DEBUG
                VUartTx___024root___dump_triggers__act(vlSelfRef.__VactTriggered, "act"s);
#endif
                VL_FATAL_MT("UartTx.sv", 9, "", "DIDNOTCONVERGE: Active region did not converge after '--converge-limit' of 10000 tries");
            }
            vlSelfRef.__VactIterCount = ((IData)(1U) 
                                         + vlSelfRef.__VactIterCount);
            vlSelfRef.__VactPhaseResult = VUartTx___024root___eval_phase__act(vlSelf);
        } while (vlSelfRef.__VactPhaseResult);
        vlSelfRef.__VnbaPhaseResult = VUartTx___024root___eval_phase__nba(vlSelf);
    } while (vlSelfRef.__VnbaPhaseResult);
}

#ifdef VL_DEBUG
void VUartTx___024root___eval_debug_assertions(VUartTx___024root* vlSelf) {
    VL_DEBUG_IF(VL_DBG_MSGF("+    VUartTx___024root___eval_debug_assertions\n"); );
    VUartTx__Syms* const __restrict vlSymsp VL_ATTR_UNUSED = vlSelf->vlSymsp;
    auto& vlSelfRef = std::ref(*vlSelf).get();
    // Body
    if (VL_UNLIKELY(((vlSelfRef.clk & 0xfeU)))) {
        Verilated::overWidthError("clk");
    }
    if (VL_UNLIKELY(((vlSelfRef.rst & 0xfeU)))) {
        Verilated::overWidthError("rst");
    }
    if (VL_UNLIKELY(((vlSelfRef.start & 0xfeU)))) {
        Verilated::overWidthError("start");
    }
}
#endif  // VL_DEBUG
