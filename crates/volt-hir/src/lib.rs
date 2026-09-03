//! HIR: isim çözümleme, tip çıkarımı ve saat alanı (domain) çıkarımı.
//!
//! F1b kapsamı: isim çözümleme (name-resolution.md) ve derleme zamanı
//! değerlendirme (const-eval.md §1-§6). Tip ve domain çıkarımı F2.

pub mod consteval;
pub mod resolve;

pub use consteval::{ConstEvaluator, ConstValue, MAX_ARRAY_LEN, MAX_WIDTH};
pub use resolve::{
    resolve_file, BuiltinKind, DefData, DefId, DefKind, ResolveResult, Scope, ScopeId, ScopeKind,
};

use volt_ast::SourceFile;
use volt_diagnostics::Diagnostic;

/// F1b anlamsal analiz sonucu.
#[derive(Debug)]
pub struct AnalysisResult {
    pub resolve: ResolveResult,
    pub diagnostics: Vec<Diagnostic>,
}

impl AnalysisResult {
    pub fn error_codes(&self) -> Vec<&'static str> {
        self.diagnostics.iter().map(|d| d.code.as_str()).collect()
    }

    /// Yalnız hatalar (uyarılar hariç).
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| !d.code.is_warning())
    }
}

/// İsim çözümleme + const değerlendirmeyi tek geçişte koşturur.
pub fn analyze(ast: &SourceFile) -> AnalysisResult {
    let resolve = resolve_file(ast);
    let mut evaluator = ConstEvaluator::new(ast, &resolve);
    evaluator.eval_all_consts();
    evaluator.check_type_positions();

    let mut diagnostics = resolve.diagnostics.clone();
    diagnostics.extend(evaluator.diagnostics);
    AnalysisResult {
        resolve,
        diagnostics,
    }
}
