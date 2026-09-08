//! Öğreten editör içerikleri (Volt-UX-Anayasası §"LSP: Öğreten Editör"):
//! anahtar kelime hover metinleri ve stdlib modül referansı
//! (docs/stdlib.md ile eşzamanlı tutulur).

use volt_syntax::TokenKind;

/// Anahtar kelime / operatör hover metni. Kullanıcıya kavramı öğretir,
/// gramer tekrarı yapmaz.
pub fn keyword_doc(kind: TokenKind) -> Option<&'static str> {
    Some(match kind {
        TokenKind::KwClock => "**clock** — Clock signal. Domain is inferred automatically.",
        TokenKind::KwReg => "**reg** — Register. Updated on clock edge.",
        TokenKind::KwWire => "**wire** — Named combinational signal; must be driven exactly once.",
        TokenKind::KwLet => "**let** — Local binding for an expression or a module instance.",
        TokenKind::KwOn => {
            "**on** — Sequential block. Statements run on the clock edge; use `<=` inside."
        }
        TokenKind::KwComb => "**comb** — Combinational block. Use `=` inside; no clock involved.",
        TokenKind::KwDomain => {
            "**domain** — Declares a clock domain. Signals from different domains \
             cannot mix without an explicit synchronizer (`sync()`, `AsyncFifo`, ...)."
        }
        TokenKind::KwModule => "**module** — Hardware module with ports and a body.",
        TokenKind::KwIn => "**in** — Input port.",
        TokenKind::KwOut => "**out** — Output port; must be driven.",
        TokenKind::KwInout => "**inout** — Bidirectional port.",
        TokenKind::KwReset => "**reset** — Reset signal; polarity and sync come from the domain.",
        TokenKind::KwIf => "**if** — Conditional; both branches must drive the same signals.",
        TokenKind::KwMatch => "**match** — Exhaustive pattern matching.",
        TokenKind::KwFor => "**for** — Compile-time generate loop.",
        TokenKind::KwConst => "**const** — Compile-time constant.",
        TokenKind::KwBits => "**bits<N>** — Raw bit vector; no arithmetic, only bitwise ops.",
        TokenKind::Le => {
            "**<=** — Non-blocking assignment (clocked). Inside `on` blocks the value \
             updates on the next clock edge. In expressions: less-or-equal."
        }
        TokenKind::Eq => "**=** — Continuous assignment (combinational).",
        TokenKind::At => "**@** — Clock domain annotation, e.g. `@Fast`.",
        _ => return None,
    })
}

/// Stdlib modülü referans girdisi (docs/stdlib.md ile aynı imzalar).
pub struct StdlibDoc {
    pub name: &'static str,
    pub signature: &'static str,
    pub doc: &'static str,
}

/// 11 yerleşik primitif — ADR-0027/ADR-0029, docs/stdlib.md.
pub const STDLIB: &[StdlibDoc] = &[
    StdlibDoc {
        name: "SyncFifo",
        signature: "SyncFifo<T, DEPTH>",
        doc: "Single-clock FIFO with an occupancy counter. \
              DEPTH: power of two, 2..=65536. See docs/stdlib.md#syncfifo.",
    },
    StdlibDoc {
        name: "Ram",
        signature: "Ram<T, DEPTH>",
        doc: "Single-port synchronous RAM (one-cycle read latency). \
              See docs/stdlib.md#ram.",
    },
    StdlibDoc {
        name: "DualPortRam",
        signature: "DualPortRam<T, DEPTH>",
        doc: "Dual-port RAM: one write port, one read port, same clock. \
              See docs/stdlib.md#dualportram.",
    },
    StdlibDoc {
        name: "Counter",
        signature: "Counter<WIDTH>",
        doc: "WIDTH-bit wrap-around counter with enable/clear and an \
              overflow pulse. WIDTH: 1..=64. See docs/stdlib.md#counter.",
    },
    StdlibDoc {
        name: "ShiftRegister",
        signature: "ShiftRegister<T, LEN>",
        doc: "LEN-stage shift register with parallel taps. LEN: 2..=64. \
              See docs/stdlib.md#shiftregister.",
    },
    StdlibDoc {
        name: "RoundRobinArbiter",
        signature: "RoundRobinArbiter<N>",
        doc: "Fair N-way arbiter; grant rotates after each served request. \
              N: 2..=64. See docs/stdlib.md#roundrobinarbiter.",
    },
    StdlibDoc {
        name: "PriorityArbiter",
        signature: "PriorityArbiter<N>",
        doc: "Fixed-priority N-way arbiter; lowest index wins. \
              N: 2..=64. See docs/stdlib.md#priorityarbiter.",
    },
    StdlibDoc {
        name: "EdgeDetect",
        signature: "EdgeDetect",
        doc: "Rising/falling edge detector producing one-cycle pulses. \
              See docs/stdlib.md#edgedetect.",
    },
    StdlibDoc {
        name: "AsyncFifo",
        signature: "AsyncFifo<T, DEPTH>",
        doc: "CDC-safe dual-clock FIFO (Gray-coded pointers). The safe way \
              to move multi-bit data between domains. \
              See docs/stdlib.md#asyncfifo-cdc.",
    },
    StdlibDoc {
        name: "HandshakeSync",
        signature: "HandshakeSync<T>",
        doc: "CDC 4-phase handshake for single-word transfers between \
              domains. See docs/stdlib.md#handshakesync-cdc.",
    },
    StdlibDoc {
        name: "PulseSync",
        signature: "PulseSync",
        doc: "CDC single-pulse synchronizer (toggle + edge detect). \
              See docs/stdlib.md#pulsesync-cdc.",
    },
];

pub fn stdlib_doc(name: &str) -> Option<&'static StdlibDoc> {
    STDLIB.iter().find(|d| d.name == name)
}
