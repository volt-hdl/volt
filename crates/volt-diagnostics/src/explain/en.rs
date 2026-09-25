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
        )
        .with_note(
            "E0003 is also reported for constructs that parse but are not implemented yet, for example type generic arguments on modules (ADR-0041), generic struct ports (ADR-0069), a port bundle as a Handshake payload, and valid Volt that has no SystemVerilog mapping yet ('not supported yet: struct type 'P' as a signal type', match guards, extern module instances). `volt check` and the editor report these too, not only `volt build` (ADR-0070).",
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
        E0014 => Explanation::new(
            "The match statement does not cover every value",
            "A 'match' statement inside an on/comb block leaves some values without an arm: a numeric match has no '_' arm, or an enum match misses a variant and has no '_' arm.",
            "In hardware, a match lowers to a 'case'; a value without an arm would have no defined action (in a comb block that is a latch). A match on a number must end with a wildcard '_' arm (ADR-0032). A match on an enum is checked for exhaustiveness instead (ADR-0074): naming every variant is enough, '_' is optional. Then the LAST named arm becomes the SystemVerilog 'default' — so the encodings that belong to no variant (a 3-variant enum is 2 bits wide; code 3 is unused) take the last arm's action. Inside the design such a code cannot appear (an enum value only comes from its variants — 'uN as Enum' is rejected), and the auto-generated state-valid invariant proves it formally; an enum input port driven from outside is the only source. Write an explicit '_' arm when invalid codes need their own recovery action. In a sequential block an empty '_ => { }' arm keeps the registers' values.",
            "on clk {\n    match state {\n        0 => { r <= 1 }     // ✗ E0014: no '_' arm\n    }\n    match s {             // enum State { Idle, Run, Done }\n        State::Idle => { r <= 1 }\n        State::Run  => { r <= 0 }   // ✗ E0014: missing State::Done\n    }\n}",
            "on clk {\n    match state {\n        0 => { r <= 1 }\n        _ => { }            // ✓ other encodings hold their value\n    }\n    match s {\n        State::Idle => { r <= 1 }\n        State::Run  => { r <= 0 }\n        State::Done => { }          // ✓ every variant named; also taken by invalid codes\n    }\n}",
        )
        .with_docs(&["docs/adr/ADR-0032-match-sirali-blokta.md", "docs/adr/ADR-0074-enum-destegi.md"]),

        E0015 => Explanation::new(
            "MMIO register map layout error",
            "Two '@reg' registers of an '@mmio' module overlap, or a register does not fit the 32-bit word.",
            "An '@mmio' module is a memory-mapped register block: every '@reg' occupies one 32-bit word at 'base + offset', and the generated address decoder selects exactly one register per address. Two registers at the same offset (or at offsets that are not 4-byte aligned) would both answer the same bus access, so the decoder could not be generated. The same error reports a register whose fields add up to more than 32 bits, a field type other than bool / bits<N> / uN, and a '@reg' outside an '@mmio' module.",
            "@mmio(base = 0x4000_0000, bus = AXI4Lite)
module Regs {
    @reg(offset = 0x00, access = ReadWrite)
    a : { v : bits<8>, @reserved : bits<24> }
    @reg(offset = 0x00, access = ReadOnly, volatile)   // ✗ E0015: same offset as 'a'
    b : { v : bits<8>, @reserved : bits<24> }
}",
            "@mmio(base = 0x4000_0000, bus = AXI4Lite)
module Regs {
    @reg(offset = 0x00, access = ReadWrite)
    a : { v : bits<8>, @reserved : bits<24> }
    @reg(offset = 0x04, access = ReadOnly, volatile)   // ✓ next word
    b : { v : bits<8>, @reserved : bits<24> }
}",
        )
        .with_docs(&["docs/adr/ADR-0044-mmio-register-haritasi.md"]),

        // ─── Name resolution (name-resolution.md) ───
        E0016 => Explanation::new(
            "declassify without a reason",
            "A 'declassify(expr, \"reason\")' call is missing its reason string, or the reason is empty.",
            "'declassify' is the only sanctioned way for information to move from a higher trust level to a lower one (ADR-0052). Every such point is a security decision that a reviewer must be able to audit later, so the language makes the justification part of the syntax: a non-empty string literal is mandatory, and the compiler repeats it in the W3008 warning it emits for every declassification. A call without a reason is a syntax error, not a warning.",
            "busy = declassify(state != IDLE)                       // ✗ E0016: no reason\nbusy = declassify(state != IDLE, \"\")                   // ✗ E0016: empty reason",
            "busy = declassify(state != IDLE, \"state visibility only\")   // ✓ reviewed, W3008 records it",
        ),
        E0017 => Explanation::new(
            "Unsupported or inconsistent timing constraint",
            "A @timing, @false_path or @multicycle attribute uses a form the compiler does not translate, names a signal that is not a port or register, or contradicts the domain frequency.",
            "Since ADR-0054 these attributes are enforced: 'volt build --emit=sdc' (or xdc) turns them into create_clock, set_max_delay, set_false_path and set_multicycle_path. A form the compiler only half-understands would still produce a constraint file, and a constraint file that silently lacks the line you wrote is worse than none. So every unsupported spelling is an error, not a warning.

Supported forms: @timing(clk = 100.mhz) (exact frequency of a clock port), @timing(clk >= 100.mhz) (minimum), @timing(max_delay(a, b) <= 5.ns), @timing(min_delay(a, b) >= 1.ns), @false_path(from = a, to = b), @multicycle(from = a, to = b, cycles = N); on a register: @false_path, @multicycle(N). Frequencies are written as 25175000 (Hz), 25_175.khz, 100.mhz or 1.ghz; delays always carry a unit (ps, ns, us). Endpoints are ports or registers of the same module — wires, lets and instance outputs are not timing endpoints. A clock requirement is checked against the domain's frequency: '=' must match, '>=' must be met.",
            "@timing(pix_clk >= 25.175.mhz)        // ✗ E0001: no decimal literals\n@timing(max_delay(a, b) <= 5)         // ✗ E0017: delay without a unit\n@false_path(from = tmp, to = y)       // ✗ E0017: 'tmp' is a let, not a register\n@timing(clk = 50.mhz)                 // ✗ E0017: domain says 100.mhz",
            "@timing(pix_clk >= 25_175.khz)        // ✓ kHz spelling\n@timing(max_delay(a, b) <= 5.ns)      // ✓\n@false_path(from = cfg_r, to = y)     // ✓ register -> port\n@timing(clk >= 50.mhz)                // ✓ a requirement, met by 100.mhz",
        )
        .with_note(
            "The generated names follow the Vivado / Design Compiler convention: registers become get_cells {name_reg*}, sub-module signals get the instance prefix (fb/mem_reg*). set_clock_groups -asynchronous is derived from the domains without any attribute; sync()/AsyncFifo/HandshakeSync/PulseSync/AsyncDualPortRam crossings get set_false_path automatically.",
        ),
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
        E1011 => Explanation::new(
            "Module not found",
            "A 'use' names a package that no file in the compilation unit provides.",
            "Since ADR-0042 'use soc::gpio::Gpio' loads a file: first './soc/gpio.volt' next to the importing file, then '<root>/src/soc/gpio.volt' where <root> is the directory holding Volt.toml, then the built-in 'std' prelude. The error lists every path that was tried. The same code is reported when the file exists but does not define the requested item — the help then lists the package's public items.",
            "use soc::gpoi::Gpio     // ✗ E1011: no soc/gpoi.volt anywhere",
            "use soc::gpio::Gpio     // ✓ examples/soc/gpio.volt declares 'package soc::gpio;'",
        ),
        E1012 => Explanation::new(
            "Extern module source missing or outside the project",
            "An extern module's SystemVerilog is needed but '@source' is missing, names a file that does not exist, or leaves the project.",
            "An 'extern module' only declares ports; its body is SystemVerilog you wrote or a vendor shipped. 'volt build' and 'volt check' only emit the instantiation, but 'volt run' and 'volt test' hand the design to Verilator and 'volt verify' to SymbiYosys, and neither can simulate or prove a module whose body it has never seen. '@source(\"path\")' names the file (or files); the path is relative to the .volt file that declares the extern and must stay inside the project — the directory of the nearest Volt.toml, or that file's directory when there is none (the same rule as read_hex). A missing '@source' is reported only by the commands that need the body.",
            "extern module Fifo {          // ✗ E1012 in 'volt test': no body\n    in  clk : clock\n    ...\n}",
            "@source(\"rtl/fifo.sv\")\nextern module Fifo {          // ✓ Verilator and sby read rtl/fifo.sv\n    in  clk : clock\n    ...\n}",
        )
        .with_docs(&["docs/adr/ADR-0076-extern-kaynaklari.md"]),
        E1013 => Explanation::new(
            "Name is a reserved word of a generated language",
            "A name Volt writes into generated SystemVerilog, or into the Rust/C driver of an @mmio register map, is a keyword there.",
            "Volt keeps your names in its output so that an external integrator, a waveform and a constraint file all see the port you wrote. That only works if the name is legal in the target language. SystemVerilog reserves 248 words (IEEE 1800-2017 Annex B, which includes every Verilog-2005 keyword): a port named 'packed' or a register named 'table' makes the generated .sv a syntax error in every tool, although the Volt source is fine. Names Volt builds by joining two of yours with '_' are checked too: a struct port 'pulsestyle' with a field 'ondetect' becomes the SystemVerilog port 'pulsestyle_ondetect', which is a keyword; the same holds for enum localparams ('<Enum>_<Variant>') and instance output wires ('<instance>_<port>'). Volt does not escape (\\packed) or rename (packed_v) behind your back: either would change the port name an external module connects to. An @mmio register or field name also becomes a function or parameter name in the generated Rust and C drivers, so it must not be a Rust, C or C++ keyword ('mod', 'loop', 'default', 'class', ...). Words that are only C++ keywords in Verilator's own model ('interrupt', 'char') are not errors: the SystemVerilog is valid and Volt handles them (ADR-0078). Matching is case-sensitive: 'Packed' is fine.",
            "module Timer {\n    in  packed : u8          // ✗ E1013: SystemVerilog keyword\n    out table  : u8          // ✗ E1013\n    table = packed\n}",
            "module Timer {\n    in  packed_in : u8      // ✓\n    out lut       : u8      // ✓\n    lut = packed_in\n}",
        )
        .with_docs(&["docs/adr/ADR-0078-hedef-dil-ayrilmis-sozcukleri.md"]),
        E1014 => Explanation::new(
            "Two @mmio names generate the same identifier in the register-map driver",
            "The Rust or C driver generated for an @mmio register map would define this identifier twice.",
            "The drivers build their names from yours: register 'ctrl' gets 'ctrl_raw()' / 'CTRL_OFFSET', a field 'en' of a multi-field register gets 'ctrl_en()' / 'CTRL_EN_SHIFT', and the C header prefixes everything with the module ('GPIO_CTRL', 'gpio_get_ctrl_en'). Two different names can meet: a field 'raw' of 'ctrl' and the raw-word accessor 'ctrl_raw'; field 'irq.status_rx' and field 'irq_status.rx' ('irq_status_rx'); registers 'ctrl' and 'Ctrl' (both 'CTRL_OFFSET'); a register named 'new', 'read' or 'write' and the driver's own methods; a register 'h' or 'base' and the header's 'GPIO_H' guard or 'GPIO_BASE'; a field named 'uint32_t' or like a macro as a C setter parameter; two @mmio modules whose snake_case name is the same file ('GpioRegs' and 'GPIORegs' both write build/sw/gpio_regs.*). The Rust driver would not compile, the C header may even compile with one definition silently replacing the other, and two modules would overwrite each other's files. Volt does not rename either name behind your back (ADR-0078): the driver API is what firmware calls. Rename one of the two.",
            "@reg(offset = 0x00, access = ReadWrite)\nctrl : { raw : u8, en : bool, @reserved : bits<23> }   // ✗ E1014: 'ctrl_raw' twice",
            "@reg(offset = 0x00, access = ReadWrite)\nctrl : { data : u8, en : bool, @reserved : bits<23> }  // ✓",
        )
        .with_docs(&["docs/adr/ADR-0079-cikti-dogrulama-agi.md"]),

        // ─── Type inference (type-inference.md) ───
        E2001 => Explanation::new(
            "Bit width mismatch",
            "The two sides of this connection have different bit widths.",
            "Implicit narrowing silently drops upper bits — a classic source of hardware bugs that only appear with large values. Volt never narrows implicitly: the truncation must be written out with 'as'. Widening to a wider target of the SAME sign is implicit only when the target type is written explicitly (a let/reg/port type or an assignment target, ADR-0041); operands of different widths with no written target still need a cast.",
            "in  a : u16\nout y : u8\ny = a                   // ✗ E2001: 16 bits into 8",
            "y = a as u8             // ✓ explicit narrowing (W2010)\nout z : u32\nz = a                   // ✓ same-sign widening, target written",
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
            "Width or length cannot be determined",
            "The compiler cannot tell how many bits this value has (or how many elements an array has).",
            "Every hardware value must have a definite width. A literal usually takes its width from the surrounding context (the port or register it is assigned to); when there is no such context, state the type explicitly. The same applies to the source of a cast, an untyped let, the source of sync(), the N of bits<N> or [T; N] when it is not a compile-time constant, and a constant array used as a whole value instead of indexed. Instance connection problems are E4011, non-constant loop bounds E2021.",
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
        E2013 => Explanation::new(
            "Invalid struct declaration",
            "This plain struct cannot describe a data value.",
            "A plain 'struct' is a one-directional data value: it is stored in a register, compared and converted with 'as', and it lowers to one hardware signal per field. So it needs at least one field (a zero-width value is no signal), and its fields carry no clock domain and no direction — the whole value lives in the domain of the signal that holds it. Per-field domains, directions and 'struct port' bundles belong to a 'struct port' (ADR-0039), which groups directed port fields and is not a value (ADR-0077).",
            "struct Empty { }                // ✗ E2013: no fields\nstruct P { a : u4 @Fast }        // ✗ E2013: domain on a field\nstruct Q { bus : AxiLite }       // ✗ E2013: AxiLite is a 'struct port'",
            "struct P { a : u4, b : bool }   // ✓\nin p : P @Fast                   // ✓ the domain goes on the signal\nstruct port Link { out d : P  in ready : bool }   // ✓ a bundle may carry a struct",
        )
        .with_docs(&["docs/adr/ADR-0077-struct-destegi.md"]),
        E2014 => Explanation::new(
            "Struct literal field missing or repeated",
            "Every field of the struct must be given exactly once in the literal.",
            "In hardware every bit needs an explicit source. An implicit default would silently zero a field in a reset value and hide a forgotten field in combinational logic, so a struct literal lists all fields, each once, in any order (the shorthand 'P { a, b }' takes same-named locals). To change a single field of a register, assign the field: 'p.a <= x' (ADR-0077).",
            "reg p : P = P { a: 0 }                 // ✗ E2014: field 'b' missing\nlet q : P = P { a: 1, a: 2, b: true }  // ✗ E2014: 'a' given twice",
            "reg p : P = P { a: 0, b: false }       // ✓\non clk { p.a <= x }                    // ✓ update one field",
        )
        .with_docs(&["docs/adr/ADR-0077-struct-destegi.md"]),

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
        E2030 => Explanation::new(
            "Invalid enum encoding",
            "The enum's variants cannot be given a single, unambiguous hardware encoding.",
            "An enum lowers to a plain bit vector: by default the variants are numbered 0, 1, 2 ... in declaration order and the width is max(1, clog2(n)). Explicit values ('Add = 0, Jal = 8') and a base type ('enum Op : u4') carry an external encoding (an opcode, a documented register code) into the design. The rules: either every variant has an explicit value or none does (a mixed list has two readings — the next value after 'A = 5' is 6 in SystemVerilog and Rust); values are distinct; the base type is an unsigned uN, uint<N> or bits<N> wide enough for every variant; the enum has at least one variant (ADR-0074).",
            "enum Op : u4 { Add = 0, Sub, Jal = 8 }   // ✗ E2030: mixed explicit/implicit values\nenum Mode : i4 { A = 0, B = 1 }          // ✗ E2030: base type must be unsigned\nenum Dup { A = 1, B = 1 }                // ✗ E2030: duplicate value 1",
            "enum Op : u4 { Add = 0, Sub = 1, Jal = 8 }   // ✓\nenum Mode : u1 { A = 0, B = 1 }             // ✓",
        )
        .with_docs(&["docs/adr/ADR-0074-enum-destegi.md"]),

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
            "Reset domain crossing (RDC)",
            "A reset release is not synchronized to every clock it reaches: one asynchronous reset port is shared by several clock domains, the same raw reset is synchronized twice on one clock, or a raw reset port does not match the domain it feeds.",
            "Asserting a reset asynchronously is harmless; RELEASING it is not. A release that is synchronous to one clock is asynchronous to every other clock, so registers of a second domain may leave reset in different cycles or go metastable (a recovery/removal violation). Volt generates one reset port per polarity ('rst' / 'rst_n'), so two 'reset = async' domains with the same polarity share one port — its release can be synchronous to at most one of their clocks. The fix is to take the raw reset in explicitly as 'in rst_n : reset(async, active_low)': the compiler then adds a two-stage release synchronizer for every clock the port feeds (asynchronous assert, synchronous release) and resets each domain from its own chain (ADR-0065). The same code reports a raw reset synchronized twice on one clock (the parent and an instance each add a chain, so the two releases can land in different cycles), a raw port whose '(sync|async, polarity)' differs from the domain it feeds, and a raw port named like the automatic port of another domain.",
            "domain Fast { clock = posedge, reset = async active_low }\ndomain Slow { clock = posedge, reset = async active_low }\nmodule Top {\n    in fast_clk : clock @Fast    // ✗ E3003: 'rst_n' serves both clocks\n    in slow_clk : clock @Slow\n}",
            "module Top {\n    in fast_clk : clock @Fast\n    in slow_clk : clock @Slow\n    in rst_n : reset(async, active_low)   // ✓ one synchronizer per clock\n}",
        )
        .with_note("Clock-domain crossings of DATA between the two domains are still E3001; E3003 is only about the reset itself. A domain with 'reset = sync' that shares 'rst' with another clock is the milder W3010.")
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
            "Data from a higher trust level reaches a lower-trust sink without passing through 'declassify'.",
            "A domain may carry 'trust_level = secret | confidential | public' (ADR-0052). Every signal inherits the trust level of its domain (K1/K2 as for clocks), an expression carries the highest level of its operands, and the compiler follows that label through assignments, 'let' bindings, registers, 'if'/'match' conditions and instance ports. Information may only flow to the same or a higher level: secret → public is a leak, public → secret is fine, constants fit everywhere. Signals whose domain has no 'trust_level' are unclassified: they take the highest level ever written into them, so an unannotated register cannot launder a secret. The only sanctioned downgrade is 'declassify(expr, \"reason\")', which turns the value public and leaves a W3008 audit trail.",
            "domain SecureCore { trust_level = secret }\ndomain Debug      { trust_level = public }\n\nmodule KeyStore {\n    in  clk       : clock\n    in  key       : u128 @SecureCore\n    out debug_out : u8   @Debug\n    debug_out = key[7:0]                // ✗ E3009: secret data flows to a public output\n}",
            "    out busy : bool @Debug\n    busy = declassify(state != IDLE, \"state visibility only\")   // ✓ deliberate, W3008 records it",
        )
        .with_note("A domain that carries a trust_level but no clock port in the module does not open a new clock domain: the signal stays in the module's clock (K11), the annotation only classifies it. Trust is checked at the type level only — the generated SystemVerilog is unchanged."),
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

        E3013 => Explanation::new(
            "Bundle fields inferred in different clock domains",
            "The flattened fields of one bundle port ended up in different clock domains.",
            "A bundle (struct port, ADR-0039) is one interface: every field crosses the module boundary together, so all of them must live in the same clock domain. A field-level @Domain annotation that differs from the port annotation, or fields used from blocks of different clocks, splits the interface across a CDC boundary. Annotate the whole port with one domain, or split the interface into two bundles.",
            "struct port Bus {\n    out data  : u8 @Fast\n    in  ready : bool @Slow    // ✗ E3013: same bundle, two domains\n}",
            "struct port Bus {\n    out data  : u8\n    in  ready : bool\n}\nmodule M {\n    in  clk : clock\n    out bus : Bus @Fast         // ✓ one domain for the whole bundle\n}",
        ),

        E3014 => Explanation::new(
            "Same symbolic domain bound to two different clocks",
            "Two clock ports of one instance carry the same domain annotation but are driven from different clock domains.",
            "Inside an extern module an unknown @Name is a symbolic clock domain (ADR-0047): it stands for exactly one real domain per instantiation, and the clock connection decides which one. When two clock ports share a symbolic domain, the wrapped SystemVerilog module is single-clock on that side — feeding those ports from different clocks would open a clock-domain crossing inside a black box the compiler cannot see into. The same rule applies to a regular module whose clock ports name the same @Domain.",
            "extern module ExtRegFile {
    in wr_clk : clock @Core
    in rd_clk : clock @Core   // one domain, two clock pins
    ...
}
let rf = ExtRegFile {
    wr_clk: sys_clk,           // @Core := SysDomain
    rd_clk: pix_clk,           // ✗ E3014: @Core is already SysDomain
}",
            "let rf = ExtRegFile {
    wr_clk: sys_clk,
    rd_clk: sys_clk,           // ✓ both pins in SysDomain
}
// or, if the core really is dual-clock, give the sides their own domains:
extern module ExtRegFile {
    in wr_clk : clock @Src
    in rd_clk : clock @Dst
    ...
}",
        )
        .with_docs(&["https://volthdl.org/guide/cdc"]),

        // ─── Connectivity/drivers (type-inference.md) ───
        E4001 => Explanation::new(
            "Double driver",
            "The same signal (or the same bits of it) is driven by two sources.",
            "Two drivers on one wire is an electrical short: whenever they disagree, the result is contention, not a value. Combine the sources into a single assignment (a mux or priority expression) so exactly one value wins at any time. Every source counts: an assignment in another block, a 'let' initializer, an input port (the instantiating module drives it) and a wire bound to a child's inout/opendrain port (driven tri-state through that port). Partial targets conflict only when their bits overlap: y[7:4] and y[3:0] are fine, y = a and y[0] = b are not. Assignments inside one 'on' or 'comb' block are a single driver (ADR-0073).",
            "y = a\ny = b                   // ✗ E4001: who wins?\nlet v = a\nv = b                   // ✗ E4001: the let initializer already drives v",
            "y = if sel { b } else { a }  // ✓ single driver\nlet v = if sel { b } else { a }  // ✓",
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

        E4005 => Explanation::new(
            "Bundle field direction violated",
            "A bundle field whose effective direction is input is assigned inside the module.",
            "Each struct port field carries a direction. Declaring the port with 'in' flips every field (out becomes in, in becomes out) so that master and slave share one definition. A field that is an input after flipping is driven from outside; assigning it would create a second driver. Drive the fields that face outward, or declare the port with the opposite direction.",
            "struct port Req { out addr : u32, in ready : bool }\nmodule Slave {\n    in req : Req               // req.addr is an INPUT here\n    req.addr = 0               // ✗ E4005\n}",
            "module Slave {\n    in req : Req\n    req.ready = true           // ✓ 'in ready' flips to output\n}",
        ),

        E4006 => Explanation::new(
            "Bus-owned MMIO register field written from RTL",
            "A field of a non-volatile '@reg' register is assigned inside the module body.",
            "In an '@mmio' module every register has exactly one writer. A register without 'volatile' is owned by the bus: software writes it, the generated decoder stores it, and the RTL only reads it ('let en = regs.control.enable'). Assigning such a field from RTL would create a second driver next to the generated write logic. Registers that the hardware updates (status, inputs, counters) are declared 'volatile': the RTL writes them with '<=' and the bus only reads them (a '@w1c' field is the exception — software clears it by writing 1).",
            "@mmio(base = 0, bus = AXI4Lite)
module Gpio {
    in clk : clock
    in pins_in : u8
    @reg(offset = 0x08, access = ReadOnly)
    input : { pins : u8, @reserved : bits<24> }
    on clk { regs.input.pins <= pins_in }   // ✗ E4006: 'input' is not volatile
}",
            "@mmio(base = 0, bus = AXI4Lite)
module Gpio {
    in clk : clock
    in pins_in : u8
    @reg(offset = 0x08, access = ReadOnly, volatile)
    input : { pins : u8, @reserved : bits<24> }
    on clk { regs.input.pins <= pins_in }   // ✓ hardware-owned
}",
        )
        .with_docs(&["docs/adr/ADR-0044-mmio-register-haritasi.md"]),

        // ─── Behavioral contracts ───
        E4007 => Explanation::new(
            "Handshake valid depends combinationally on ready",
            "The producer side of a 'Handshake<T>' port derives 'valid' from 'ready' through a combinational path.",
            "A valid/ready handshake completes when both signals are high in the same cycle. The protocol (Volt Handshake, AXI A3.3.1) puts the obligation on the producer: it raises 'valid' when it has data and holds it until 'ready' arrives, without looking at 'ready' first. The consumer is free to derive 'ready' from 'valid'. If the producer also waited for 'ready', two such parties would wait for each other forever. The compiler follows continuous assignments, 'let' bindings and 'comb' blocks (conditions included) from 'valid' back to 'ready'; a register ('on clk') breaks the path, so decide 'valid' from registered state instead. Instance outputs are opaque to this check.",
            "module Producer {\n    in  clk : clock\n    out tx  : Handshake<u8>\n    tx.valid = tx.ready && have_data    // ✗ E4007: valid waits for ready\n    tx.data  = 0\n}",
            "module Producer {\n    in  clk : clock\n    out tx  : Handshake<u8>\n    reg valid_r : bool = false\n    on clk {\n        if tx.fired { valid_r <= false }\n        else if have_data { valid_r <= true }\n    }\n    tx.valid = valid_r                  // ✓ registered decision\n    tx.data  = 0\n}",
        ),
        E4008 => Explanation::new(
            "Bidirectional port misuse",
            "An 'inout' or 'opendrain' port is assigned directly, driven outside an 'on' block, or used with a member it does not have.",
            "A bidirectional pad is shared with the outside world, so its value is not a plain expression: at every moment the module either drives it or leaves it to the other devices (high impedance / the pull-up). Volt keeps that decision in the module's own registers (<p>_oe and <p>_out for 'inout', <p>_drive_low for 'opendrain'), synthesised by the compiler, and generates the single tri-state buffer 'assign p = enable ? value : \'z' itself (ADR-0051). A continuous assignment 'p = expr' would produce a push-pull driver that fights the bus; a drive call outside an 'on' block has no register to hold the state; other member names have no meaning on a pad.\n\nThe only operations are: p.drive(value) (inout), p.drive_low() (opendrain), p.release() -- statements inside 'on clk'; p.read() -- the resolved line level as an expression; p.released / p.driving -- the drive state, usable in contracts and expressions. An 'opendrain' port is always 'bool'; an 'inout' port is bool, uN, iN or bits<N>.",
            "module Pad {\n    in  clk : clock\n    in  en  : bool\n    opendrain sda : bool\n    sda = if en { false } else { true }    // ✗ E4008: push-pull on an open-drain line\n}",
            "module Pad {\n    in  clk : clock\n    in  en  : bool\n    opendrain sda : bool\n    on clk {\n        if en { sda.drive_low() } else { sda.release() }   // ✓ registered drive intent\n    }\n    invariant: !en -> sda.released\n}",
        ),

        E4009 => Explanation::new(
            "Recursive type",
            "A type contains itself, directly or through other types: a struct or struct port field, an enum variant payload or base type, or a type alias target leads back to the type.",
            "Every Volt type is a fixed number of bits, and a port group is flattened to plain ports at compile time, one port per leaf field ('req_addr', 'req_ready', ...). A type that contains itself has no finite width: 'struct P { d : u8, f : P }' would need 8 + width(P) bits, and a recursive bundle would expand to 'req_req_addr', 'req_req_req_addr', ... forever. The cycle may pass through arrays ('[S; 4]'), tuples, enum payloads, type aliases and generic arguments ('Box<P>' when Box stores its parameter).\n\nBefore ADR-0067 a recursive struct port was silently cut at nesting depth 8 (and, with several self-referencing fields, expanded into millions of ports -- found by the fuzzer as a multi-gigabyte memory blow-up). Before ADR-0069 a recursive plain struct, enum or alias passed 'volt check' without any diagnostic.",
            "struct port Req {\n    out addr : u32\n    in  req  : Req      // ✗ E4009: Req contains Req\n}\nstruct P {\n    d : u8\n    f : [P; 2]          // ✗ E4009: through an array\n}\ntype T = T              // ✗ E4009",
            "struct port Req {\n    out addr  : u32\n    in  ready : bool    // ✓ leaf fields only, or another (non-recursive) type\n}\nstruct P {\n    d : u8\n    f : [u8; 2]\n}",
        )
        .with_note(
            "Every type on the cycle is reported once, with the member that closes the cycle and the cycle path ('A.b → B.a → A'). Types that merely refer to a recursive type are not reported and are not flattened either; fix the cycle first. A generic argument counts only if the generic type stores that parameter: 'Tag<P>' with 'struct Tag<T> { v : u8 }' is finite.",
        ),
        E4010 => Explanation::new(
            "Bundle flattening budget exceeded",
            "Flattening the bundle ports of one module would produce more than 4096 plain ports, or a bundle port nests deeper than 8 levels.",
            "Bundle flattening is exponential in the shape of the type graph: a struct port with two fields of a struct port with two fields of ... doubles at every level, and a bundle array ([Bundle; N], ADR-0056) multiplies by N. Even without a cycle (E4009) an accidental diamond-shaped graph can request millions of ports. The budget turns that into a diagnostic instead of a memory blow-up (ADR-0067): at most 4096 flat ports per module (a 256-element bundle array of a 16-field interface) and at most 8 levels of nesting. Real interfaces stay far below both limits; a module that needs more should be split.",
            "struct port Wide { out f0 : u8  /* ... f16 */ }   // 17 fields\nmodule Sink {\n    in ch : [Wide; 256]     // ✗ E4010: 256 x 17 = 4352 flat ports\n}",
            "struct port Wide { out f0 : u8  /* ... f15 */ }   // 16 fields\nmodule Sink {\n    in ch : [Wide; 256]     // ✓ 4096 flat ports, within the budget\n}\n// or split the interface across several modules",
        ),
        E4011 => Explanation::new(
            "Instance port connection error",
            "A port of a module, extern module or builtin instance is connected in a way that has no hardware meaning.",
            "An instance literal connects the parent's signals to the child's ports: every input and clock must be bound (there is no default value), an output is read as inst.port and never bound in the literal, an inout/opendrain port shares a line and must be bound to a wire or a bidirectional port by name, and the child's ports are driven only by the child — the parent cannot assign inst.port. A child whose clock domain has a reset also needs a clock of that domain (with its reset) in the parent. This is a connection error, not a width problem (reported as E2005 before ADR-0072).",
            "let f = Filter { clk }                         // ✗ E4011: input 'sample' is not bound\nlet g = Filter { clk, sample: x, result: y }   // ✗ E4011: output bound in the literal",
            "let f = Filter { clk, sample: x }\ny = f.result                                   // ✓",
        ),
        E4012 => Explanation::new(
            "Part of a signal is never driven",
            "The signal is assigned piece by piece, and some of its pieces have no driver.",
            "A wire, output port or typed 'let' that is driven field by field (p.a = ..., p.b = ...) or slice by slice (y[3:0] = ...) must have every field and every bit driven; an undriven part is X/undriven in SystemVerilog (Verilator reports UNDRIVEN) and Volt never produces X (ADR-0008). This is the field-by-field form of the rule that a struct literal lists every field (E2014). Registers are exempt: an unassigned field keeps its value, and the reset value is complete (ADR-0077).",
            "wire p : P\np.a = x                 // ✗ E4012: field 'p.b' is never driven\nout y : u8\ny[3:0] = a              // ✗ E4012: bits 7..4 of 'y' are never driven",
            "wire p : P\np.a = x\np.b = go                // ✓\ny = (a as u8)           // ✓ or drive y[7:4] too",
        )
        .with_docs(&["docs/adr/ADR-0077-struct-destegi.md"]),

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
        E5002 => Explanation::new(
            "Contract not proven",
            "In --mode prove the base case held but the induction step failed: the contract may be true, yet it is not inductive (sby status UNKNOWN).",
            "k-induction proves a property in two steps. The base case checks the first --depth cycles from reset — it found no counterexample, so this is not E5001. The induction step assumes the property held for --depth consecutive cycles starting from ANY state and asks whether it still holds one cycle later. That arbitrary state may be one the design can never reach, e.g. two registers that always move together but start out of step. The solver found such a state, so the proof is inconclusive. Exit code 7 (not 6: nothing was refuted; not 3: the tool did not fail).",
            "reg a : u8 = 0\nreg b : u8 = 0          // a and b always equal\ninvariant: a <= 10      // ✗ E5002: induction may start from a != b",
            "invariant: a == b      // ✓ strengthening invariant: excludes the\ninvariant: a <= 10     //   unreachable start states — now inductive",
        )
        .with_note(
            "Two remedies: a larger --depth lets the induction step see more history (it helps when the bad start state leads back to a violation only after many cycles), and an extra invariant that relates the registers removes the unreachable start states. The induction trace is copied to build/formal/<task>_induct.vcd; its first cycles show the unreachable state the solver chose.",
        )
        .with_docs(&["https://volthdl.org/guide/verify", "docs/adr/ADR-0075-yaniltici-rapor-ve-tani-temizligi.md"]),
        E5004 => Explanation::new(
            "Contract expression is not Bool",
            "requires/ensures/invariant/cover/assert/assume conditions must be Bool expressions.",
            "A contract states a property that either holds or does not; only a Bool expression carries that meaning. A numeric expression such as 'speed + 1' has no truth value, so the compiler cannot turn it into an assertion, an assumption or a coverage goal.",
            "module M {\n    in speed : u8\n    requires: speed + 1     // ✗ E5004: type is u9, not bool\n}",
            "module M {\n    in speed : u8\n    requires: speed <= 2    // ✓ comparison yields bool\n}",
        ),
        E5010 => Explanation::new(
            "Timing misalignment",
            "In a @strict_timing module, values whose pipeline delays differ cannot be combined directly.",
            "Every signal in a pipelined design belongs to an instruction that entered the pipe some number of cycles ago — its delay (ADR-0037, L1). Combining a 3-cycle-old value with a 2-cycle-old one usually means a missing stage register or a forward from the wrong stage; the result silently mixes two different instructions. Inside a @strict_timing module the compiler tracks a delay for each port (0), register (source delay + 1) and let (join of its operands), and rejects any operator whose operands disagree.\n\nIf the mix is intentional (forwarding, bypass), state the result's delay explicitly — 'let fwd : Delayed<u32, 2> = ...' — or re-align a younger value with 'delay<K>(x)'. Constants and literals are exempt: they carry no timing.",
            "@strict_timing\nmodule P {\n    in x : u32\n    reg a : Delayed<u32, 1> = 0\n    reg b : Delayed<u32, 2> = 0\n    let sum = a + b        // ✗ E5010: 1 cycle vs 2 cycles\n    on clk { a <= x  b <= a }\n}",
            "@strict_timing\nmodule P {\n    in x : u32\n    reg a : Delayed<u32, 1> = 0\n    reg b : Delayed<u32, 2> = 0\n    let sum = delay<1>(a) + b   // ✓ both sides are 2 cycles old\n    on clk { a <= x  b <= a }\n}",
        ),
        E5011 => Explanation::new(
            "Invalid pipeline structure",
            "The stage count must match pipeline(N), stage names must be unique, and the pipeline needs exactly one clock port.",
            "pipeline(N) declares the depth of the pipe up front; the compiler derives every stage register, stall guard and flush guard from it (ADR-0038). A mismatch between N and the number of 'stage' blocks, a duplicated stage name, or an ambiguous clock would make the generated structure ill-defined, so each is rejected here rather than surfacing later as a confusing downstream error.",
            "pipeline(5) P {\n    in clk : clock\n    stage F { }\n    stage D { }      // ✗ E5011: 2 stages, 5 declared\n}",
            "pipeline(2) P {\n    in clk : clock\n    stage F { }\n    stage D { }      // ✓ depth matches\n}",
        ),
        E5012 => Explanation::new(
            "Invalid stage reference",
            "stage(X).y must name a known stage and a value that already exists at stage X.",
            "stage(X).y reads value 'y' as stage X sees it: the live signal in y's own stage, or the pipeline register carrying it into a later stage. The reference is invalid when X is not a stage of this pipeline (unknown name, or a relative form like stage(+9) that walks past the last stage), when 'y' is not a stage-local value, or when X is earlier than the stage that defines 'y' — the value simply does not exist yet at that point. The relative forms stage(+k)/stage(-k) are anchored to the current stage, so they are only meaningful inside a stage body.",
            "pipeline(2) P {\n    in clk : clock\n    stage F { let a : u32 = 1 }\n    stage D { let b : u32 = stage(+9).a }   // ✗ E5012: past the last stage\n}",
            "pipeline(2) P {\n    in clk : clock\n    stage F { let a : u32 = 1 }\n    stage D { let b : u32 = stage(-1).a }   // ✓ previous stage\n}",
        ),
        E5013 => Explanation::new(
            "Invalid stall/flush statement",
            "A stall set must be a contiguous prefix of the pipeline; stage lists must name real stages.",
            "Stalling a stage means every earlier stage must also hold — otherwise the held stage would be overwritten by the one still advancing behind it. The compiler therefore requires the stalled set to start at the first stage and be contiguous (ADR-0038 §4). The bare form 'stall when cond' infers that prefix from the stage it is written in, so at module level it has no anchor and the stage list is mandatory. Flush lists are free-form but must name stages of this pipeline.",
            "pipeline(3) P {\n    in clk : clock\n    stage F { }\n    stage D { }\n    stage X { }\n    stall D when hazard      // ✗ E5013: D without F is not a prefix\n}",
            "pipeline(3) P {\n    in clk : clock\n    stage F { }\n    stage D { }\n    stage X { }\n    stall F, D when hazard   // ✓ contiguous prefix\n}",
        ),
        E5014 => Explanation::new(
            "Pipelined value needs an explicit scalar type",
            "A stage-local let that crosses a stage boundary must be annotated with bool, uN, iN or bits<K>.",
            "When a value defined in one stage is read in a later one, the compiler materializes a register per crossed boundary and a zero-valued bubble for stall and flush. Both need the concrete type: the register declaration is emitted from it and the bubble is its zero (false or 0). This is the pipeline counterpart of the 'reg types must be written explicitly' rule (E2012). Values consumed only inside their own stage may stay unannotated.",
            "pipeline(2) P {\n    in clk : clock\n    in x : u32\n    stage F { let a = x + 1 }\n    stage D { let b : u32 = a }   // ✗ E5014: 'a' crosses, no type\n}",
            "pipeline(2) P {\n    in clk : clock\n    in x : u32\n    stage F { let a : u32 = x + 1 }\n    stage D { let b : u32 = a }   // ✓",
        ),
        E5015 => Explanation::new(
            "Combinational cycle through stage references",
            "stage(...) reads of live values must not form a dependency cycle.",
            "A stage(X).y reference to a value in its own defining stage is a plain wire, not a register. If two such wires depend on each other — a in stage F reads stage(D).b while b in stage D reads stage(F).a — the generated netlist would contain a combinational loop. The compiler orders the hoisted lets by dependency and rejects any cycle. Break the loop by routing one direction through a pipeline register (reference the value from a later stage) or by recomputing one side locally.",
            "stage F { let a : u32 = stage(+1).b }\nstage D { let b : u32 = stage(-1).a }   // ✗ E5015: a → b → a",
            "stage F { let a : u32 = pc }\nstage D { let b : u32 = a + 4 }          // ✓ acyclic",
        ),
        E5016 => Explanation::new(
            "Stage-local value name is not unique",
            "Every stage-local let in a pipeline must have a distinct name.",
            "Cross-stage references are made by name ('a' in a later stage means 'the pipelined copy of a'), and the generated registers are named <stage>_<name>_r. If two stages both defined 'a', both the reference and the generated SystemVerilog would be ambiguous. There is no shadowing inside a pipeline; pick distinct names — the stage prefix in the generated code keeps them readable.",
            "stage F { let v : u32 = 1 }\nstage D { let v : u32 = 2 }   // ✗ E5016: 'v' defined twice",
            "stage F { let f_v : u32 = 1 }\nstage D { let d_v : u32 = 2 }  // ✓",
        ),

        E5017 => Explanation::new(
            "prev() used outside a contract or with invalid arguments",
            "The prev() builtin appears in RTL (a let, an assignment, an on/comb block) or its arguments are not (signal) / (signal, positive literal).",
            "prev(x) is the previous-cycle value of x and prev(x, N) the value N cycles ago; it exists only for sequential contracts (requires/ensures/invariant/cover/assert/assume) and lowers to $past(x) in SVA or to a helper register chain in the Yosys flow (ADR-0040). Hardware itself has no implicit history: a past value in RTL must be an explicit register so that its clock, reset and width are visible.",
            "module M {\n    in  clk : clock\n    in  x : u8\n    out y : u8\n    y = prev(x)              // ✗ E5017: RTL context\n}",
            "module M {\n    in  clk : clock\n    in  x : u8\n    out y : u8\n    reg x_r : u8 = 0\n    on clk { x_r <= x }\n    y = x_r                  // ✓ explicit register\n    invariant: prev(x) == x_r   // ✓ prev() inside a contract\n}",
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

        // ─── Simulation tests (ADR-0033) ───
        E8501 => Explanation::new(
            "Unknown module instantiated in a test block",
            "The 'let dut = X { };' statement names a module that does not exist.",
            "A test drives one design-under-test. The module must be defined in the same file, or — for a file named X_test.volt — in the sibling file X.volt, which is parsed automatically. A typo in the module name is the usual cause.",
            "test \"t\" {\n    let dut = Countr { };   // ✗ E8501: no module 'Countr'\n}",
            "test \"t\" {\n    let dut = Counter { };  // ✓\n}",
        ),
        E8502 => Explanation::new(
            "Unknown port in a test block",
            "The referenced port does not exist on the instantiated module.",
            "Test statements may only touch the module's declared ports; internal registers are not visible from a testbench. Check the port list of the module for the exact name.",
            "dut.enabel = true;      // ✗ E8502: no port 'enabel'",
            "dut.enable = true;      // ✓",
        ),
        E8503 => Explanation::new(
            "Write to a port that is not an input",
            "Only 'in' ports may be driven from a test; clock ports are driven by the simulator itself.",
            "Outputs are produced by the design — writing them from the testbench would create a driver conflict. The clock is toggled automatically by step(), and the implicit reset line is controlled by reset(), so neither may be assigned directly.",
            "dut.count = 3;          // ✗ E8503: 'count' is an output",
            "step(3);                // ✓ let the design produce count",
        ),
        E8504 => Explanation::new(
            "Read from a port that is not an output",
            "Assertions and reads may only observe 'out' ports.",
            "A testbench observes the design through its outputs. Reading an input back would only reflect the value the test itself wrote, which asserts nothing about the design.",
            "assert_eq(dut.enable, 1);   // ✗ E8504: 'enable' is an input",
            "assert_eq(dut.count, 1);    // ✓",
        ),
        E8505 => Explanation::new(
            "Invalid test builtin call",
            "The call does not match any test builtin, or its arguments are wrong.",
            "The test body accepts exactly six builtins: step(n) with an integer n >= 1, reset() with no arguments, assert_eq(a, b), assert_ne(a, b), assert_true(a) and assert_false(a). Anything else — unknown names, wrong arity, step(0) — is rejected at compile time.",
            "step();                 // ✗ E8505: step needs a cycle count",
            "step(1);                // ✓",
        ),
        E8506 => Explanation::new(
            "Undefined or duplicate instance name in a test block",
            "An instance is used before its 'let' statement, or the same name is bound twice.",
            "Each test names its design-under-test with a single 'let dut = Module { };' line before any use. Re-binding the same name would make the following statements ambiguous.",
            "test \"t\" {\n    dut.enable = true;      // ✗ E8506: 'dut' not defined yet\n}",
            "test \"t\" {\n    let dut = Counter { };\n    dut.enable = true;      // ✓\n}",
        ),
        E8507 => Explanation::new(
            "Test data file not found or outside the project",
            "read_hex() names a file that does not exist, or a path that leaves the project directory.",
            "Test data paths are resolved relative to the test file, and a test may only read files inside its project (the directory of the nearest Volt.toml, or the test file's own directory when there is none). A test that reaches outside — an absolute path, or '..' past the project root — would make the result depend on the machine it runs on, and would let a downloaded test read arbitrary files.",
            "test \"t\" {\n    let dut = Soc { };\n    let rom = read_hex(\"../../secrets.hex\");   // ✗ E8507: leaves the project\n}",
            "test \"t\" {\n    let dut = Soc { };\n    let rom = read_hex(\"sw/hello.hex\");        // ✓ relative to the test file\n}",
        ),
        E8508 => Explanation::new(
            "Malformed hex data file",
            "The file given to read_hex() is not valid $readmemh text.",
            "read_hex() accepts the Verilog $readmemh format: whitespace-separated hexadecimal words, '_' separators, '//' and '/* */' comments and '@addr' to move the write position (in elements). Words wider than 64 bits, x/z digits, an empty file and addresses beyond 1M elements are rejected, because the test script works on plain 64-bit values.",
            "// data.hex\n0000_0013\n0000_00G3        // ✗ E8508: 'G' is not a hex digit",
            "// data.hex\n@0\n0000_0013 0000_0093   // ✓\n/* gap */ @8\nDEADBEEF",
        ),
        E8509 => Explanation::new(
            "load() target is not a memory array",
            "The first argument of load() must name an array register of the design under test.",
            "load(dut.mem, data) writes test data straight into a memory of the simulated design, so the target has to be a 'reg' with an array type — in the test's module (dut.mem) or in a sub-instance (dut.cpu.mem). Ports, wires, scalar registers and unknown names cannot be loaded. Elements wider than 64 bits are not supported.",
            "test \"t\" {\n    let dut = Soc { };\n    let rom = [0x13, 0x93];\n    load(dut.halted, rom);      // ✗ E8509: 'halted' is a port\n}",
            "test \"t\" {\n    let dut = Soc { };\n    let rom = [0x13, 0x93];\n    load(dut.imem, rom);        // ✓ reg imem : [u32; 128]\n}",
        ),
        E8510 => Explanation::new(
            "load() source does not fit the target array",
            "The data is longer than the target memory, or a value needs more bits than one element has.",
            "load() copies element 0 to element 0 and leaves the rest of a longer target untouched — like $readmemh. Data that is longer than the memory, or a word that does not fit the element width, would be silently truncated in simulation, and the program you think is running would not be the one in memory. When the memory size is not a plain number the same check runs at simulation time and fails the test.",
            "test \"t\" {\n    let dut = Soc { };             // reg imem : [u32; 2]\n    let rom = [1, 2, 3];\n    load(dut.imem, rom);           // ✗ E8510: 3 elements into 2\n}",
            "test \"t\" {\n    let dut = Soc { };             // reg imem : [u32; 4]\n    let rom = [1, 2, 3];\n    load(dut.imem, rom);           // ✓ imem[3] keeps its value\n}",
        ),
        E8511 => Explanation::new(
            "Type mismatch in a test expression",
            "An array is used where a number is expected, or the other way round.",
            "Test values are either 64-bit numbers or arrays of them. Ports, step(), assertions, operators and loop bounds take numbers; indexing, len() and the source of load() take arrays. Array literals hold constant numbers only and cannot be empty, and read_hex() takes a string literal and must be bound with 'let' first.",
            "test \"t\" {\n    let dut = Sbox { };\n    let expected = [0x63, 0x7c];\n    assert_eq(dut.q, expected);      // ✗ E8511: an array, not a number\n}",
            "test \"t\" {\n    let dut = Sbox { };\n    let expected = [0x63, 0x7c];\n    assert_eq(dut.q, expected[0]);   // ✓\n}",
        ),
        E8512 => Explanation::new(
            "Value does not fit in port width",
            "A test writes a constant to a port, or compares a port with a constant, that the port's type cannot hold.",
            "The testbench hands a port a plain C++ integer and the simulator does not mask it: writing 8 to a u3 port leaves a stray bit in the model, and the simulation then runs a state the hardware can never be in — a lookup reads the wrong entry and a comparison takes the wrong branch, silently. Constants are checked at compile time — a literal, an expression over literals, a 'let' bound to such a value (let n = 8; dut.addr = n) and a literal top-level const; the test language has no reassignment, so a constant 'let' stays constant. A value only known during the run (a port read, a loop counter, an array element) is checked when the test runs and fails the test with the port, the value and the loop iteration. A signed port accepts its bit patterns (i8: 0..255) and negative numbers written as 0 - n (down to -128). Ports always read back as bit patterns, so a comparison against a constant above the port's maximum can never match.",
            "test \"t\" {\n    let dut = Table { };      // in addr : u3\n    dut.addr = 8;             // ✗ E8512: u3 holds 0..7\n}",
            "test \"t\" {\n    let dut = Table { };      // in addr : u3\n    for i in 0..8 {\n        dut.addr = i;         // ✓ 0..7\n        step(1);\n    }\n}",
        ),
        E8513 => Explanation::new(
            "Top-level port collides with the Verilator model API",
            "The module simulated by 'volt test' or 'volt run' has a port named like a member of the C++ class Verilator generates for it.",
            "Verilator turns the top module into a C++ class V<Top> whose ports are data members next to its own API: eval(), final(), trace(), name(), contextp(), rootp and a few more. A port with one of those names produces a class that does not compile, and Verilator does not rename it (it renames only C++ keywords such as 'char' to '__SYM__char', which Volt's test bench follows). The SystemVerilog itself is valid, so 'volt check' and 'volt build' accept the module; only simulating it with this module as the top fails. Rename the port, or simulate a wrapper that instantiates the module.",
            "module Probe {\n    in  clk  : clock\n    out name : u8            // ✗ E8513 in 'volt test': VProbe::name()\n}",
            "module Probe {\n    in  clk     : clock\n    out name_id : u8         // ✓\n}",
        )
        .with_docs(&["docs/adr/ADR-0078-hedef-dil-ayrilmis-sozcukleri.md"]),

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

        E9003 => Explanation::new(
            "Register map drift between a driver file and the RTL",
            "A driver file (.h, .rs or regmap .json) promises register offsets, masks, access rights or reset values that the hardware does not have.",
            "Firmware and RTL usually live in different repositories. Generating both from one @mmio map protects against typing an offset twice, but not against the three ways a register map really goes wrong: a generator bug that makes the driver and the SystemVerilog disagree, a stale header left in the firmware tree after the RTL moved on, and a hand edit inside a generated file. None of these fails a compile: the firmware builds, the bitstream builds, and the first symptom is a write that lands in the wrong register on the board.

'volt check-regmap <design.volt> --against <file>' reduces the file to the facts a driver promises (base, register name/offset/access/mask/reset, field name/position/mask/behaviour) and compares them with the design's map; comments, whitespace and declaration order are not facts, so they never count as drift. Every generated file carries '// regmap-hash:' — when the hash recomputed from the file's content equals the design's, the detailed comparison is skipped. For a file generated by the same Volt version the accessor code is also compared token by token, which catches a hand edit inside a function body. 'volt build --emit=c --check-regmap' runs the same comparison between the freshly generated drivers and the SystemVerilog address decode.",
            "$ volt check-regmap gpio.volt --against firmware/gpio.h\nerror[E9003]: register map drift in firmware/gpio.h\n  = GPIO_CONTROL offset: file 0x0C, RTL 0x10\n  = missing in file: GPIO_IRQ_STATUS (0x14)",
            "$ volt build --emit=c --target-dir firmware/generated gpio.volt\n$ volt check-regmap gpio.volt --against firmware/generated/sw/gpio.h   // ✓ exit 0",
        )
        .with_note(
            "The note lines say which side holds which value ('file' = the checked file, 'RTL' = the design). The cause line uses the declared regmap-hash: equal to the design's means the file was edited after generation; equal to the file's own content means it is stale. Use '--format json' in CI; the exit code is 1 on drift.",
        ),
        E9004 => Explanation::new(
            "Not a Volt-generated register map file",
            "The file given to --against is not a driver or regmap that Volt generated, so it cannot be compared.",
            "check-regmap reads only the formats Volt writes: a .h from --emit=c, a .rs from --emit=rust and a .json from --emit=regmap, each signed with '// Generated by Volt' and '// regmap-hash:' (the JSON carries a regmap_hash key). A hand-written header has no fixed shape — guessing which macro is an offset would turn a real mismatch into silence, so such a file is rejected explicitly instead. Files generated before the signature existed are recognised and reported separately: regenerate them once.",
            "$ volt check-regmap gpio.volt --against legacy/gpio_regs.h\nerror[E9004]: legacy/gpio_regs.h is not a Volt-generated register map",
            "$ volt build --emit=c gpio.volt          // regenerate, then vendor build/sw/gpio.h\n$ volt check-regmap gpio.volt --against build/sw/gpio.h   // ✓",
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
            "Attribute is parsed but not yet enforced",
            "The attribute is valid syntax, but no compiler pass reads it: no constraint, check or output is generated from it.",
            "Volt forbids silently ignoring what the user wrote. @budget, @version, @abi_version, @dft, @debug_visible, @debug_trace, @synthesis_target and @domain are in the grammar (so they are not W0020), yet none of them is enforced today — @budget checks nothing, @version compares nothing. Without this warning you would believe a check exists when it does not, and the gap would only surface in the vendor tool or in silicon.

Keep the attribute if you want it to start working the day it is enforced, and acknowledge the gap explicitly: @allow(unenforced) on the same item silences W0021 for that item (ports and body included); Volt.toml [lint] unenforced_attributes = \"allow\" silences it for the whole package. The diagnostic's second note lists exactly which attributes are still unenforced in this compiler version.",
            "@budget(lut = 5000)   // ⚠ W0021: no utilization check exists
module VgaTiming { /* ... */ }",
            "@budget(lut = 5000) @allow(unenforced)   // ✓ acknowledged
module VgaTiming { /* ... */ }

// or, package-wide, in Volt.toml:
// [lint]
// unenforced_attributes = \"allow\"",
        )
        .with_note(
            "@timing, @false_path and @multicycle left this list with ADR-0054: 'volt build --emit=sdc' (or xdc) turns them into create_clock, set_max_delay, set_false_path and set_multicycle_path in build/constraints/<Module>.sdc, and a malformed one is E0017. An @allow(unenforced) written for them does nothing now and can be removed. ADR-0048 remains the roadmap for @budget (E6001) and the versioning checks (E7001/E7002).",
        ),
        W0023 => Explanation::new(
            "Too many diagnostics; the rest are hidden",
            "The compilation produced more diagnostics than the reporting limit (default 1000), so only the first ones are shown — errors before warnings, in source order.",
            "A single mistake in a template that is expanded many times (an unrolled 'for', a generic module instantiated with many arguments, a bundle array) is folded into one diagnostic with a note saying how many copies it stands for, so the limit is reached only by genuinely distinct diagnostics. Past a thousand of them nobody reads the list; the limit keeps the terminal, CI logs and JSON consumers usable and is a safety net against unknown blow-ups. Nothing is lost silently: this warning says exactly how many diagnostics were hidden.",
            "$ volt check design.volt\n...\nwarning[W0023]: too many diagnostics: 1000 shown, 64365 hidden",
            "$ volt check --max-diagnostics=0 design.volt   // ✓ everything, unlimited\n$ volt check --max-diagnostics=50 design.volt  // ✓ a shorter list",
        ),
        W0022 => Explanation::new(
            "Clock domain has no frequency; no create_clock emitted",
            "A constraint file was requested (--emit=sdc or xdc), but this clock's domain declares no 'frequency', so its create_clock line is missing.",
            "A create_clock needs a period. Without it the timing tool treats every path in that domain as unconstrained: synthesis reports no violation because it checks nothing, and the design can fail on the board while every report is green. Volt knows the domain from the @Name annotation; it only lacks the number. The warning is emitted once per domain (or per unannotated clock port) and only when a constraint file is actually being generated — a design that never asks for SDC is not asked for frequencies.

Declare the frequency in the domain so that every module sharing it is constrained the same way; @timing(clk = F) on a module also fills the gap, but it lives on one module only.",
            "domain PixDomain {\n    clock = posedge,\n    reset = sync active_high,\n}                        // ⚠ W0022 when --emit=sdc: no period for pix_clk",
            "domain PixDomain {\n    clock = posedge,\n    reset = sync active_high,\n    frequency = 25_175.khz,   // ✓ create_clock -period 39.722\n}",
        )
        .with_note(
            "Clocks without a frequency are also left out of set_clock_groups (get_clocks would fail on an undefined clock) and of the -from [get_clocks ...] form of generated false paths; those fall back to register and port names. Decimal literals do not exist: 25.175 MHz is written 25_175.khz or 25175000.",
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
        W2014 => Explanation::new(
            "Unreachable match arm",
            "An earlier arm of this enum 'match' already covers the same variant, so this arm never runs.",
            "In a 'case' the first matching label wins; a second arm for the same variant is dead hardware and usually a copy-paste slip (the arm meant another variant). The compiler drops the arm from the generated SystemVerilog. Merge the two bodies or name the variant this arm was meant for (ADR-0074).",
            "match s {\n    State::Idle => { a <= 1 }\n    State::Idle => { a <= 2 }   // ⚠ W2014: unreachable\n    _ => { }\n}",
            "match s {\n    State::Idle => { a <= 1 }\n    State::Run  => { a <= 2 }   // ✓\n    _ => { }\n}",
        )
        .with_docs(&["docs/adr/ADR-0074-enum-destegi.md"]),
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
            "Each bit synchronizes independently, so during a change the receiver can observe mixtures of old and new bits (0b1111 → 0b1100 for one cycle). For counters use Gray coding, for data streams an AsyncFifo, for control a handshake, for random-access data (frame buffers, lookup tables) an AsyncDualPortRam whose memory array is the crossing — sync() alone is only safe for single bits.",
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
        W3006 => Explanation::new(
            "Same-address port collision",
            "Two memory ports can touch the same address in the same cycle; the result is not what either port expects (DualPortRam: port B silently wins; AsyncDualPortRam: the read is undefined).",
            "DualPortRam gives two independent read/write ports on one clock. The generated memory applies port A's write first and port B's write second, so a same-cycle write to the same address keeps only port B's data. AsyncDualPortRam (ADR-0049) has one write port and one read port on different clocks; a read that overlaps a write to the same address from the other clock returns an undefined value, because the memory array itself is the clock-domain crossing and no synchronizer can order the two accesses. Addresses are runtime values, so the compiler cannot rule the collision out statically; it reminds you of the constraint at every instantiation. Guarantee by construction that the ports use disjoint addresses (e.g. one writer per region, ping-pong buffers, a handshake before reading), or arbitrate the writers in front of a single-port Ram.",
            "let m = DualPortRam<u8, 256> { clk: clk, a_addr: x, ..., b_addr: y, ... }   // ⚠ W3006",
            "// ensure x != y whenever a_wr_en && b_wr_en, or:\nlet m = Ram<u8, 256> { ... }   // ✓ single writer, no collision",
        ),
        W3007 => Explanation::new(
            "External bidirectional signal read without synchronization",
            "The level of an 'inout' / 'opendrain' port is read directly; the other end of that line is a device outside the module's clock domain.",
            "An ordinary 'in' port is trusted to be in the module's domain (K2). A bidirectional pad is different: by definition it is driven by another device (an I2C slave, an SDRAM, a bus master) whose timing has nothing to do with this clock, so a direct read samples an asynchronous signal and can go metastable. The read is therefore treated as external (ADR-0051): pass it through sync() first and use the synchronised copy. Reads inside contracts and as the source of sync() are not reported. The warning is deliberate, not an error: a testbench or a design whose peer is known to share the clock may read the line directly.",
            "module I2c {\n    in  clk : clock\n    opendrain sda : bool\n    reg bit_r : bool = false\n    on clk { bit_r <= sda.read() }    // ⚠ W3007: asynchronous line sampled directly\n}",
            "module I2c {\n    in  clk : clock\n    opendrain sda : bool\n    wire sda_s : bool\n    sda_s = sync(sda.read(), clk)      // ✓ two-flop synchroniser\n    reg bit_r : bool = false\n    on clk { bit_r <= sda_s }\n}",
        ),
        W3008 => Explanation::new(
            "Deliberate trust downgrade",
            "A 'declassify(expr, \"reason\")' call lowers information from a higher trust level to public.",
            "Declassification is the one legitimate path across the trust lattice (ADR-0052), so the compiler never blocks it — but it never lets it pass silently either. Every call produces this warning with the source level and the reason the author wrote, which makes a security review a matter of reading the compiler output: the warnings are the complete list of places where classified information is intentionally revealed. There is nothing to fix unless the reason no longer holds; if it does not, remove the call and the flow becomes an E3009 error again.",
            "    out busy : bool @Debug\n    busy = declassify(state != IDLE, \"state visibility only\")   // ⚠ W3008: secret → public, reason recorded",
            "// Keep the call and review the reason; the warning is the audit trail, not a defect.",
        ),
        W3009 => Explanation::new(
            "Asynchronous reset assumed to be released synchronously",
            "A module that nothing in the unit instantiates has a 'reset = async' domain but no raw reset port, so nothing in Volt synchronizes the release of its automatic reset port.",
            "The automatic 'rst' / 'rst_n' port carries a contract: the reset it receives is released synchronously to the clock of the domain (ADR-0065 §1). Inside a Volt hierarchy the compiler keeps that promise — a parent connects its own synchronized reset to the child. At the root of the unit nobody does: if the port is wired to a pad or a power-on reset, the release is asynchronous and every flip-flop of the domain can leave reset in a different cycle. The warning makes that assumption visible once per module. If the reset really comes from outside, declare it as a raw port and the compiler adds the synchronizer; if an integrator already synchronizes it (IP delivered to another design), the warning documents the contract and needs no change.",
            "domain Core { clock = posedge, reset = async active_low }\nmodule Top {\n    in clk : clock @Core     // ⚠ W3009: 'rst_n' assumed synchronous to 'clk'\n}",
            "module Top {\n    in clk : clock @Core\n    in rst_n : reset(async, active_low)   // ✓ compiler synchronizes the release\n}",
        )
        .with_docs(&["https://volthdl.org/guide/cdc"]),
        W3010 => Explanation::new(
            "Synchronous reset shared by several clock domains",
            "Two or more clock domains with 'reset = sync' (the default) use the same generated 'rst' / 'rst_n' port, so its release is asynchronous to at least one of their clocks.",
            "A synchronous reset is sampled like data: every flip-flop sees it on its D input. When one reset port reaches flip-flops of two unrelated clocks, the edge on which it is released is asynchronous to at least one of them — the same hazard as an asynchronous reset release, only milder because the reset path itself is timed. At the top of a design the fix is one line: declare the raw reset explicitly and the compiler synchronizes its release to each clock. It stays a warning (ADR-0065, R5' decision) because a module instantiated under a parent that has flip-flops on the same clocks has no error-free form yet: its own raw port would be synchronized a second time (E3003) and its one automatic port can carry only one clock's synchronized reset. Only clocks that reset something count: a clock that drives nothing but the AsyncDualPortRam write side or an extern instance samples no reset.",
            "domain Sys { clock = posedge, reset = sync active_high }\ndomain Pix { clock = posedge, reset = sync active_high }\nmodule Video {\n    in sys_clk : clock @Sys    // ⚠ W3010: 'rst' sampled by 'sys_clk' and 'pix_clk'\n    in pix_clk : clock @Pix\n}",
            "module Video {\n    in sys_clk : clock @Sys\n    in pix_clk : clock @Pix\n    in rst : reset(sync, active_high)   // ✓ one release synchronizer per clock\n}",
        )
        .with_docs(&["https://volthdl.org/guide/cdc"]),
        W4001 => Explanation::new(
            "Unused signal",
            "Reserved for a netlist-level unused-signal check; this compiler does not emit it. A driven wire nobody reads is reported as W1001 (ADR-0075).",
            "An unread signal used to be reported twice, as W1001 and W4001, from the same usage data. Since ADR-0075 the source-level W1001 is the single report. The code stays reserved for an analysis on the elaborated design, which would also see readers that are themselves dead. Synthesis prunes an unread signal with any logic feeding only it; if keeping it is intentional (debug probe, reserved pin), prefix the name with '_'.",
            "wire spare : u4         // ⚠ W1001: no readers",
            "wire _spare : u4        // ✓ explicitly kept",
        ),
        W4002 => Explanation::new(
            "Register written but never read (netlist)",
            "Reserved for a netlist-level check; this compiler does not emit it. A register written but never read is reported as W1004 (ADR-0075).",
            "A write-only register used to be reported twice with the same message, as W1004 and W4002, from the same usage data. Since ADR-0075 the source-level W1004 is the single report. The code stays reserved for an analysis on the elaborated design, where a register may be read only by code that itself turned out to be dead. Synthesis strips the flip-flops of an unread register.",
            "reg stat : u8 = 0\non clk { stat <= s }   // ⚠ W1004: nobody reads stat",
            "result = stat           // ✓ observed in the netlist",
        ),
        W5001 => Explanation::new(
            "Contract cannot be monitored in simulation",
            "volt test runs contracts as simulation monitors (ADR-0064), but this contract's expression has no SystemVerilog form yet, so no monitor was generated for it.",
            "The rest of the design is still tested and every other contract is still monitored; only this one is silently absent from simulation, which is why it is reported. 'volt verify' needs the same SystemVerilog form and rejects the expression with E0003. Rewrite the contract with constructs that lower to SystemVerilog (operators, if-expressions, prev()) so both flows can check it.",
            "invariant: match a { 0 => true, _ => a != 7 }   // ⚠ W5001 in volt test",
            "invariant: a == 0 || a != 7                     // ✓ monitored and provable",
        )
        .with_note(
            "--no-contracts turns all monitors off and silences this warning.",
        ),
    }
}
