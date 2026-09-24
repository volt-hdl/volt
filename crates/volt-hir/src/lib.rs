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
pub mod extern_source;
pub mod handshake;
pub mod manifest_search;
pub mod pipeline;
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
pub mod unit_load;

pub use attrs::{check_attributes, UnenforcedLint, UNENFORCED_ATTRIBUTES};
pub use builtin::{BuiltinPort, BuiltinPrim, DomainRole, PortKind};
pub use consteval::{ConstEvaluator, ConstValue, MAX_ARRAY_LEN, MAX_WIDTH};
pub use constraints::{
    check_constraints, collect_constraints, Bridge, ClockConstraint, ConstraintResult, Crossing,
    CrossingClass, ModuleConstraints, PathKind, PathRule, ResetChain, Target,
};
pub use domain::{
    check_rdc, infer_domains, without_raw_reset_unused, DomainId, DomainInfo, DomainResult,
    DomainSource, InferVar,
};
pub use extern_source::{
    check_source_attributes, extern_source_decls, instantiated_externs, missing_sources,
    resolve_extern_sources, ExternSourceFile, FsSourceLocator, SourceLocator,
};
pub use handshake::check_handshakes;
pub use manifest_search::{
    find_manifest_dir, find_manifest_dir_in, SearchEnv, SearchStop, MANIFEST_DIR_ENV, MANIFEST_FILE,
};
pub use pipeline::{pre_resolve_checks, run_semantic_stages, SemanticStages};
pub use resolve::{
    resolve_file, resolve_unit, BuiltinKind, DefData, DefId, DefKind, ResolveResult, Scope,
    ScopeId, ScopeKind,
};
pub use sim::{check_tests, check_tests_with_files, collect_modules};
pub use sim_const::TestConsts;
pub use sim_load::{resolve_load_target, LoadTarget};
pub use sim_port::{
    const_test_value, describe_value, port_enum as sim_port_enum, test_port_width, PortWidth,
    ScalarKind,
};
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

/// Tanı toplayıcısının tek çıkış kapısı: önce özdeş tanılar katlanır
/// (ADR-0068 — `Span.ctx` dışında aynı olan kopyalar bir kez kalır),
/// sonra açılım bağlamı not olarak yazılır.
///
/// * Katlanmamış, açılmış `for` yinelemesine düşen tanı (ADR-0056):
///   "'for' döngüsünün i = 2 yinelemesinde" notu ve döngü deyimine
///   ikincil etiket. Kaynak konumu zaten kullanıcının yazdığı satırdır
///   (klon span'leri korur); not hangi kopyada olduğunu söyler.
/// * Katlanmış tanı: "bir kez raporlandı; N açılmış 'for' yinelemesinde
///   (i = 0..282)" / "N generic örneklemede" / "N özdeş kopya" notu.
///
/// Aynı tanıya ikinci kez uygulanmaz (not imleri).
pub fn annotate_generate(
    ast: &SourceFile,
    diagnostics: Vec<volt_diagnostics::Diagnostic>,
) -> Vec<volt_diagnostics::Diagnostic> {
    let mut diagnostics = diagnostics;
    volt_diagnostics::fold_duplicates(&mut diagnostics, 0);
    if ast.generate.iterations.is_empty() && diagnostics.iter().all(|d| d.folded_ctxs.is_empty()) {
        return diagnostics;
    }
    diagnostics
        .into_iter()
        .map(|d| annotate_one(ast, d))
        .collect()
}

fn annotate_one(ast: &SourceFile, d: volt_diagnostics::Diagnostic) -> volt_diagnostics::Diagnostic {
    use volt_diagnostics::NoteKind;
    let Some(ctx) = d.primary_span().map(|s| s.span.ctx) else {
        return d;
    };
    if d.notes
        .iter()
        .any(|n| n.text.contains(GENERATE_NOTE_MARK) || n.text.contains(FOLD_NOTE_MARK))
    {
        return d;
    }
    let note = if d.folded_ctxs.is_empty() {
        let chain = ast.generate.chain(ctx);
        if chain.is_empty() {
            return d;
        }
        let vars: Vec<String> = chain.iter().map(|(v, n)| format!("{v} = {n}")).collect();
        let vars = vars.join(", ");
        volt_diagnostics::lstr!(
            en: "in the unrolled 'for' iteration {vars} {GENERATE_NOTE_MARK}";
            tr: "'for' döngüsünün {vars} yinelemesinde {GENERATE_NOTE_MARK}"
        )
    } else {
        fold_note(ast, ctx, &d.folded_ctxs)
    };
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
}

/// Katlanmış tanının notu: kopyaların ctx'leri yineleme zincirine
/// (değişken başına min..max) ve köklerine (0 = elle yazılmış, başka =
/// monomorf klon) ayrıştırılır.
fn fold_note(ast: &SourceFile, primary_ctx: u16, folded: &[u16]) -> String {
    let n = folded.len() + 1;
    let mut ranges: Vec<(String, i128, i128)> = Vec::new();
    let mut roots: std::collections::BTreeSet<u16> = std::collections::BTreeSet::new();
    for &c in std::iter::once(&primary_ctx).chain(folded) {
        for (k, (var, val)) in ast.generate.chain(c).iter().enumerate() {
            match ranges.get_mut(k) {
                Some(r) => {
                    r.1 = r.1.min(*val);
                    r.2 = r.2.max(*val);
                }
                None => ranges.push((var.clone(), *val, *val)),
            }
        }
        roots.insert(root_ctx(ast, c));
    }
    let instantiations = roots.iter().filter(|&&r| r != 0).count();
    let ranges_txt = ranges
        .iter()
        .map(|(v, lo, hi)| {
            if lo == hi {
                format!("{v} = {lo}")
            } else {
                format!("{v} = {lo}..{hi}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    match (ranges.is_empty(), instantiations) {
        (false, 0) => volt_diagnostics::lstr!(
            en: "reported once; occurs in {n} unrolled 'for' iterations ({ranges_txt}) {FOLD_NOTE_MARK}";
            tr: "bir kez raporlandı; {n} açılmış 'for' yinelemesinde geçiyor ({ranges_txt}) {FOLD_NOTE_MARK}"
        ),
        (false, k) => volt_diagnostics::lstr!(
            en: "reported once; occurs in {n} copies: unrolled 'for' iterations ({ranges_txt}) across {k} generic instantiations {FOLD_NOTE_MARK}";
            tr: "bir kez raporlandı; {n} kopyada geçiyor: {k} generic örneklemedeki açılmış 'for' yinelemeleri ({ranges_txt}) {FOLD_NOTE_MARK}"
        ),
        (true, k) if k == n => volt_diagnostics::lstr!(
            en: "reported once; occurs in {n} generic instantiations {FOLD_NOTE_MARK}";
            tr: "bir kez raporlandı; {n} generic örneklemede geçiyor {FOLD_NOTE_MARK}"
        ),
        (true, _) => volt_diagnostics::lstr!(
            en: "reported once; {n} identical occurrences {FOLD_NOTE_MARK}";
            tr: "bir kez raporlandı; {n} özdeş kopya {FOLD_NOTE_MARK}"
        ),
    }
}

/// Yineleme zincirinin kökü: tabloda olmayan ilk ctx (0 ya da monomorf
/// klonun ctx'i); ctx tabloda değilse kendisi.
fn root_ctx(ast: &SourceFile, ctx: u16) -> u16 {
    let mut cur = ctx;
    while let Some(it) = ast.generate.iterations.get(&cur) {
        if it.parent == 0 || it.parent == cur {
            return 0;
        }
        cur = it.parent;
    }
    cur
}

/// Bağlam notunun tanınma imi (çift uygulamaya karşı).
const GENERATE_NOTE_MARK: &str = "(ADR-0056)";
/// Katlama notunun tanınma imi.
const FOLD_NOTE_MARK: &str = "(ADR-0068)";
