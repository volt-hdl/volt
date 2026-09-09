// Verilated -*- C++ -*-
// DESCRIPTION: Verilator output: Symbol table internal header
//
// Internal details; most calling programs do not need this header,
// unless using verilator public meta comments.

#ifndef VERILATED_VUARTTX__SYMS_H_
#define VERILATED_VUARTTX__SYMS_H_  // guard

#include "verilated.h"

// INCLUDE MODEL CLASS

#include "VUartTx.h"

// INCLUDE MODULE CLASSES
#include "VUartTx___024root.h"

// SYMS CLASS (contains all model state)
class alignas(VL_CACHE_LINE_BYTES) VUartTx__Syms final : public VerilatedSyms {
  public:
    // INTERNAL STATE
    VUartTx* const __Vm_modelp;
    VlDeleter __Vm_deleter;
    bool __Vm_didInit = false;

    // MODULE INSTANCE STATE
    VUartTx___024root              TOP;

    // CONSTRUCTORS
    VUartTx__Syms(VerilatedContext* contextp, const char* namep, VUartTx* modelp);
    ~VUartTx__Syms();

    // METHODS
    const char* name() const { return TOP.vlNamep; }
};

#endif  // guard
