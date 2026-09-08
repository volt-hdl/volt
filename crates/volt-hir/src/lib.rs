//! HIR: isim çözümleme, tip çıkarımı ve saat alanı (domain) çıkarımı.
//!
//! F1b: isim çözümleme (name-resolution.md) ve derleme zamanı
//! değerlendirme (const-eval.md §1-§6). F2a: tip gösterimi + çift yönlü
//! tip kontrolü (type-inference.md §1-§6) ve sürücü analizi (§11).
//! F2c: domain çıkarımı ve CDC kontrolü (domain-inference.md K1-K9).

pub mod builtin;
pub mod consteval;
pub mod domain;
pub mod drivers;
pub mod resolve;
pub mod ty;
pub mod typeck;

pub use builtin::{BuiltinPort, BuiltinPrim, DomainRole, PortKind};
pub use consteval::{ConstEvaluator, ConstValue, MAX_ARRAY_LEN, MAX_WIDTH};
pub use domain::{infer_domains, DomainId, DomainInfo, DomainResult, DomainSource, InferVar};
pub use resolve::{
    resolve_file, BuiltinKind, DefData, DefId, DefKind, ResolveResult, Scope, ScopeId, ScopeKind,
};
pub use ty::{EnumId, ModuleId, StructId, Ty, TypeArena, TypeId};
pub use typeck::{typecheck, TypeckResult};

use volt_ast::SourceFile;
use volt_diagnostics::Diagnostic;

/// F1b + F2a + F2c anlamsal analiz sonucu.
#[derive(Debug)]
pub struct AnalysisResult {
    pub resolve: ResolveResult,
    pub typeck: TypeckResult,
    pub domain: DomainResult,
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

/// İsim çözümleme + const değerlendirme + tip kontrolü + domain
/// çıkarımını tek geçişte koşturur.
pub fn analyze(ast: &SourceFile) -> AnalysisResult {
    let resolve = resolve_file(ast);
    let mut evaluator = ConstEvaluator::new(ast, &resolve);
    evaluator.eval_all_consts();
    evaluator.check_type_positions();
    let typeck = typecheck(ast, &resolve, &mut evaluator);
    let domain = infer_domains(ast, &resolve, &typeck);

    let mut diagnostics = resolve.diagnostics.clone();
    diagnostics.extend(evaluator.diagnostics);
    diagnostics.extend(typeck.diagnostics.iter().cloned());
    diagnostics.extend(domain.diagnostics.iter().cloned());
    AnalysisResult {
        resolve,
        typeck,
        domain,
        diagnostics,
    }
}
