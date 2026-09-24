//! Hedef dillerin ayrılmış sözcükleri (ADR-0078) — SV emitter'ı, `@mmio`
//! denetimi ve simülasyon düzeneği bu tek tablodan okur.
//!
//! Dört küme, dört ayrı sonuç:
//! - [`is_sv_keyword`]: IEEE 1800-2017 Annex B. Bu adla bir SV tanımlayıcısı
//!   geçersizdir → E1013 (Volt reddeder).
//! - [`sw_keyword_language`]: Rust 2021/2024 ve C11/C++20 anahtar sözcükleri.
//!   `@mmio` register/alan adı üretilen sürücüde parametre ya da işlev adı
//!   olur → E1013.
//! - [`verilator_symbol`]: Verilator'ın C++ çakışması saydığı sözcükler
//!   (`V3LanguageWords.h`). SV geçerlidir; Verilator üst modül portunu
//!   `__SYM__<ad>` diye yeniden adlandırır ve SYMRSVDWORD uyarır.
//! - [`is_verilator_model_member`]: Verilator'ın ürettiği model sınıfının
//!   üyeleri. Üst modül portu bu adı taşırsa model C++'ı derlenmez.

/// IEEE 1800-2017 Annex B ayrılmış sözcükleri (248). Verilog-2005
/// (1364-2005) kümesini kapsar; 1800-2023 yeni sözcük eklemedi. Sıralı —
/// ikili arama.
pub const SV_KEYWORDS: &[&str] = &[
    "accept_on",
    "alias",
    "always",
    "always_comb",
    "always_ff",
    "always_latch",
    "and",
    "assert",
    "assign",
    "assume",
    "automatic",
    "before",
    "begin",
    "bind",
    "bins",
    "binsof",
    "bit",
    "break",
    "buf",
    "bufif0",
    "bufif1",
    "byte",
    "case",
    "casex",
    "casez",
    "cell",
    "chandle",
    "checker",
    "class",
    "clocking",
    "cmos",
    "config",
    "const",
    "constraint",
    "context",
    "continue",
    "cover",
    "covergroup",
    "coverpoint",
    "cross",
    "deassign",
    "default",
    "defparam",
    "design",
    "disable",
    "dist",
    "do",
    "edge",
    "else",
    "end",
    "endcase",
    "endchecker",
    "endclass",
    "endclocking",
    "endconfig",
    "endfunction",
    "endgenerate",
    "endgroup",
    "endinterface",
    "endmodule",
    "endpackage",
    "endprimitive",
    "endprogram",
    "endproperty",
    "endsequence",
    "endspecify",
    "endtable",
    "endtask",
    "enum",
    "event",
    "eventually",
    "expect",
    "export",
    "extends",
    "extern",
    "final",
    "first_match",
    "for",
    "force",
    "foreach",
    "forever",
    "fork",
    "forkjoin",
    "function",
    "generate",
    "genvar",
    "global",
    "highz0",
    "highz1",
    "if",
    "iff",
    "ifnone",
    "ignore_bins",
    "illegal_bins",
    "implements",
    "implies",
    "import",
    "incdir",
    "include",
    "initial",
    "inout",
    "input",
    "inside",
    "instance",
    "int",
    "integer",
    "interconnect",
    "interface",
    "intersect",
    "join",
    "join_any",
    "join_none",
    "large",
    "let",
    "liblist",
    "library",
    "local",
    "localparam",
    "logic",
    "longint",
    "macromodule",
    "matches",
    "medium",
    "modport",
    "module",
    "nand",
    "negedge",
    "nettype",
    "new",
    "nexttime",
    "nmos",
    "nor",
    "noshowcancelled",
    "not",
    "notif0",
    "notif1",
    "null",
    "or",
    "output",
    "package",
    "packed",
    "parameter",
    "pmos",
    "posedge",
    "primitive",
    "priority",
    "program",
    "property",
    "protected",
    "pull0",
    "pull1",
    "pulldown",
    "pullup",
    "pulsestyle_ondetect",
    "pulsestyle_onevent",
    "pure",
    "rand",
    "randc",
    "randcase",
    "randsequence",
    "rcmos",
    "real",
    "realtime",
    "ref",
    "reg",
    "reject_on",
    "release",
    "repeat",
    "restrict",
    "return",
    "rnmos",
    "rpmos",
    "rtran",
    "rtranif0",
    "rtranif1",
    "s_always",
    "s_eventually",
    "s_nexttime",
    "s_until",
    "s_until_with",
    "scalared",
    "sequence",
    "shortint",
    "shortreal",
    "showcancelled",
    "signed",
    "small",
    "soft",
    "solve",
    "specify",
    "specparam",
    "static",
    "string",
    "strong",
    "strong0",
    "strong1",
    "struct",
    "super",
    "supply0",
    "supply1",
    "sync_accept_on",
    "sync_reject_on",
    "table",
    "tagged",
    "task",
    "this",
    "throughout",
    "time",
    "timeprecision",
    "timeunit",
    "tran",
    "tranif0",
    "tranif1",
    "tri",
    "tri0",
    "tri1",
    "triand",
    "trior",
    "trireg",
    "type",
    "typedef",
    "union",
    "unique",
    "unique0",
    "unsigned",
    "until",
    "until_with",
    "untyped",
    "use",
    "uwire",
    "var",
    "vectored",
    "virtual",
    "void",
    "wait",
    "wait_order",
    "wand",
    "weak",
    "weak0",
    "weak1",
    "while",
    "wildcard",
    "wire",
    "with",
    "within",
    "wor",
    "xnor",
    "xor",
];

/// Rust anahtar sözcükleri: 2021 katı + ayrılmış kümesi ve 2024'ün `gen`'i
/// (üretilen sürücü hangi sürümde derlenirse derlensin). `union`, `raw`,
/// `macro_rules` gibi bağlamsal sözcükler tanımlayıcı olabilir, listede yok
/// (ölçüm: `rustc --edition 2021`, ADR-0078).
pub const RUST_KEYWORDS: &[&str] = &[
    "Self", "abstract", "as", "async", "await", "become", "box", "break", "const", "continue",
    "crate", "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if",
    "impl", "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub",
    "ref", "return", "self", "static", "struct", "super", "trait", "true", "try", "type", "typeof",
    "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

/// C11 ve C++20 anahtar sözcükleri + `<stdbool.h>`/`<stddef.h>` makroları.
/// Üretilen başlık `extern "C"` korumasıyla C++'ta da derlenir, bu yüzden
/// iki küme birlikte (ölçüm: `gcc -std=c11`, `g++ -std=c++20`, ADR-0078).
pub const C_CPP_KEYWORDS: &[&str] = &[
    "NULL",
    "_Alignas",
    "_Alignof",
    "_Atomic",
    "_Bool",
    "_Complex",
    "_Generic",
    "_Imaginary",
    "_Noreturn",
    "_Static_assert",
    "_Thread_local",
    "alignas",
    "alignof",
    "and",
    "and_eq",
    "asm",
    "auto",
    "bitand",
    "bitor",
    "bool",
    "break",
    "case",
    "catch",
    "char",
    "char16_t",
    "char32_t",
    "char8_t",
    "class",
    "co_await",
    "co_return",
    "co_yield",
    "compl",
    "concept",
    "const",
    "const_cast",
    "consteval",
    "constexpr",
    "constinit",
    "continue",
    "decltype",
    "default",
    "delete",
    "do",
    "double",
    "dynamic_cast",
    "else",
    "enum",
    "explicit",
    "export",
    "extern",
    "false",
    "float",
    "for",
    "friend",
    "goto",
    "if",
    "inline",
    "int",
    "long",
    "mutable",
    "namespace",
    "new",
    "noexcept",
    "not",
    "not_eq",
    "nullptr",
    "operator",
    "or",
    "or_eq",
    "private",
    "protected",
    "public",
    "register",
    "reinterpret_cast",
    "requires",
    "restrict",
    "return",
    "short",
    "signed",
    "sizeof",
    "static",
    "static_assert",
    "static_cast",
    "struct",
    "switch",
    "template",
    "this",
    "thread_local",
    "throw",
    "true",
    "try",
    "typedef",
    "typeid",
    "typename",
    "typeof",
    "union",
    "unsigned",
    "using",
    "virtual",
    "void",
    "volatile",
    "wchar_t",
    "while",
    "xor",
    "xor_eq",
];

/// Verilator 5.050 `V3LanguageWords.h` tanımlayıcı sözcükleri (C++ anahtar
/// sözcüğü, "C++ common word", SystemC; ön işlemci yönergeleri ve `std::`
/// girdileri hariç). Verilator bu adla bir üst modül portunu C++ modelinde
/// `__SYM__<ad>` diye adlandırır ve SYMRSVDWORD uyarır (varsayılan açık).
pub const VERILATOR_CPP_WORDS: &[&str] = &[
    "abort",
    "alignas",
    "alignof",
    "and",
    "and_eq",
    "asm",
    "atomic_cancel",
    "atomic_commit",
    "atomic_noexcept",
    "auto",
    "bit_vector",
    "bitand",
    "bitor",
    "bool",
    "break",
    "case",
    "catch",
    "cdecl",
    "char",
    "char16_t",
    "char32_t",
    "class",
    "compl",
    "complex",
    "concept",
    "const",
    "const_cast",
    "const_iterator",
    "constexpr",
    "continue",
    "decltype",
    "default",
    "delete",
    "do",
    "double",
    "dynamic_cast",
    "else",
    "enum",
    "explicit",
    "export",
    "extern",
    "false",
    "far",
    "float",
    "for",
    "friend",
    "goto",
    "huge",
    "if",
    "import",
    "inline",
    "int",
    "interrupt",
    "iterator",
    "long",
    "module",
    "mutable",
    "namespace",
    "near",
    "new",
    "noexcept",
    "not",
    "not_eq",
    "nullptr",
    "operator",
    "or",
    "or_eq",
    "override",
    "pascal",
    "private",
    "protected",
    "public",
    "queue",
    "reference",
    "register",
    "reinterpret_cast",
    "requires",
    "restrict",
    "return",
    "sc_clock",
    "sc_in",
    "sc_inout",
    "sc_out",
    "sc_signal",
    "sensitive",
    "sensitive_neg",
    "sensitive_pos",
    "short",
    "signed",
    "sizeof",
    "stack",
    "static",
    "static_assert",
    "static_cast",
    "struct",
    "switch",
    "synchronized",
    "template",
    "this",
    "thread_local",
    "throw",
    "transaction_safe",
    "transaction_safe_dynamic",
    "true",
    "try",
    "type_info",
    "typedef",
    "typeid",
    "typename",
    "uint16_t",
    "uint32_t",
    "uint8_t",
    "union",
    "unsigned",
    "using",
    "virtual",
    "void",
    "volatile",
    "wchar_t",
    "while",
    "xor",
    "xor_eq",
];

/// Verilator'ın ürettiği model sınıfının (`V<Top>`, taban `VerilatedModel`)
/// üye adları. Üst modül portu model sınıfında aynı adlı bir üye olur;
/// bunlarla çakışan port modelin C++'ını derlenemez yapar (Verilator
/// korumaz; ölçüm 5.050, ADR-0078).
pub const VERILATOR_MODEL_MEMBERS: &[&str] = &[
    "atClone",
    "contextp",
    "eval",
    "eval_end_step",
    "eval_step",
    "eventsPending",
    "final",
    "hierName",
    "m_context",
    "modelName",
    "name",
    "nextTimeSlot",
    "prepareClone",
    "rootp",
    "threads",
    "trace",
    "traceBaseModel",
    "traceCapable",
    "traceConfig",
    "vlSymsp",
];

fn contains(list: &[&str], name: &str) -> bool {
    list.binary_search(&name).is_ok()
}

/// `name` bir SystemVerilog ayrılmış sözcüğü mü (büyük/küçük harf duyarlı;
/// SV anahtar sözcüklerinin hepsi küçük harf).
pub fn is_sv_keyword(name: &str) -> bool {
    contains(SV_KEYWORDS, name)
}

/// `name` üretilen yazılım sürücüsünde (Rust, C, C++) ayrılmışsa dilin adı.
pub fn sw_keyword_language(name: &str) -> Option<&'static str> {
    if contains(RUST_KEYWORDS, name) {
        Some("Rust")
    } else if contains(C_CPP_KEYWORDS, name) {
        Some("C/C++")
    } else {
        None
    }
}

/// Verilator'ın C++ modelinde üst modül portunun adı: çakışan sözcükte
/// `__SYM__<ad>`, değilse aynen.
pub fn verilator_symbol(name: &str) -> std::borrow::Cow<'_, str> {
    if is_verilator_cpp_word(name) {
        std::borrow::Cow::Owned(format!("__SYM__{name}"))
    } else {
        std::borrow::Cow::Borrowed(name)
    }
}

/// Verilator bu adı C++ sözcüğü sayıp yeniden adlandırır mı.
pub fn is_verilator_cpp_word(name: &str) -> bool {
    contains(VERILATOR_CPP_WORDS, name)
}

/// Üst modül `top`'un portu `name` Verilator modelinin bir üyesiyle
/// (ya da sınıf adı `V<top>` ile) çakışıyor mu.
pub fn is_verilator_model_member(top: &str, name: &str) -> bool {
    contains(VERILATOR_MODEL_MEMBERS, name)
        || name.strip_prefix('V').is_some_and(|rest| rest == top)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted_unique(list: &[&str]) -> bool {
        list.windows(2).all(|w| w[0] < w[1])
    }

    #[test]
    fn lists_are_sorted_for_binary_search() {
        assert!(sorted_unique(SV_KEYWORDS));
        assert!(sorted_unique(RUST_KEYWORDS));
        assert!(sorted_unique(C_CPP_KEYWORDS));
        assert!(sorted_unique(VERILATOR_CPP_WORDS));
        assert!(sorted_unique(VERILATOR_MODEL_MEMBERS));
    }

    #[test]
    fn sv_list_is_annex_b() {
        assert_eq!(SV_KEYWORDS.len(), 248);
        for w in [
            "packed",
            "always_ff",
            "pulsestyle_ondetect",
            "table",
            "global",
        ] {
            assert!(is_sv_keyword(w), "{w}");
        }
        for w in ["Packed", "data", "interrupt", "fifo", "ram", "delete"] {
            assert!(!is_sv_keyword(w), "{w}");
        }
    }

    #[test]
    fn sw_keywords_name_their_language() {
        assert_eq!(sw_keyword_language("mod"), Some("Rust"));
        assert_eq!(sw_keyword_language("gen"), Some("Rust"));
        assert_eq!(sw_keyword_language("default"), Some("C/C++"));
        assert_eq!(sw_keyword_language("class"), Some("C/C++"));
        assert_eq!(sw_keyword_language("union"), Some("C/C++"));
        assert_eq!(sw_keyword_language("enable"), None);
        assert_eq!(sw_keyword_language("raw"), None);
    }

    #[test]
    fn verilator_names() {
        assert_eq!(verilator_symbol("interrupt"), "__SYM__interrupt");
        assert_eq!(verilator_symbol("char"), "__SYM__char");
        assert_eq!(verilator_symbol("data"), "data");
        assert!(is_verilator_model_member("Top", "eval"));
        assert!(is_verilator_model_member("Top", "VTop"));
        assert!(!is_verilator_model_member("Top", "VT"));
        assert!(!is_verilator_model_member("Top", "valid"));
    }
}
