//! HIR: isim çözümleme, tip çıkarımı ve saat alanı (domain) çıkarımı.
//!
//! F1b: isim çözümleme (name-resolution.md) ve derleme zamanı
//! değerlendirme (const-eval.md §1-§6). F2a: tip gösterimi + çift yönlü
//! tip kontrolü (type-inference.md §1-§6) ve sürücü analizi (§11).
//! F2c: domain çıkarımı ve CDC kontrolü (domain-inference.md K1-K9).
//! F2f: bilgi akışı denetimi, trust_level (ADR-0052, K11).

pub mod attrs;
pub mod builtin;
pub mod consteval;
pub mod constraints;
pub mod domain;
pub mod drivers;
pub mod handshake;
pub mod manifest_search;
pub mod resolve;
pub mod sim;
mod sim_const;
mod sim_expr;
mod sim_load;
mod sim_port;
pub mod testdata;
pub mod timing;
pub mod trust;
pub mod ty;
pub mod typeck;
pub mod unit;

pub use attrs::{check_attributes, UnenforcedLint, UNENFORCED_ATTRIBUTES};
pub use builtin::{BuiltinPort, BuiltinPrim, DomainRole, PortKind};
pub use consteval::{ConstEvaluator, ConstValue, MAX_ARRAY_LEN, MAX_WIDTH};
pub use constraints::{
    check_constraints, collect_constraints, Bridge, ClockConstraint, ConstraintResult,
    ModuleConstraints, PathKind, PathRule, Target,
};
pub use domain::{
    check_rdc, infer_domains, without_raw_reset_unused, DomainId, DomainInfo, DomainResult,
    DomainSource, InferVar,
};
pub use handshake::check_handshakes;
pub use manifest_search::{
    find_manifest_dir, find_manifest_dir_in, SearchEnv, SearchStop, MANIFEST_DIR_ENV, MANIFEST_FILE,
};
pub use resolve::{
    resolve_file, resolve_unit, BuiltinKind, DefData, DefId, DefKind, ResolveResult, Scope,
    ScopeId, ScopeKind,
};
pub use sim::{check_tests, check_tests_with_files, collect_modules};
pub use sim_const::TestConsts;
pub use sim_load::{resolve_load_target, LoadTarget};
pub use sim_port::{const_test_value, describe_value, test_port_width, PortWidth, ScalarKind};
pub use testdata::{
    normalize_data_path, parse_readmemh, HexError, HexErrorReason, HexImage, TestFileError,
    TestFileLoader,
};
pub use timing::check_timing;
pub use trust::check_trust;
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

    // Güven seviyeleri (ADR-0052): saat çıkarımının sonucu üzerinde
    // bilgi akışı denetimi; trust_level/declassify yoksa hiç koşmaz.
    let trust_diags = trust::check_trust(ast, &resolve, &typeck, &domain);

    // Reset alanı denetimi (ADR-0065): saat çıkarımının sonucu üzerinde.
    let rdc_diags = domain::check_rdc(ast, &resolve, &domain);

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

    // Handshake protokolü (ADR-0050): üretici valid'i ready'ye
    // kombinasyonel bağlayamaz (E4007).
    let handshake_diags = handshake::check_handshakes(ast, &resolve);

    // Zamanlama kısıtları (ADR-0054): @timing / @false_path / @multicycle
    // biçim ve tutarlılık denetimi (E0017); W0022 yalnız --emit=sdc,xdc'de.
    let constraint_diags = constraints::check_constraints(ast);

    // Uygulanmayan nitelikler (ADR-0048): W0021 — çözümlemeden bağımsız,
    // tek dosya API'sinde varsayılan politika uyarıdır.
    let mut diagnostics = attrs::check_attributes(ast, UnenforcedLint::Warn);
    diagnostics.extend(without_raw_reset_unused(ast, &resolve.diagnostics));
    diagnostics.extend(evaluator.diagnostics);
    diagnostics.extend(typeck.diagnostics.iter().cloned());
    diagnostics.extend(domain.diagnostics.iter().cloned());
    diagnostics.extend(trust_diags);
    diagnostics.extend(rdc_diags);
    diagnostics.extend(test_diags);
    diagnostics.extend(timing_diags);
    diagnostics.extend(handshake_diags);
    diagnostics.extend(constraint_diags);
    let diagnostics = annotate_generate(ast, diagnostics);
    AnalysisResult {
        resolve,
        typeck,
        domain,
        diagnostics,
    }
}

/// Açılmış `for` yinelemesine düşen tanılara bağlam notu ekler
/// (ADR-0056): birincil span'in `ctx`'i `SourceFile::generate`
/// tablosundaysa "'for' döngüsünün i = 2 yinelemesinde" notu ve döngü
/// deyimine ikincil etiket. Kaynak konumu zaten kullanıcının yazdığı
/// satırdır (klon span'leri korur); not hangi kopyada olduğunu söyler.
/// Aynı tanıya ikinci kez uygulanmaz.
pub fn annotate_generate(
    ast: &SourceFile,
    diagnostics: Vec<volt_diagnostics::Diagnostic>,
) -> Vec<volt_diagnostics::Diagnostic> {
    use volt_diagnostics::NoteKind;
    if ast.generate.iterations.is_empty() {
        return diagnostics;
    }
    diagnostics
        .into_iter()
        .map(|d| {
            let Some(ctx) = d.primary_span().map(|s| s.span.ctx) else {
                return d;
            };
            let chain = ast.generate.chain(ctx);
            if chain.is_empty() || d.notes.iter().any(|n| n.text.contains(GENERATE_NOTE_MARK)) {
                return d;
            }
            let vars: Vec<String> = chain.iter().map(|(v, n)| format!("{v} = {n}")).collect();
            let vars = vars.join(", ");
            let note = volt_diagnostics::lstr!(
                en: "in the unrolled 'for' iteration {vars} {GENERATE_NOTE_MARK}";
                tr: "'for' döngüsünün {vars} yinelemesinde {GENERATE_NOTE_MARK}"
            );
            let d = d.with_note(NoteKind::Note, note);
            match ast.generate.outermost_span(ctx) {
                Some(span) if !d.spans.iter().any(|s| s.span == span) => d.with_secondary(
                    span,
                    volt_diagnostics::lstr!(
                        en: "'for' loop unrolled at compile time here";
                        tr: "'for' döngüsü burada derleme zamanında açıldı"
                    ),
                ),
                _ => d,
            }
        })
        .collect()
}

/// Bağlam notunun tanınma imi (çift uygulamaya karşı).
const GENERATE_NOTE_MARK: &str = "(ADR-0056)";
