// Verilated -*- C++ -*-
// DESCRIPTION: Verilator output: Model implementation (design independent parts)

#include "VUartTx__pch.h"

//============================================================
// Constructors

VUartTx::VUartTx(VerilatedContext* _vcontextp__, const char* _vcname__)
    : VerilatedModel{*_vcontextp__}
    , vlSymsp{new VUartTx__Syms(contextp(), _vcname__, this)}
    , clk{vlSymsp->TOP.clk}
    , rst{vlSymsp->TOP.rst}
    , start{vlSymsp->TOP.start}
    , data{vlSymsp->TOP.data}
    , tx{vlSymsp->TOP.tx}
    , busy{vlSymsp->TOP.busy}
    , rootp{&(vlSymsp->TOP)}
{
    // Register model with the context
    contextp()->addModel(this);
}

VUartTx::VUartTx(const char* _vcname__)
    : VUartTx(Verilated::threadContextp(), _vcname__)
{
}

//============================================================
// Destructor

VUartTx::~VUartTx() {
    delete vlSymsp;
}

//============================================================
// Evaluation function

#ifdef VL_DEBUG
void VUartTx___024root___eval_debug_assertions(VUartTx___024root* vlSelf);
#endif  // VL_DEBUG
void VUartTx___024root___eval_static(VUartTx___024root* vlSelf);
void VUartTx___024root___eval_initial(VUartTx___024root* vlSelf);
void VUartTx___024root___eval_settle(VUartTx___024root* vlSelf);
void VUartTx___024root___eval(VUartTx___024root* vlSelf);

void VUartTx::eval_step() {
    VL_DEBUG_IF(VL_DBG_MSGF("+++++TOP Evaluate VUartTx::eval_step\n"); );
#ifdef VL_DEBUG
    // Debug assertions
    VUartTx___024root___eval_debug_assertions(&(vlSymsp->TOP));
#endif  // VL_DEBUG
    vlSymsp->__Vm_deleter.deleteAll();
    if (VL_UNLIKELY(!vlSymsp->__Vm_didInit)) {
        VL_DEBUG_IF(VL_DBG_MSGF("+ Initial\n"););
        VUartTx___024root___eval_static(&(vlSymsp->TOP));
        VUartTx___024root___eval_initial(&(vlSymsp->TOP));
        VUartTx___024root___eval_settle(&(vlSymsp->TOP));
        vlSymsp->__Vm_didInit = true;
    }
    VL_DEBUG_IF(VL_DBG_MSGF("+ Eval\n"););
    VUartTx___024root___eval(&(vlSymsp->TOP));
    // Evaluate cleanup
    Verilated::endOfEval(vlSymsp->__Vm_evalMsgQp);
}

//============================================================
// Events and timing
bool VUartTx::eventsPending() { return false; }

uint64_t VUartTx::nextTimeSlot() {
    VL_FATAL_MT(__FILE__, __LINE__, "", "No delays in the design");
    return 0;
}

//============================================================
// Utilities

const char* VUartTx::name() const {
    return vlSymsp->name();
}

//============================================================
// Invoke final blocks

void VUartTx___024root___eval_final(VUartTx___024root* vlSelf);

VL_ATTR_COLD void VUartTx::final() {
    contextp()->executingFinal(true);
    VUartTx___024root___eval_final(&(vlSymsp->TOP));
    contextp()->executingFinal(false);
}

//============================================================
// Implementations of abstract methods from VerilatedModel

const char* VUartTx::hierName() const { return vlSymsp->name(); }
const char* VUartTx::modelName() const { return "VUartTx"; }
unsigned VUartTx::threads() const { return 1; }
void VUartTx::prepareClone() const { contextp()->prepareClone(); }
void VUartTx::atClone() const {
    contextp()->threadPoolpOnClone();
}
