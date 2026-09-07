//! Hata/uyarı tanıları: kod tablosu, 5 parçalı tanı modeli ve
//! insan/short/JSON çıktı üreticileri (cli-contract.md §5).

pub mod code;
pub mod diagnostic;
pub mod emit;
pub mod explain;
pub mod json;
pub mod messages;

pub use code::ErrorCode;
pub use diagnostic::{
    Applicability, Diagnostic, LabeledSpan, Note, NoteKind, Severity, Suggestion,
};
pub use emit::{render_human, render_short};
pub use explain::{render_explanation, render_list};
pub use json::{to_json_string, to_json_value};
pub use messages::{lang, set_lang, Lang};
