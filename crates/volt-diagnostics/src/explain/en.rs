//! English (DEFAULT) long explanations for `volt explain` (§9).
//!
//! Terminology: docs/spec/GLOSSARY.md — BINDING. The `match` is
//! deliberately non-wildcard: adding a code forces both en.rs and
//! tr.rs to cover it. Code snippets, signal names and API names stay
//! untranslated in both columns (GLOSSARY.md §0).

use super::Explanation;
use crate::code::ErrorCode;

/// Long-form English explanation for `code` (cli-contract.md §9).
pub fn explanation(code: ErrorCode) -> Explanation {
    use ErrorCode::*;
    match code {
        // ─── Syntax (grammar-full.ebnf §18) ───
        E0001 => Explanation::new(
            "Unexpected token",
            "The parser found a token that is not valid at this position.",
            "The grammar defines exactly which tokens may follow each other. A stray token usually means a typo, a missing delimiter, or syntax borrowed from another language. The parser recovers at the next safe point, so later diagnostics may be follow-on noise — always fix the first error first.",
            "module Counter {\n    out count : u8 = = 0    // ✗ E0001: second '='\n}",
            "module Counter {\n    out count : u8 = 0      // ✓\n}",
        ),
        E0002 => Explanation::new(
            "Missing closing delimiter",
            "A '{', '(' or '[' was opened but never closed.",
            "Every bracket must be balanced. The diagnostic points at the spot where the parser expected the closing delimiter and at the opening one it belongs to. This is usually caused by deleting a line during editing or by a misplaced brace.",
            "module M {\n    in a : u1\n// ✗ E0002: '}' is missing",
            "module M {\n    in a : u1\n}               // ✓",
        ),
        E0003 => Explanation::new(
            "Reserved keyword",
            "This keyword is part of the Volt language but is not supported in this version yet.",
            "Volt reserves keywords for planned features so that code written today does not silently change meaning when the feature ships. Using a reserved keyword is an error until the corresponding language version supports it.",
            "trait Resettable {      // ✗ E0003: 'trait' is reserved\n}",
            "// Use the features of the current language version;\n// track the roadmap for when the keyword becomes available.",
        ),
        E0004 => Explanation::new(
            "Block end name does not match",
            "The optional name after a closing '}' must repeat the name of the block it closes.",
            "The trailing 'module <name>' label exists to keep long files readable: it documents which block the '}' closes. A mismatching name means either the label is stale after a rename or a brace is closing a different block than you think.",
            "module Counter {\n    // ...\n} module Timer          // ✗ E0004: 'Counter' expected",
            "module Counter {\n    // ...\n} module Counter        // ✓ (or omit the label)",
        ),
        E0005 => Explanation::new(
            "Invalid numeric literal",
            "The literal contains a digit or form that its base does not allow.",
            "Binary literals may only contain 0 and 1, octal 0-7, and decimal digits must not appear in a malformed prefix. A wrong digit is almost always a typo, but it can also mean the wrong base prefix was used.",
            "let mask = 0b1021       // ✗ E0005: '2' is not a binary digit",
            "let mask = 0b1011       // ✓",
        ),
        E0006 => Explanation::new(
            "'=' used in a sequential block",
            "Inside an 'on' block, registers are assigned with '<=', not '='.",
            "Sequential ('on') blocks describe what happens at a clock edge; the non-blocking '<=' makes every register update use the values from before the edge. Using '=' would suggest immediate update semantics that sequential hardware does not have, so Volt rejects it instead of silently reinterpreting it.",
            "on clk {\n    r = r + 1           // ✗ E0006\n}",
            "on clk {\n    r <= r + 1          // ✓\n}",
        ),
        E0007 => Explanation::new(
            "'<=' used in a combinational block",
            "Outside 'on' blocks, signals are assigned with '=', not '<='.",
            "Combinational assignments describe wiring that is always active, so the blocking '=' is the correct operator. '<=' is reserved for register updates inside 'on' blocks; mixing them up usually means the statement is in the wrong kind of block.",
            "wire y : u8\ny <= a + b              // ✗ E0007",
            "wire y : u8\ny = a + b               // ✓",
        ),
        E0008 => Explanation::new(
            "Missing 'else' in an 'if' expression",
            "An 'if' used as a value must say what the value is when the condition is false.",
            "In hardware, a value-producing 'if' without 'else' would have to remember its previous value — that is a latch, one of the most common sources of timing bugs and simulation/synthesis mismatch. Volt makes the latch impossible by requiring the 'else' branch.",
            "y = if enable { a }         // ✗ E0008: value when !enable?",
            "y = if enable { a } else { 0 }   // ✓",
        ),
        E0009 => Explanation::new(
            "Invalid attribute argument",
            "The attribute exists, but its argument has the wrong form or type.",
            "Each attribute defines the exact argument list it accepts. A wrong argument would either be ignored or mean something you did not intend, so Volt rejects it at parse time.",
            "@multicycle(\"two\")      // ✗ E0009: expects an integer\nreg r : u8 = 0",
            "@multicycle(2)          // ✓\nreg r : u8 = 0",
        ),
        E0010 => Explanation::new(
            "Comparison operators cannot be chained",
            "Expressions like 'a < b < c' are not allowed; write the two comparisons explicitly.",
            "In most languages 'a < b < c' silently parses as '(a < b) < c', comparing a boolean with a number — almost never what was meant. Volt refuses the chain outright so the intent must be spelled out.",
            "ok = a < b < c          // ✗ E0010",
            "ok = (a < b) && (b < c) // ✓",
        ),
        E0011 => Explanation::new(
            "Unexpected end of file",
            "The file ended in the middle of a declaration or block.",
            "The parser still expected more tokens — usually a closing brace or the rest of a declaration. This often comes from a truncated file, an unclosed block at the very end, or an editing accident.",
            "module M {\n    in a : u1\n    out y : u1\n// ✗ E0011: file ends here",
            "module M {\n    in a : u1\n    out y : u1\n    y = a\n}                       // ✓",
        ),
        E0012 => Explanation::new(
            "Invalid escape sequence",
            "The string contains a backslash escape that Volt does not define.",
            "Only a fixed set of escapes is meaningful. An unknown escape is usually a typo or a Windows path written with single backslashes; silently passing it through would corrupt the string.",
            "let s = \"col\\qrow\"     // ✗ E0012: '\\q' is not an escape",
            "let s = \"col\\tqrow\"    // ✓ ('\\t' tab)",
        ),
        E0013 => Explanation::new(
            "Unterminated block comment",
            "A '/*' comment was opened but its matching '*/' never appears.",
            "Everything after the '/*' is being swallowed as comment text, so the rest of the file effectively disappears. This often shows up as a confusing cascade of errors far from the real cause — the missing '*/'.",
            "/* explanation of the module\nmodule M {              // ✗ E0013: still inside the comment",
            "/* explanation of the module */\nmodule M {              // ✓",
        ),

        // ─── Name resolution (name-resolution.md) ───
        E1001 => Explanation::new(
            "Undefined name",
            "This name is not declared anywhere visible from this point.",
            "Every signal, constant, type or module must be declared before it can be referenced. The most common cause is a simple typo; the diagnostic suggests the closest existing name when one is similar enough.",
            "reg countr : u8 = 0\nresult = counter        // ✗ E1001: did you mean 'countr'?",
            "reg counter : u8 = 0\nresult = counter        // ✓",
        ),
        E1002 => Explanation::new(
            "Use before declaration",
            "The name exists in this scope, but it is used before the line that declares it.",
            "Item-level names are visible everywhere, but local declarations ('let', 'const' inside bodies) only take effect from their declaration point onward. Reading the value earlier would be reading something that does not exist yet.",
            "y = LIMIT               // ✗ E1002\nconst LIMIT : u32 = 8;",
            "const LIMIT : u32 = 8;\ny = LIMIT               // ✓",
        ),
        E1003 => Explanation::new(
            "Duplicate definition in the same scope",
            "Two declarations in the same scope use the same name.",
            "Within one scope every name must be unique — otherwise any reference to it would be ambiguous. If both declarations are intentional, rename one of them; shadowing in an *inner* scope is allowed (and reported separately as W1002).",
            "reg state : u2 = 0\nwire state : u2         // ✗ E1003",
            "reg state      : u2 = 0\nwire state_next : u2    // ✓",
        ),
        E1004 => Explanation::new(
            "Access to a private item",
            "The item exists but is not marked 'pub', so it is not visible from outside its module.",
            "Items are private by default so a module's internals can change without breaking users. If the item is meant to be part of the public interface, mark it 'pub' at its definition; otherwise access it through the module's public API.",
            "// in lib.volt:  const DEPTH : u32 = 4;\nuse lib::DEPTH          // ✗ E1004: DEPTH is private",
            "// in lib.volt:  pub const DEPTH : u32 = 4;\nuse lib::DEPTH          // ✓",
        ),
        E1005 => Explanation::new(
            "'::' used on a non-namespace item",
            "The path operator '::' only works on namespaces such as modules and enums.",
            "'a::b' means \"look up b inside the namespace a\". If 'a' is a signal or a value, there is nothing to look up inside it — the path is meaningless. This usually means the wrong name was used as the prefix.",
            "in data : u8\ny = data::first         // ✗ E1005: 'data' is a signal",
            "y = State::Idle         // ✓ ('State' is an enum)",
        ),
        E1006 => Explanation::new(
            "Cyclic module dependency",
            "Two or more modules import each other in a cycle.",
            "Name resolution processes modules in dependency order; a cycle makes that order undefined, and in hardware it usually signals a layering problem in the design. Break the cycle by moving the shared definitions into a third module both can import.",
            "// a.volt: use b::T\n// b.volt: use a::U     // ✗ E1006: a → b → a",
            "// common.volt: pub definitions used by both\n// a.volt and b.volt: use common::...   // ✓",
        ),
        E1007 => Explanation::new(
            "Enum variant not found",
            "The enum exists, but it has no variant with this name.",
            "Variant names are checked at compile time so that a typo cannot silently create a new state. Check the enum definition — the diagnostic lists a close match when one exists.",
            "enum State { Idle, Busy }\nnext = State::Idl       // ✗ E1007",
            "next = State::Idle      // ✓",
        ),
        E1008 => Explanation::new(
            "Struct field not found",
            "The struct type has no field with this name.",
            "Field access is resolved against the struct's declaration; an unknown field is usually a typo or an out-of-date use after the struct was refactored.",
            "struct Pkt { data : u8, valid : bool }\nb = pkt.vaild           // ✗ E1008",
            "b = pkt.valid           // ✓",
        ),
        E1009 => Explanation::new(
            "Module port not found",
            "The instantiation connects a port name that the module does not declare.",
            "Port connections are checked by name against the module declaration, so a renamed or misspelled port is caught here instead of leaving a dangling wire in the netlist.",
            "// module Fifo { in push : bool, ... }\nFifo { psuh: enq, ... } // ✗ E1009",
            "Fifo { push: enq, ... } // ✓",
        ),
        E1010 => Explanation::new(
            "Ambiguous import",
            "Two 'use' declarations bring in different items under the same name.",
            "When both imports are in scope, a bare reference to the name could mean either item — Volt refuses to guess. Disambiguate by renaming one import or by using the full path at the use site.",
            "use fifo::Config\nuse uart::Config        // ✗ E1010: which 'Config'?",
            "use fifo::Config as FifoConfig\nuse uart::Config as UartConfig   // ✓",
        ),

        // ─── Type inference (type-inference.md) ───
        E2001 => Explanation::new(
            "Bit width mismatch",
            "The two sides of this connection have different bit widths.",
            "Implicit width changes silently drop or invent bits — a classic source of hardware bugs that only appear with large values. Volt never resizes implicitly: widening and narrowing must both be written out with 'as'.",
            "in  a : u8\nout y : u16\ny = a                   // ✗ E2001: 8 vs 16 bits",
            "y = a as u16            // ✓ explicit widening",
        ),
        E2002 => Explanation::new(
            "Signedness mismatch",
            "A signed and an unsigned value are combined without an explicit conversion.",
            "The same bit pattern means different numbers as signed vs unsigned (0xFF is 255u8 but -1i8). Mixing them implicitly would make comparisons and arithmetic surprising, so the conversion must be spelled out with 'as'.",
            "in  a : i8\nin  b : u8\ny = a + b               // ✗ E2002",
            "y = a + (b as i8)       // ✓ (make the intent explicit)",
        ),
        E2003 => Explanation::new(
            "Type mismatch",
            "The expression's type does not match what this position requires.",
            "Each context expects a specific type: a condition needs bool, a port connection needs the port's declared type. Passing something else is rejected instead of being coerced, because coercion rules are exactly where subtle bugs hide.",
            "in  count : u8\ny = if count { a } else { b }   // ✗ E2003: u8 is not bool",
            "y = if count != 0 { a } else { b }   // ✓",
        ),
        E2004 => Explanation::new(
            "Arithmetic on a bits<N> type",
            "bits<N> is a raw bit container; it has no numeric meaning, so '+', '-', '*' do not apply.",
            "A bits<N> value might encode a number, a bitmask, or a set of flags — the type deliberately does not say. Arithmetic requires numeric intent, so first cast to an unsigned or signed integer of the same width; bitwise operators (&, |, ^) work on bits<N> directly.",
            "in  b : bits<8>\ny = b + 1               // ✗ E2004",
            "y = (b as u8) + 1       // ✓",
        ),
        E2005 => Explanation::new(
            "Literal width cannot be determined",
            "There is no context from which this literal's bit width can be inferred.",
            "Every hardware value must have a definite width. A literal usually takes its width from the surrounding context (the port or register it is assigned to); when there is no such context, state the type explicitly.",
            "let x = 5               // ✗ E2005: 5 as how many bits?",
            "let x : u8 = 5          // ✓",
        ),
        E2006 => Explanation::new(
            "Index or range out of bounds",
            "The index or range reaches past the width of the value.",
            "A u8 has bits 0 through 7 — reading bit 8 would read hardware that does not exist. Out-of-range selects are always a design error, so they are rejected at compile time rather than producing undefined wiring.",
            "in  a : u8\nb = a[8]                // ✗ E2006: valid bits are 0..7",
            "b = a[7]                // ✓ (most significant bit)",
        ),
        E2007 => Explanation::new(
            "Reversed range",
            "The range's end is smaller than its start.",
            "Ranges are written low-to-high ('start..end' with start ≤ end). A reversed range selects nothing meaningful and is almost always the two bounds swapped by accident.",
            "b = a[5..2]             // ✗ E2007",
            "b = a[2..5]             // ✓",
        ),
        E2008 => Explanation::new(
            "Variable range bound",
            "Range bounds must be compile-time constants, not runtime signals.",
            "A bit select determines physical wiring, and wiring cannot change at runtime. If you need a runtime-selected portion, use a shift and a fixed-width select, or a mux over constant ranges.",
            "in  n : u3\nb = a[0..n]             // ✗ E2008: 'n' is a signal",
            "b = (a >> n)[0..4]      // ✓ shift, then constant range",
        ),
        E2009 => Explanation::new(
            "Invalid cast",
            "'as' cannot convert between these two types.",
            "Casts are only defined between numeric/bit types of matching structure (u/i/bits). Converting a bool or a clock into a number, or vice versa, has no single obvious meaning — express the intent with an explicit expression instead.",
            "in  ck : clock\ny = ck as u1            // ✗ E2009: clocks are not data",
            "y = if flag { 1 } else { 0 }    // ✓ (bool → number, explicit)",
        ),
        E2010 => Explanation::new(
            "Literal does not fit the target type",
            "The literal's value is outside the range this type can represent.",
            "A u4 can hold 0..15; assigning 200 would silently keep only the low bits in other languages. Volt rejects it: either widen the type or fix the value.",
            "let x : u4 = 200        // ✗ E2010: u4 max is 15",
            "let x : u8 = 200        // ✓",
        ),
        E2011 => Explanation::new(
            "Invalid Trit literal",
            "A Trit (balanced ternary digit) can only be -1, 0 or +1.",
            "Trit is Volt's balanced-ternary digit type used by ternary arithmetic blocks. Any other value has no ternary encoding, so the literal is rejected at compile time.",
            "let t : Trit = 2        // ✗ E2011",
            "let t : Trit = 1        // ✓ (-1, 0, +1 are valid)",
        ),
        E2012 => Explanation::new(
            "Register type cannot be determined",
            "The register has neither a type annotation nor an initial value to infer one from.",
            "A register's width defines real flip-flops, so it must be known at compile time. Give the register an explicit type, or an initial value whose type is unambiguous.",
            "reg r                   // ✗ E2012: width unknown",
            "reg r : u8 = 0          // ✓",
        ),

        // ─── Constant evaluation (const-eval.md) ───
        E2020 => Explanation::new(
            "Cyclic constant dependency",
            "Evaluating this constant requires its own value.",
            "Constants are computed at compile time in dependency order; a cycle has no well-defined result. Break the cycle by computing one of the values from independent inputs.",
            "const A : u32 = B + 1;\nconst B : u32 = A + 1;  // ✗ E2020: A → B → A",
            "const A : u32 = 4;\nconst B : u32 = A + 1;  // ✓",
        ),
        E2021 => Explanation::new(
            "Constant expression expected",
            "This position needs a compile-time value, but the expression depends on a runtime signal.",
            "Type widths, array sizes and range bounds shape the hardware itself, so they must be fixed before the design is built. Replace the signal with a 'const', or restructure so the varying part happens at runtime on fixed-size hardware.",
            "in  n : u8\nwire buf : bits<n>      // ✗ E2021: 'n' is runtime data",
            "const N : u32 = 8;\nwire buf : bits<N>      // ✓",
        ),
        E2022 => Explanation::new(
            "Compile-time overflow",
            "Evaluating this constant expression overflows its type.",
            "Constant evaluation uses the declared type's exact range — the same range the hardware will have. An overflow at compile time means the value could never exist in hardware, so it is an error, not a silent wrap.",
            "const X : u8 = 250 + 10;    // ✗ E2022: 260 > 255",
            "const X : u16 = 250 + 10;   // ✓",
        ),
        E2023 => Explanation::new(
            "Division by zero",
            "A constant expression divides (or takes a remainder) by zero.",
            "Division by zero has no value, at compile time or in hardware. This often appears indirectly, when the divisor is another constant that works out to zero — check the chain of constants feeding this expression.",
            "const STEP : u32 = 8 / (DEPTH - DEPTH);  // ✗ E2023",
            "const STEP : u32 = 8 / DEPTH;            // ✓ (DEPTH > 0)",
        ),
        E2024 => Explanation::new(
            "Invalid shift amount",
            "The constant shift amount is negative or not smaller than the value's width.",
            "Shifting a u8 by 8 or more always yields 0 (and negative shifts are undefined), so a constant shift outside 0..width is certainly a mistake — typically a wrong width constant or an off-by-one.",
            "const Y : u8 = X << 9;  // ✗ E2024: u8 allows shifts 0..7",
            "const Y : u8 = X << 3;  // ✓",
        ),
        E2025 => Explanation::new(
            "Invalid width",
            "A width must be a positive integer within the implementation limit.",
            "bits<0> would be hardware with no wires, and a negative or enormous width has no physical meaning. Widths are usually computed from other constants — check that computation for an underflow or a wrong operand.",
            "wire w : bits<0>        // ✗ E2025",
            "wire w : bits<8>        // ✓",
        ),
        E2026 => Explanation::new(
            "Array size limit exceeded",
            "The declared array is larger than the implementation limit.",
            "A huge array size is almost always a miscomputed constant (for example a subtraction that wrapped around). The limit protects the compiler and downstream tools from being handed an impossible design.",
            "wire mem : [u8; 1 << 40]    // ✗ E2026",
            "wire mem : [u8; 1024]       // ✓",
        ),
        E2027 => Explanation::new(
            "Loop unrolling limit exceeded",
            "This compile-time 'for' loop expands past the unrolling limit.",
            "Every iteration of a 'for' becomes real hardware, so a loop of a million iterations is a million copies of the body. Exceeding the limit usually means the bound is a wrong constant; if the design genuinely needs that much hardware, restructure it into a memory or a sequential process.",
            "for i in 0..10_000_000 {    // ✗ E2027\n    t[i] = d[i]\n}",
            "for i in 0..WIDTH {         // ✓ bounded by a small const\n    t[i] = d[i]\n}",
        ),
        E2028 => Explanation::new(
            "Invalid range (end < start)",
            "The constant range's end is smaller than its start.",
            "Ranges iterate upward; with end < start there is nothing to iterate and the bounds are almost certainly swapped. Note that an empty range written intentionally (start == end) is allowed — only reversed ones are rejected.",
            "for i in 8..0 {         // ✗ E2028\n    t[i] = d[i]\n}",
            "for i in 0..8 {         // ✓\n    t[i] = d[i]\n}",
        ),
        E2029 => Explanation::new(
            "Constant array index out of bounds",
            "This compile-time index is outside the array's bounds.",
            "The array's size and the index are both known at compile time, so the out-of-bounds access is certain — not a possibility. It usually comes from a loop bound that does not match the array size.",
            "wire t : [u8; 4]\ny = t[4]                // ✗ E2029: valid indices 0..3",
            "y = t[3]                // ✓",
        ),

        // ─── Clock/reset domains (domain-inference.md) ───
        E3001 => Explanation::new(
            "Clock Domain Crossing (CDC) violation",
            "Signals in two different clock domains cannot be connected directly.",
            "If the destination flip-flop captures the source signal inside its setup or hold window, it goes metastable: the output stays unstable for a while and then settles to a random 0 or 1.\n\nThis compiles silently in Verilog and typically surfaces in silicon — the most expensive place to debug. Volt makes the crossing a compile error instead.",
            "domain Fast { clock = posedge }\ndomain Slow { clock = posedge }\n\nmodule Bad {\n    in  data   : u8 @Fast\n    out result : u8 @Slow\n\n    result = data          // ✗ E3001\n}",
            "result = sync(data, slow_clk)    // ✓ two flip-flops",
        )
        .with_note("sync() synchronizes each bit independently. For multi-bit data the bits may be captured on different clock edges (0b11111111 → 0b11110000 as an invalid intermediate value). For multi-bit crossings use Gray coding (counters), AsyncFifo (data streams) or a handshake protocol (control).")
        .with_docs(&["https://volthdl.org/guide/cdc"]),
        E3002 => Explanation::new(
            "Undefined clock domain",
            "The '@' annotation names a domain that is never declared.",
            "Every domain annotation must refer to a 'domain' declaration so the compiler knows its clock and reset behaviour. An unknown domain name is usually a typo, or the declaration lives in a module that was not imported.",
            "module M {\n    in data : u8 @Fasst    // ✗ E3002: no 'domain Fasst'\n}",
            "domain Fast { clock = posedge }\nmodule M {\n    in data : u8 @Fast     // ✓\n}",
        )
        .with_docs(&["https://volthdl.org/guide/cdc"]),
        E3003 => Explanation::new(
            "Reset domain mismatch (RDC)",
            "The signal crosses between registers that use different, unsynchronized resets.",
            "When the source domain's reset asserts, the destination register can capture the value mid-change — the same metastability risk as a clock crossing, but triggered by reset. Reset Domain Crossings are as real as CDCs and are checked the same way.",
            "// src register: reset = rst_a, dst register: reset = rst_b\ndst <= src              // ✗ E3003",
            "dst <= sync(src, dst_clk)    // ✓ synchronize the crossing",
        )
        .with_docs(&["https://volthdl.org/guide/cdc"]),
        E3004 => Explanation::new(
            "Reset sequence violation",
            "A register leaves reset before a domain it depends on has been released.",
            "Reset release order is a contract: if consumer logic wakes up before its producer, it processes garbage from the not-yet-reset side. The declared reset sequence must match the dependency direction of the data flow.",
            "// Consumer released at step 1, its producer at step 2\n// ✗ E3004: consumer wakes before producer",
            "// Release the producer domain first, then the consumer\n// ✓ order matches the data flow",
        ),
        E3005 => Explanation::new(
            "Conditional reset not satisfied",
            "On some path this register is reset only under a condition that is not guaranteed.",
            "A register that declares a reset value must actually receive it whenever reset asserts. If the reset assignment sits under an additional 'if', there are reset cycles where the register keeps its old (unknown) value — the declared reset value is a lie.",
            "on clk {\n    if mode == 0 {\n        r <= 0          // ✗ E3005: reset only when mode == 0\n    }\n}",
            "on clk {\n    r <= 0              // ✓ unconditional reset value\n}",
        ),
        E3006 => Explanation::new(
            "Power domain crossing without isolation",
            "A signal leaves a switchable power domain without an isolation cell.",
            "When the source domain powers down, its outputs float to undefined levels; without isolation (clamp_low, clamp_high or latch) the receiving logic reads garbage. Every crossing out of a switchable domain must state its isolation behaviour.",
            "// src in switchable domain PD1, no isolation\ny = src                 // ✗ E3006",
            "@isolate(clamp_low)\ny = src                 // ✓ defined value while PD1 is off",
        )
        .with_note("Power domains are a V1 feature; this check is inactive in F-series versions."),
        E3007 => Explanation::new(
            "Power sequence violation",
            "A domain is powered up or down out of the declared order.",
            "Power domains depend on each other: an island must not wake before the rails and domains it relies on are stable. The declared power sequence is checked against the dependency graph, and violations are rejected at compile time.",
            "// PD2 depends on PD1 but powers up first\n// ✗ E3007",
            "// Power-up order: PD1 → PD2\n// ✓ matches the declared dependency",
        )
        .with_note("Power domains are a V1 feature; this check is inactive in F-series versions."),
        E3008 => Explanation::new(
            "Missing retention",
            "State in a switchable power domain is lost on power-down but read after power-up.",
            "When a domain powers down, its registers lose their contents. If the design reads that state after wake-up, the registers need retention cells (or the state must be rebuilt explicitly). Declaring neither is an error, because the post-wake value would be undefined.",
            "// reg cfg lives in switchable PD1, read after wake\nreg cfg : u8 = 0        // ✗ E3008",
            "@retain\nreg cfg : u8 = 0        // ✓ value survives power-down",
        )
        .with_note("Power domains are a V1 feature; this check is inactive in F-series versions."),
        E3009 => Explanation::new(
            "Information flow violation (trust_level)",
            "Data flows from a high-trust source into a lower-trust sink without declassification.",
            "trust_level annotations let the compiler track where secret or privileged data may flow. A direct assignment from high to low would leak information; the flow must pass through an explicit declassify point that documents the decision.",
            "// key: trust_level = secret, dbg: trust_level = public\ndbg = key               // ✗ E3009",
            "dbg = declassify(key.parity())  // ✓ explicit, reviewed leak",
        )
        .with_note("Information-flow checking is a V1 feature; this check is inactive in F-series versions."),
        E3010 => Explanation::new(
            "Ambiguous domain",
            "The module has multiple clock domains and this signal does not say which one it belongs to.",
            "With a single clock everything is inferred automatically and you never see domains. As soon as two domains exist, an unannotated signal could belong to either — and guessing wrong would hide a real CDC. Annotate the signal with '@Domain'.",
            "module M {\n    in a : u8 @Fast\n    in b : u8 @Slow\n    wire t : u8         // ✗ E3010: @Fast or @Slow?\n}",
            "    wire t : u8 @Fast   // ✓ stated explicitly",
        )
        .with_docs(&["https://volthdl.org/guide/cdc"]),
        E3011 => Explanation::new(
            "Register written from more than one domain",
            "Two 'on' blocks in different clock domains write the same register.",
            "A flip-flop has exactly one clock input; writing it from two domains is physically impossible to synthesize faithfully and simulates as a race. Keep the register in one domain and bring the other domain's data across with sync() or a FIFO.",
            "on fast_clk { r <= a }\non slow_clk { r <= b }  // ✗ E3011",
            "on fast_clk {\n    r <= if sel { sync(b, fast_clk) } else { a }   // ✓ one domain\n}",
        )
        .with_docs(&["https://volthdl.org/guide/cdc"]),
        E3012 => Explanation::new(
            "Foreign-domain signal read inside an 'on' block",
            "The 'on' block's clock belongs to one domain, but the expression reads a signal from another.",
            "Reading a foreign-domain signal at this clock's edge is a hidden CDC — the value can change exactly while it is being sampled. The read must go through sync() (single bit) or a proper multi-bit bridge first.",
            "on slow_clk {\n    r <= fast_data      // ✗ E3012: fast_data is @Fast\n}",
            "on slow_clk {\n    r <= sync(fast_data, slow_clk)   // ✓\n}",
        )
        .with_docs(&["https://volthdl.org/guide/cdc"]),

        // ─── Connectivity/drivers (type-inference.md) ───
        E4001 => Explanation::new(
            "Double driver",
            "The same signal is driven by two assignments.",
            "Two drivers on one wire is an electrical short: whenever they disagree, the result is contention, not a value. Combine the sources into a single assignment (a mux or priority expression) so exactly one value wins at any time.",
            "y = a\ny = b                   // ✗ E4001: who wins?",
            "y = if sel { b } else { a }  // ✓ single driver",
        ),
        E4002 => Explanation::new(
            "Undriven output port",
            "This output port is declared but never assigned.",
            "An undriven output floats: downstream logic reads an undefined level. If the port is genuinely unused, remove it from the interface; otherwise the missing assignment is a bug in this module.",
            "module M {\n    out y : u8          // ✗ E4002: never assigned\n}",
            "module M {\n    out y : u8\n    y = 0               // ✓ (or a real value)\n}",
        ),
        E4003 => Explanation::new(
            "Linear port consumed twice",
            "A port with a linear type (&inv T) is used in more than one place.",
            "Linear types encode use-exactly-once resources — for example a bus grant or a token that must have a single consumer. Consuming it twice would duplicate the resource. Route the single consumption through whichever component truly owns it.",
            "// grant : &inv Token\na = grant\nb = grant               // ✗ E4003: second consumption",
            "a = grant               // ✓ exactly one consumer",
        )
        .with_note("Linear types are a V1 feature; this check is inactive in F-series versions."),
        E4004 => Explanation::new(
            "Linear port never consumed",
            "A port with a linear type (&inv T) is never used.",
            "A linear value must be consumed exactly once — dropping it silently would lose the resource it represents (a token, a grant, a one-shot handle). If the port is genuinely unneeded, remove it from the interface instead of ignoring it.",
            "// grant : &inv Token, never referenced\n// ✗ E4004",
            "sink = grant            // ✓ consumed exactly once",
        )
        .with_note("Linear types are a V1 feature; this check is inactive in F-series versions."),

        // ─── Behavioral contracts ───
        E5001 => Explanation::new(
            "Contract violated",
            "Formal verification found an execution that breaks a contract of this module.",
            "A contract (invariant/ensures/assert) is a promise about every reachable state of the design. 'volt verify' asked SymbiYosys to prove it; instead the solver constructed a concrete input sequence — a counterexample — that drives the design into a state where the contract is false. This is not a tool artifact: the RTL as written really can reach that state.\n\nInspect the counterexample waveform (.vcd) to see the exact cycle-by-cycle path, then either fix the logic or, if the scenario is genuinely impossible in the real environment, exclude it with a 'requires'/'assume' contract on the inputs.",
            "module Ctrl {\n    invariant: !(busy && done)   // ✗ E5001: violated at cycle 7\n}",
            "// 1) Fix the logic so busy and done are never high together, or\n// 2) constrain the environment:\nrequires: !(start && abort)",
        )
        .with_note(
            "The counterexample .vcd is written next to the .sby file under build/formal/. Open it with 'gtkwave' or 'surfer'. BMC only explores up to --depth cycles; a pass at depth N is not a full proof — use --mode prove for unbounded induction.",
        )
        .with_docs(&["https://volthdl.org/guide/verify"]),
        E5004 => Explanation::new(
            "Contract expression is not Bool",
            "requires/ensures/invariant/cover/assert/assume conditions must be Bool expressions.",
            "A contract states a property that either holds or does not; only a Bool expression carries that meaning. A numeric expression such as 'speed + 1' has no truth value, so the compiler cannot turn it into an assertion, an assumption or a coverage goal.",
            "module M {\n    in speed : u8\n    requires: speed + 1     // ✗ E5004: type is u9, not bool\n}",
            "module M {\n    in speed : u8\n    requires: speed <= 2    // ✓ comparison yields bool\n}",
        ),

        // ─── Budget and timing contracts ───
        E6001 => Explanation::new(
            "Resource budget exceeded",
            "The module uses more of a declared resource (LUTs, registers, BRAM) than its @budget allows.",
            "Budgets are contracts: they let a team partition a chip and catch growth early, at compile time, instead of at the final fitting stage. If the overrun is legitimate, raise the budget consciously in one reviewed place — do not delete the annotation.",
            "@budget(regs = 100)\nmodule M { /* needs 140 registers */ }   // ✗ E6001",
            "@budget(regs = 150)     // ✓ consciously raised\nmodule M { /* ... */ }",
        ),
        E6003 => Explanation::new(
            "@false_path could not be proven",
            "The attribute claims this path never carries data, but analysis found it actually does.",
            "@false_path tells timing analysis to ignore a path; if the path is real, ignoring it hides a genuine timing violation in silicon. Volt only accepts the attribute when it can prove the path is unreachable — otherwise fix the logic or remove the claim.",
            "@false_path(from = a, to = y)\ny = if sel { a } else { b }     // ✗ E6003: a reaches y",
            "// Either make the path truly unreachable, or\ny = b                            // ✓ claim now provable",
        ),
        E6004 => Explanation::new(
            "@multicycle does not match the pipeline depth",
            "The declared multicycle count disagrees with the actual number of register stages on the path.",
            "@multicycle(N) relaxes timing on the promise that data needs N cycles to traverse the path. If the real pipeline is shallower, the relaxed check hides a violation; if deeper, the constraint is wasted. The declaration must match the structure.",
            "@multicycle(2)\n// path actually has 3 register stages   // ✗ E6004",
            "@multicycle(3)          // ✓ matches the pipeline",
        ),

        // ─── Versioning ───
        E7001 => Explanation::new(
            "SemVer violation: breaking change without a MAJOR bump",
            "The public interface changed incompatibly, but the package version only bumped MINOR or PATCH.",
            "Consumers pin against MAJOR versions; an incompatible port or type change under the same MAJOR silently breaks their builds or, worse, their hardware. Either restore compatibility or bump the MAJOR version.",
            "// v1.2.0 → v1.3.0 while removing port 'ready'\n// ✗ E7001",
            "// v1.2.0 → v2.0.0 with the removal documented\n// ✓",
        ),
        E7002 => Explanation::new(
            "Interface changed without an abi_version bump",
            "The module's wire-level interface changed but abi_version stayed the same.",
            "abi_version is what other teams' netlists and constraint files key on. Any change to ports, widths or timing contracts must bump it, so downstream integrations fail loudly at integration time instead of mysteriously at runtime.",
            "// port width u8 → u16, abi_version still 3\n// ✗ E7002",
            "@abi_version(4)         // ✓ bumped with the change",
        ),

        // ─── Release discipline ───
        E9001 => Explanation::new(
            "Release builds cannot contain todo!",
            "A todo! placeholder is still present while building in release mode.",
            "todo! marks logic that is intentionally unfinished — in simulation it traps, but in a release netlist it would become real, undefined hardware. Release builds refuse to proceed until every todo! is implemented or the feature is cut for real.",
            "on clk {\n    r <= todo!(\"CRC\")   // ✗ E9001 in --release\n}",
            "on clk {\n    r <= crc8(data)     // ✓ implemented\n}",
        ),
        E9002 => Explanation::new(
            "Determinism violation",
            "The build depends on something that changes between runs (time, randomness, environment).",
            "The same source must always produce bit-identical output — that is what makes hardware reviews, caching and sign-off trustworthy. Timestamps, random seeds and environment lookups break reproducibility, so they are rejected in the build path.",
            "const SEED : u32 = now();   // ✗ E9002: differs every build",
            "const SEED : u32 = 0xC0FFEE;    // ✓ fixed and reviewable",
        ),

        // ─── Warnings ───
        W0010 => Explanation::new(
            "Ambiguous operator precedence",
            "This expression mixes operators whose relative precedence is easy to misread.",
            "The compiler knows the precedence, but the next reader may not — and precedence bugs survive review precisely because the code 'looks right'. Volt asks for parentheses in the known-treacherous combinations (shift with arithmetic, bitwise with comparison).",
            "y = a & b == c          // ⚠ W0010: '==' binds before '&'",
            "y = a & (b == c)        // ✓ intent is visible",
        ),
        W0020 => Explanation::new(
            "Unknown attribute",
            "No attribute with this name exists; it is ignored.",
            "Attributes carry real semantics (timing, budgets, retention). A misspelled attribute silently does nothing — which for something like @false_path means a constraint you think exists, does not. Check the spelling against the attribute list.",
            "@multicyle(2)           // ⚠ W0020: typo, ignored\nreg r : u8 = 0",
            "@multicycle(2)          // ✓\nreg r : u8 = 0",
        ),
        W0021 => Explanation::new(
            "Unused doc comment",
            "This doc comment is not attached to any item.",
            "Doc comments ('///') document the item that immediately follows them. A doc comment followed by a blank stretch, an inner statement, or the end of a block documents nothing and will not appear in generated documentation. Move it directly above its item, or make it a regular comment.",
            "/// Counts events.\n\n// (blank line breaks the attachment)  ⚠ W0021\nmodule Counter { }",
            "/// Counts events.\nmodule Counter { }      // ✓",
        ),
        W1001 => Explanation::new(
            "Unused signal or binding",
            "This name is declared but never read.",
            "Dead declarations accumulate and hide real signals in reviews. If the value is intentionally unused (documentation, partial implementation), prefix the name with '_' to state that explicitly; otherwise delete it.",
            "let scratch = a + b     // ⚠ W1001: never read",
            "let _scratch = a + b    // ✓ explicitly unused (or delete it)",
        ),
        W1002 => Explanation::new(
            "Shadowing",
            "An inner-scope declaration reuses a name that is already visible.",
            "The inner name hides the outer one for the rest of the scope — legal, but a frequent source of 'why is my value wrong' confusion, especially in long blocks. Rename one of them if the two values are genuinely different things.",
            "let limit = 8\nif en {\n    let limit = 4       // ⚠ W1002: hides outer 'limit'\n}",
            "let limit = 8\nif en {\n    let fast_limit = 4  // ✓ distinct name\n}",
        ),
        W1003 => Explanation::new(
            "Shadowing of a builtin name",
            "This declaration reuses the name of a builtin function or type.",
            "After this line, 'sync' (or another builtin) refers to your local value — any later call to the builtin in this scope silently resolves to the wrong thing, and CDC helpers like sync() are exactly where that hurts. Pick a name that does not collide.",
            "let sync = a & b        // ⚠ W1003: hides builtin sync()",
            "let sync_mask = a & b   // ✓",
        ),
        W1004 => Explanation::new(
            "Register written but never read",
            "This register is assigned, but its value is never used anywhere.",
            "Flip-flops that feed nothing are dead state: they cost area and power, and they usually mean the consuming logic was removed or renamed while the producer stayed behind. Delete the register, or reconnect the logic that was supposed to read it.",
            "reg dbg : u8 = 0\non clk { dbg <= data }  // ⚠ W1004: nobody reads dbg",
            "// Either delete it, or actually use it:\nresult = dbg            // ✓",
        ),
        W1005 => Explanation::new(
            "Unused import",
            "This 'use' declaration brings in a name that is never referenced.",
            "Stale imports suggest dependencies that no longer exist and slow down readers scanning the header. Remove the 'use'; if the dependency is coming back soon, a comment says that better than a dead import.",
            "use fifo::AsyncFifo     // ⚠ W1005: never used",
            "// (removed)            // ✓",
        ),
        W2010 => Explanation::new(
            "Narrowing conversion",
            "This 'as' cast drops high bits — information is lost.",
            "The cast is explicit, so this is only a warning, but the discarded bits are gone: u16 → u8 keeps only the low byte. If the value can genuinely exceed the target range, mask or saturate deliberately so the behaviour is documented.",
            "in  big : u16\nsmall = big as u8       // ⚠ W2010: top 8 bits dropped",
            "small = (big & 0xFF) as u8   // ✓ truncation is explicit",
        ),
        W2011 => Explanation::new(
            "Unused type parameter",
            "The generic parameter is declared but not used by any port, signal or expression.",
            "An unused parameter still forces every instantiation to supply a value, widening the interface for nothing. Remove it, or wire it into the logic it was meant to configure.",
            "module Fifo<DEPTH> {    // ⚠ W2011: DEPTH unused\n    in d : u8\n}",
            "module Fifo<DEPTH> {\n    wire mem : [u8; DEPTH]   // ✓ actually used\n}",
        ),
        W2012 => Explanation::new(
            "Type not specified, default used",
            "No type was given here, so the language default was applied.",
            "The default keeps quick sketches short, but in lasting code an implicit width is a review hazard — the reader must know the default to know the hardware. State the type once the code is meant to stay.",
            "const N = 8;            // ⚠ W2012: default type used",
            "const N : u32 = 8;      // ✓ explicit",
        ),
        W2013 => Explanation::new(
            "Shift amount exceeds the width",
            "Shifting by at least the value's width always produces 0.",
            "The expression is legal but constant: every bit is shifted out. This almost always means a wrong width assumption or a shift amount computed from the wrong constant.",
            "in  a : u8\ny = a << 8              // ⚠ W2013: result is always 0",
            "y = a << 3              // ✓ (shift < 8)",
        ),
        W2020 => Explanation::new(
            "Constant condition",
            "This condition always evaluates to the same value, so the branch never changes.",
            "One arm of the 'if' is dead hardware. Sometimes that is intentional (configuration via const), but often the condition compares the wrong constants or a signal that cannot vary. Verify the inputs; if it is configuration, the warning documents the frozen branch.",
            "if WIDTH > 0 { y = a }  // ⚠ W2020: WIDTH is const 8, always true",
            "y = a                   // ✓ say it directly",
        ),
        W2021 => Explanation::new(
            "Unused const declaration",
            "This constant is never referenced.",
            "Dead constants mislead readers into searching for their uses and often survive from removed features. Delete it, or if it documents a protocol value kept for reference, say so in a comment.",
            "const RETRIES : u32 = 3;    // ⚠ W2021: never used",
            "// (removed)                // ✓",
        ),
        W3001 => Explanation::new(
            "Register is never written",
            "This register is read, but no 'on' block ever assigns it.",
            "The register holds its reset value forever — the reading logic is consuming a constant while looking like it consumes state. Either the write logic is missing, or the register should be replaced by a const.",
            "reg state : u2 = 0\ny = state               // ⚠ W3001: state never written",
            "on clk { state <= next }    // ✓ write logic added",
        ),
        W3002 => Explanation::new(
            "Redundant sync()",
            "Both sides of this sync() are in the same clock domain.",
            "sync() exists to bridge domains; within one domain it only adds two cycles of latency and two flip-flops of area for nothing. Remove it — or, if a crossing was intended, check which domain each side actually lives in.",
            "// data and clk are both @Fast\ny = sync(data, clk)     // ⚠ W3002: same domain",
            "y = data                // ✓ direct, no extra latency",
        ),
        W3003 => Explanation::new(
            "Multi-bit sync()",
            "sync() is applied to a multi-bit signal; bit coherence is not guaranteed.",
            "Each bit synchronizes independently, so during a change the receiver can observe mixtures of old and new bits (0b1111 → 0b1100 for one cycle). For counters use Gray coding, for data streams an AsyncFifo, for control a handshake — sync() alone is only safe for single bits.",
            "slow_bus = sync(fast_bus, slow_clk)   // ⚠ W3003: 8 bits",
            "slow_bus = AsyncFifo { push: fast_bus, ... }   // ✓",
        )
        .with_docs(&["https://volthdl.org/guide/cdc"]),
        W3004 => Explanation::new(
            "Unused domain definition",
            "This 'domain' is declared but no signal or block belongs to it.",
            "An unused domain usually remains from a removed clock or an unfinished integration. It costs nothing in hardware but misleads readers about how many clocks the design has. Delete it, or annotate the signals that were supposed to live in it.",
            "domain Debug { clock = posedge }    // ⚠ W3004: nothing uses it",
            "// (removed)                        // ✓",
        ),
        W3005 => Explanation::new(
            "PulseSync minimum pulse spacing",
            "PulseSync uses a toggle protocol; source pulses that arrive too close together are swallowed.",
            "PulseSync converts each source pulse into a level toggle, synchronizes the toggle with two flops in the destination domain, and re-derives a pulse by edge detection. If a second source pulse flips the toggle back before the destination has sampled the first flip, the destination sees no edge at all and BOTH pulses are lost. The clock ratio is not known at compile time, so the compiler cannot prove the spacing; it reminds you of the usage constraint instead: keep at least 3 destination clock cycles between consecutive source pulses, or use AsyncFifo/HandshakeSync for bursts.",
            "let ps = PulseSync { src_clk: fast_clk, pulse_in: p, dst_clk: slow_clk }   // ⚠ W3005",
            "// guarantee >= 3 dst_clk cycles between pulses, or:\nlet hs = HandshakeSync<u8> { ... }   // ✓ flow control built in",
        ),
        W4001 => Explanation::new(
            "Unused signal",
            "This signal is declared in the netlist but drives nothing.",
            "After elaboration the signal has no readers, so synthesis will prune it — along with any logic feeding only it. If keeping it is intentional (debug probe, reserved pin), prefix the name with '_' to silence the warning explicitly.",
            "wire spare : u4         // ⚠ W4001: no readers",
            "wire _spare : u4        // ✓ explicitly kept",
        ),
        W4002 => Explanation::new(
            "Register written but never read (netlist)",
            "After elaboration, nothing in the final netlist observes this register's value.",
            "Unlike W1004 (which looks at the source), this check runs on the elaborated design: the register may be read in code that itself turned out to be dead. Synthesis will strip the flip-flops; if that surprises you, follow the chain of consumers to find where the path died.",
            "reg stat : u8 = 0\non clk { stat <= s }\n// its only reader was optimized away   // ⚠ W4002",
            "result = stat           // ✓ observed in the netlist",
        ),
    }
}
