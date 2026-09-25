//! İngilizce (VARSAYILAN) kod açıklamaları.
//!
//! Terminoloji: docs/spec/GLOSSARY.md — BAĞLAYICI. `match` bilinçli
//! olarak joker kolsuz: yeni kod eklenince derleyici burayı da zorlar,
//! böylece en.rs ve tr.rs aynı kod kümesini kapsar.

use crate::code::ErrorCode;

/// Short English description for `code` (fed into `volt explain`).
pub fn description(code: ErrorCode) -> &'static str {
    use ErrorCode::*;
    match code {
        // ─── Syntax (grammar-full.ebnf §18) ───
        E0001 => "Unexpected token",
        E0002 => "Missing closing delimiter: } ) ]",
        E0003 => "Keyword is reserved, not yet supported",
        E0004 => "Block end name does not match",
        E0005 => "Invalid numeric literal",
        E0006 => "'=' used in a sequential block (should be '<=')",
        E0007 => "'<=' used in a combinational block (should be '=')",
        E0008 => "Missing 'else' in 'if' expression (latch risk)",
        E0009 => "Invalid attribute argument",
        E0010 => "Comparison operators cannot be chained",
        E0011 => "Unexpected end of file",
        E0012 => "Invalid escape sequence",
        E0013 => "Unterminated block comment",
        E0014 => "The match statement does not cover every value",
        E0015 => "MMIO register map layout error (ADR-0044)",
        E0016 => "declassify without a reason string (ADR-0052)",
        E0017 => "Unsupported or inconsistent timing constraint (ADR-0054)",
        E0018 => "Nesting or chain too deep (ADR-0080)",

        // ─── Name resolution (name-resolution.md) ───
        E1001 => "Undefined name",
        E1002 => "Use before declaration",
        E1003 => "Duplicate definition in the same scope",
        E1004 => "Access to a private (non-pub) item",
        E1005 => "'::' used on an item that is not a namespace",
        E1006 => "Cyclic module dependency",
        E1007 => "Enum variant not found",
        E1008 => "Struct field not found",
        E1009 => "Module port not found",
        E1010 => "Ambiguous import (two 'use' bring in the same name)",
        E1011 => "Module not found (no file provides the imported package)",
        E1012 => "Extern module SystemVerilog source missing, not found or outside the project",
        E1013 => "Name is a reserved word of a generated language",
        E1014 => "Two @mmio names generate the same identifier in the register-map driver",

        // ─── Type inference (type-inference.md) ───
        E2001 => "Bit width mismatch",
        E2002 => "Signedness mismatch",
        E2003 => "Type mismatch (general)",
        E2004 => "Arithmetic on a bits<N> type",
        E2005 => "Width or length cannot be determined (literal, cast source, let, sync source, bits<N> / [T; N] size)",
        E2006 => "Index/range out of bounds",
        E2007 => "Reversed range (hi < lo)",
        E2008 => "Variable range bound",
        E2009 => "Invalid cast",
        E2010 => "Literal does not fit the target type",
        E2011 => "Invalid Trit literal",
        E2012 => "Register type cannot be determined",
        E2013 => "Invalid struct declaration: no fields, a clock-domain annotation on a field, or a 'struct port' bundle as a field type (ADR-0077)",
        E2014 => "Struct literal is missing a field or sets a field twice (ADR-0077)",

        // ─── Constant evaluation (const-eval.md) ───
        E2020 => "Cyclic constant dependency",
        E2021 => "Constant expression expected (runtime value used)",
        E2022 => "Compile-time overflow",
        E2023 => "Division by zero",
        E2024 => "Invalid shift amount",
        E2025 => "Invalid width (zero, negative, or too large)",
        E2026 => "Array size limit exceeded",
        E2027 => "Loop unrolling limit exceeded",
        E2028 => "Invalid range (end < start)",
        E2029 => "Constant array index out of bounds",
        E2030 => "Invalid enum encoding",

        // ─── Clock/reset domains (domain-inference.md) ───
        E3001 => "Clock domain mismatch (CDC)",
        E3002 => "Undefined clock domain",
        E3003 => "Reset domain mismatch (RDC)",
        E3004 => "Reset sequence violation",
        E3005 => "Conditional reset not satisfied",
        E3006 => "Power domain crossing without isolation [V1]",
        E3007 => "Power sequence violation [V1]",
        E3008 => "Missing retention [V1]",
        E3009 => "Information flow violation (trust_level)",
        E3010 => "Ambiguous domain (multiple clocks, no annotation)",
        E3011 => "Register written from more than one domain",
        E3012 => "Foreign-domain signal read inside an 'on' block",
        E3013 => "Bundle fields inferred in different clock domains (ADR-0039)",
        E3014 => "Same symbolic domain bound to two different clocks (ADR-0047)",

        // ─── Connectivity/drivers (type-inference.md) ───
        E4001 => "Double driver",
        E4002 => "Undriven output port",
        E4003 => "Linear port consumed twice [V1]",
        E4004 => "Linear port never consumed [V1]",
        E4005 => "Bundle field direction violated (ADR-0039)",
        E4006 => "Bus-owned MMIO register field written from RTL (ADR-0044)",
        E4007 => "Handshake valid depends combinationally on ready (ADR-0050)",
        E4008 => "Bidirectional port misuse: direct assignment, unknown member or drive call outside an on block (ADR-0051)",
        E4009 => "Recursive type: a struct, struct port, enum or type alias contains itself and has no finite width (ADR-0067, ADR-0069)",
        E4010 => "Bundle flattening budget exceeded: more than 4096 flat ports in one module or nesting deeper than 8 levels (ADR-0067)",
        E4011 => "Instance port connection error: input or clock not bound, output or bidirectional port bound wrongly, missing reset in the parent, or an instance port driven from outside (ADR-0072)",
        E4012 => "Part of a signal is never driven: a struct field or some bits of a vector assigned piece by piece (ADR-0077)",

        // ─── Behavioral contracts ───
        E5001 => "Contract violated (formal verification found a counterexample)",
        E5002 => "Contract not proven (induction step failed; sby status UNKNOWN)",
        E5004 => "Contract expression is not Bool",
        E5010 => "Timing misalignment (values from different pipeline stages combined)",
        E5011 => "Invalid pipeline structure (stage count, duplicate stage, clock ports)",
        E5012 => "Invalid stage reference",
        E5013 => "Invalid stall/flush statement",
        E5014 => "Pipelined value needs an explicit scalar type",
        E5015 => "Combinational cycle through stage references",
        E5016 => "Stage-local value name is not unique across the pipeline",
        E5017 => "prev() used outside a contract or with invalid arguments (ADR-0040)",

        // ─── Budget and timing contracts ───
        E6001 => "Resource budget exceeded",
        E6003 => "@false_path could not be proven (the path actually exists)",
        E6004 => "@multicycle does not match the pipeline depth",

        // ─── Versioning ───
        E7001 => "SemVer violation: breaking change without a MAJOR bump",
        E7002 => "Interface changed without an abi_version bump",

        // ─── Simulation tests (ADR-0033) ───
        E8501 => "Unknown module instantiated in a test block",
        E8502 => "Unknown port in a test block",
        E8503 => "Write to a port that is not an input",
        E8504 => "Read from a port that is not an output",
        E8505 => "Invalid test builtin call",
        E8506 => "Undefined or duplicate instance name in a test block",
        E8507 => "Test data file not found or outside the project",
        E8508 => "Malformed hex data file",
        E8509 => "load() target is not a memory array",
        E8510 => "load() source does not fit the target array",
        E8511 => "Type mismatch in a test expression",
        E8512 => "Value does not fit in port width",
        E8513 => "Top-level port collides with the Verilator model API",

        // ─── Release discipline ───
        E9001 => "Release builds cannot contain todo!",
        E9002 => "Determinism violation",
        E9003 => "Register map drift between a driver file and the RTL (ADR-0063)",
        E9004 => "Not a Volt-generated register map file (ADR-0063)",

        // ─── Warnings ───
        W0010 => "Ambiguous operator precedence, parentheses recommended",
        W0020 => "Unknown attribute",
        W0021 => "Attribute is parsed but not yet enforced",
        W0022 => "Clock domain has no frequency; no create_clock emitted (ADR-0054)",
        W0023 => "Too many diagnostics; the rest are hidden (ADR-0068)",
        W1001 => "Unused signal / binding",
        W1002 => "Shadowing (same name in an inner scope)",
        W1003 => "Shadowing of a builtin name",
        W1004 => "Register written but never read",
        W1005 => "Unused import",
        W2010 => "Narrowing conversion (information loss)",
        W2011 => "Unused type parameter",
        W2012 => "Type not specified, default used",
        W2013 => "Shift amount exceeds the width (result is always 0)",
        W2014 => "Unreachable match arm (variant already covered)",
        W2020 => "Constant condition — the branch always takes the same path",
        W2021 => "Unused const declaration",
        W3001 => "Register is never written",
        W3002 => "Redundant sync() (same domain)",
        W3003 => "Multi-bit sync() — bit coherence is not guaranteed",
        W3004 => "Unused domain definition",
        W3005 => "PulseSync pulses need spacing in the destination domain",
        W3006 => "Same-address port collision is not detected (DualPortRam write-write, AsyncDualPortRam read-during-write)",
        W3007 => "External bidirectional signal read without synchronization (ADR-0051)",
        W3008 => "Deliberate trust downgrade (declassify) — review it (ADR-0052)",
        W3009 => "Asynchronous reset release assumed to be synchronized outside the unit (ADR-0065)",
        W3010 => "Synchronous reset shared by several clock domains (ADR-0065)",
        W4001 => "Signal unread in the netlist (reserved)",
        W4002 => "Register unread in the netlist (reserved)",
        W5001 => "Contract cannot be monitored in simulation; skipped (ADR-0064)",
    }
}
