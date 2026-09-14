//! HIR: isim çözümleme, tip çıkarımı ve saat alanı (domain) çıkarımı.
//!
//! F1b: isim çözümleme (name-resolution.md) ve derleme zamanı
//! değerlendirme (const-eval.md §1-§6). F2a: tip gösterimi + çift yönlü
//! tip kontrolü (type-inference.md §1-§6) ve sürücü analizi (§11).
//! F2c: domain çıkarımı ve CDC kontrolü (domain-inference.md K1-K9).

pub mod attrs;
pub mod builtin;
pub mod consteval;
pub mod domain;
pub mod drivers;
pub mod resolve;
pub mod sim;
pub mod timing;
pub mod ty;
pub mod typeck;
pub mod unit;

pub use attrs::{check_attributes, UnenforcedLint, UNENFORCED_ATTRIBUTES};
pub use builtin::{BuiltinPort, BuiltinPrim, DomainRole, PortKind};
pub use consteval::{ConstEvaluator, ConstValue, MAX_ARRAY_LEN, MAX_WIDTH};
pub use domain::{infer_domains, DomainId, DomainInfo, DomainResult, DomainSource, InferVar};
pub use resolve::{
    resolve_file, resolve_unit, BuiltinKind, DefData, DefId, DefKind, ResolveResult, Scope,
    ScopeId, ScopeKind,
};
pub use sim::{check_tests, collect_modules};
pub use timing::check_timing;
pub use ty::{EnumId, ModuleId, StructId, Ty, TypeArena, TypeId};
pub use typeck::{typecheck, TypeckResult};
pub use unit::{check_imports, FileScope, ImportResult, UnitInfo};

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
    analyze_with(ast, resolve_file(ast))
}

/// `analyze`'in derleme birimi biçimi (ADR-0042): `scopes` dosya başına
/// import görünürlüğü (`check_imports`).
pub fn analyze_unit(
    ast: &SourceFile,
    scopes: &std::collections::HashMap<volt_span::FileId, FileScope>,
) -> AnalysisResult {
    analyze_with(ast, resolve_unit(ast, scopes))
}

fn analyze_with(ast: &SourceFile, resolve: ResolveResult) -> AnalysisResult {
    let mut evaluator = ConstEvaluator::new(ast, &resolve);
    evaluator.eval_all_consts();
    evaluator.check_type_positions();
    let typeck = typecheck(ast, &resolve, &mut evaluator);
    let domain = infer_domains(ast, &resolve, &typeck);

    // Test blokları (ADR-0033): dosyada hiç modül yoksa testler kardeş
    // dosyanın modüllerini kullanıyordur — modül-varlık denetimi atlanır
    // (sürücü, kardeş dosyayı yükleyip tam denetimi kendisi yapar).
    let has_modules = ast
        .items
        .iter()
        .any(|i| matches!(ast.items_arena[*i].kind, volt_ast::ItemKind::Module(_)));
    let test_diags = sim::check_tests(&[ast], ast, !has_modules);

    // L1 zamanlama (ADR-0037): yalnız @strict_timing modüllerinde çalışır.
    let timing_diags = timing::check_timing(ast, &resolve);

    // Uygulanmayan nitelikler (ADR-0048): W0021 — çözümlemeden bağımsız,
    // tek dosya API'sinde varsayılan politika uyarıdır.
    let mut diagnostics = attrs::check_attributes(ast, UnenforcedLint::Warn);
    diagnostics.extend(resolve.diagnostics.iter().cloned());
    diagnostics.extend(evaluator.diagnostics);
    diagnostics.extend(typeck.diagnostics.iter().cloned());
    diagnostics.extend(domain.diagnostics.iter().cloned());
    diagnostics.extend(test_diags);
    diagnostics.extend(timing_diags);
    AnalysisResult {
        resolve,
        typeck,
        domain,
        diagnostics,
    }
}
