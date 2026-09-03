//! HIR: isim çözümleme, tip çıkarımı ve saat alanı (domain) çıkarımı.
//!
//! F1b: isim çözümleme (name-resolution.md) ve derleme zamanı
//! değerlendirme (const-eval.md §1-§6). F2a: tip gösterimi + çift yönlü
//! tip kontrolü (type-inference.md §1-§6) ve sürücü analizi (§11).
//! Domain çıkarımı F3.

pub mod consteval;
pub mod drivers;
pub mod resolve;
pub mod ty;
pub mod typeck;

pub use consteval::{ConstEvaluator, ConstValue, MAX_ARRAY_LEN, MAX_WIDTH};
pub use resolve::{
    resolve_file, BuiltinKind, DefData, DefId, DefKind, ResolveResult, Scope, ScopeId, ScopeKind,
};
pub use ty::{EnumId, ModuleId, StructId, Ty, TypeArena, TypeId};
pub use typeck::{typecheck, TypeckResult};

use volt_ast::SourceFile;
use volt_diagnostics::Diagnostic;

/// F1b + F2a anlamsal analiz sonucu.
#[derive(Debug)]
pub struct AnalysisResult {
    pub resolve: ResolveResult,
    pub typeck: TypeckResult,
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

/// İsim çözümleme + const değerlendirme + tip kontrolünü tek geçişte
/// koşturur.
pub fn analyze(ast: &SourceFile) -> AnalysisResult {
    let resolve = resolve_file(ast);
    let mut evaluator = ConstEvaluator::new(ast, &resolve);
    evaluator.eval_all_consts();
    evaluator.check_type_positions();
    let typeck = typecheck(ast, &resolve, &mut evaluator);

    let mut diagnostics = resolve.diagnostics.clone();
    diagnostics.extend(evaluator.diagnostics);
    diagnostics.extend(typeck.diagnostics.iter().cloned());
    AnalysisResult {
        resolve,
        typeck,
        diagnostics,
    }
}
