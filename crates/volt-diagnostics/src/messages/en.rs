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

        // ─── Type inference (type-inference.md) ───
        E2001 => "Bit width mismatch",
        E2002 => "Signedness mismatch",
        E2003 => "Type mismatch (general)",
        E2004 => "Arithmetic on a bits<N> type",
        E2005 => "Literal width cannot be determined",
        E2006 => "Index/range out of bounds",
        E2007 => "Reversed range (hi < lo)",
        E2008 => "Variable range bound",
        E2009 => "Invalid cast",
        E2010 => "Literal does not fit the target type",
        E2011 => "Invalid Trit literal",
        E2012 => "Register type cannot be determined",

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

        // ─── Clock/reset domains (domain-inference.md) ───
        E3001 => "Clock domain mismatch (CDC)",
        E3002 => "Undefined clock domain",
        E3003 => "Reset domain mismatch (RDC)",
        E3004 => "Reset sequence violation",
        E3005 => "Conditional reset not satisfied",
        E3006 => "Power domain crossing without isolation [V1]",
        E3007 => "Power sequence violation [V1]",
        E3008 => "Missing retention [V1]",
        E3009 => "Information flow violation (trust_level) [V1]",
        E3010 => "Ambiguous domain (multiple clocks, no annotation)",
        E3011 => "Register written from more than one domain",
        E3012 => "Foreign-domain signal read inside an 'on' block",

        // ─── Connectivity/drivers (type-inference.md) ───
        E4001 => "Double driver",
        E4002 => "Undriven output port",
        E4003 => "Linear port consumed twice [V1]",
        E4004 => "Linear port never consumed [V1]",

        // ─── Behavioral contracts ───
        E5001 => "Contract violated (formal verification found a counterexample)",
        E5004 => "Contract expression is not Bool",

        // ─── Budget and timing contracts ───
        E6001 => "Resource budget exceeded",
        E6003 => "@false_path could not be proven (the path actually exists)",
        E6004 => "@multicycle does not match the pipeline depth",

        // ─── Versioning ───
        E7001 => "SemVer violation: breaking change without a MAJOR bump",
        E7002 => "Interface changed without an abi_version bump",

        // ─── Release discipline ───
        E9001 => "Release builds cannot contain todo!",
        E9002 => "Determinism violation",

        // ─── Warnings ───
        W0010 => "Ambiguous operator precedence, parentheses recommended",
        W0020 => "Unknown attribute",
        W0021 => "Unused doc comment",
        W1001 => "Unused signal / binding",
        W1002 => "Shadowing (same name in an inner scope)",
        W1003 => "Shadowing of a builtin name",
        W1004 => "Register written but never read",
        W1005 => "Unused import",
        W2010 => "Narrowing conversion (information loss)",
        W2011 => "Unused type parameter",
        W2012 => "Type not specified, default used",
        W2013 => "Shift amount exceeds the width (result is always 0)",
        W2020 => "Constant condition — the branch always takes the same path",
        W2021 => "Unused const declaration",
        W3001 => "Register is never written",
        W3002 => "Redundant sync() (same domain)",
        W3003 => "Multi-bit sync() — bit coherence is not guaranteed",
        W3004 => "Unused domain definition",
        W4001 => "Unused signal (silence with a '_' prefix)",
        W4002 => "Register written but never read",
    }
}
