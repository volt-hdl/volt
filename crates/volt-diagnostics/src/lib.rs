//! Hata/uyarı tanıları: kod tablosu, 5 parçalı tanı modeli ve
//! insan/short/JSON çıktı üreticileri (cli-contract.md §5).

pub mod cap;
pub mod code;
pub mod diagnostic;
pub mod emit;
pub mod explain;
pub mod fold;
pub mod grammar;
pub mod json;
pub mod messages;

pub use cap::{cap_diagnostics, LSP_MAX_DIAGNOSTICS};
pub use code::ErrorCode;
pub use diagnostic::{
    apply_edits, Applicability, Diagnostic, Edit, EditKind, LabeledSpan, Note, NoteKind, Severity,
    Suggestion,
};
pub use emit::{render_human, render_short};
pub use explain::{render_explanation, render_list};
pub use fold::{fold_duplicates, same_identity};
pub use grammar::a_an;
pub use json::{to_json_string, to_json_value};
pub use messages::{lang, set_lang, Lang};
