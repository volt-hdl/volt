//! Kapılı anlamsal boru hattı (ADR-0070) — `volt check`/`volt build`
//! (volt-driver `compile()`) ve editör (volt-lsp) AYNI fonksiyonu
//! çağırır. Önceden her tüketici aşama listesini kendi kopyalıyordu;
//! kopya zamanla saptı (LSP'de güven seviyesi, zamanlama, handshake ve
//! test denetimleri yoktu; katlama hiç yoktu). Aşama eklemenin tek yeri
//! burasıdır.
//!
//! Sıra: resolve → const+typeck → domain (+ güven, RDC) → zamanlama,
//! handshake, test blokları, kısıtlar. SV eşleme sınırları (E0003) bu
//! aşamaların ardından çıktısız emit doğrulamasıyla gelir (sürücü ve
//! LSP `volt_sv_emit::validate_unit`/`emit_unit`). Hatalı
//! aşamadan sonrakiler koşmaz (kaskad tanı önlemi); çözümleme sonucu
//! her zaman döner (LSP hover/tanım bunu kullanır).

use std::collections::HashMap;

use volt_ast::SourceFile;
use volt_diagnostics::{Diagnostic, Severity};

use crate::{ConstEvaluator, ConstraintResult, DomainResult, ResolveResult, TestFileLoader};
use crate::{DefId, TypeckResult, UnenforcedLint};

/// Aşama ürünleri: `typeck`/`domain`/`constraints` yalnız önceki
/// aşamalar hatasızsa doludur.
pub struct SemanticStages {
    pub resolve: ResolveResult,
    pub typeck: Option<TypeckResult>,
    pub domain: Option<DomainResult>,
    pub constraints: Option<ConstraintResult>,
    /// `@strict_timing` kesin gecikmeleri (ADR-0037); `domain` ile aynı
    /// kapıdan geçer. Editör gecikme ipucu kaynağı (ADR-0091).
    pub delays: Option<HashMap<DefId, u32>>,
}

fn count_errors(diags: &[Diagnostic]) -> usize {
    diags
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count()
}

/// Çözümlemeden bağımsız ön denetimler: uygulanmayan nitelikler
/// (ADR-0048, W0021). Parse hatasızsa, çözümlemeden önce koşar.
pub fn pre_resolve_checks(ast: &SourceFile, lint: UnenforcedLint) -> Vec<Diagnostic> {
    let mut out = crate::check_attributes(ast, lint);
    // `@source` biçimi (ADR-0076) — dosya denetimi sürücüde.
    out.extend(crate::extern_source::check_source_attributes(ast));
    out
}

/// Aşama 2-4 + çözümleme sonrası denetimler. `resolve` çağıran
/// tarafından üretilir (sürücü: birim modu `resolve_unit`, LSP: tek
/// dosya `resolve_file`). Tanılar `out`'a eklenir.
///
/// `tests`: test bloklarının hedefi ([`crate::unit_load::TestTarget`]).
/// `Full` ise test blokları `volt test` ile aynı tam denetimden geçer
/// (bilinmeyen DUT, port); kardeş `X.volt` varsa onun modülleri de.
pub fn run_semantic_stages(
    ast: &SourceFile,
    resolve: ResolveResult,
    test_files: Option<&dyn TestFileLoader>,
    tests: &crate::unit_load::TestTarget,
    out: &mut Vec<Diagnostic>,
) -> SemanticStages {
    let mut stages = SemanticStages {
        resolve,
        typeck: None,
        domain: None,
        constraints: None,
        delays: None,
    };
    let resolve = &stages.resolve;
    // ADR-0065: ham reset portu senkronizörce örtük okunur (W1001 değil).
    out.extend(crate::without_raw_reset_unused(ast, &resolve.diagnostics));
    if count_errors(&resolve.diagnostics) > 0 {
        // Tip denetimi koşmaz; ertelenmiş kesin enum E0014'leri (ADR-0075).
        out.extend(crate::typeck::gated_enum_exhaustiveness(ast, resolve));
        return stages;
    }

    // ── Aşama 3: const eval + tip kontrolü ──
    let mut evaluator = ConstEvaluator::new(ast, resolve);
    evaluator.eval_all_consts();
    evaluator.check_type_positions();
    let typeck = crate::typecheck(ast, resolve, &mut evaluator);
    // Fonksiyonlar (ADR-0081): saflık, özyineleme, açılım bütçesi.
    let fn_diags = crate::functions::check_functions(ast, resolve);
    let stage_failed = count_errors(&evaluator.diagnostics) > 0
        || count_errors(&typeck.diagnostics) > 0
        || count_errors(&fn_diags) > 0;
    out.extend(evaluator.diagnostics.iter().cloned());
    out.extend(typeck.diagnostics.iter().cloned());
    out.extend(fn_diags);
    if stage_failed {
        stages.typeck = Some(typeck);
        return stages;
    }

    // ── Aşama 4: domain çıkarımı ve CDC; güven seviyeleri (ADR-0052)
    // ve reset alanı denetimi (ADR-0065) onun sonucu üzerinde ──
    let domain = crate::infer_domains(ast, resolve, &typeck);
    let trust = crate::check_trust(ast, resolve, &typeck, &domain);
    let rdc = crate::check_rdc(ast, resolve, &domain);
    out.extend(domain.diagnostics.iter().cloned());
    out.extend(trust);
    out.extend(rdc);

    // ── L1 zamanlama (ADR-0037): yalnız @strict_timing modülleri ──
    let timing = crate::analyze_timing(ast, resolve);
    out.extend(timing.diagnostics);
    // ── Handshake protokolü (ADR-0050) ──
    out.extend(crate::check_handshakes(ast, resolve));

    // ── Test blokları (ADR-0033): `X_test.volt` birimiyle `volt test` ile
    // aynı tam denetim (ADR-0101); kardeş tasarım yüklendiyse de tam.
    // Kısmi hedefte dosyada hiç modül yoksa testler görülmeyen bir
    // dosyanın modüllerini kullanıyordur — modül-varlık denetimi atlanır.
    let has_modules = ast
        .items
        .iter()
        .any(|i| matches!(ast.items_arena[*i].kind, volt_ast::ItemKind::Module(_)));
    let test_dut = tests.dut();
    let mut sources = vec![ast];
    sources.extend(test_dut);
    let assume_external = match tests {
        crate::unit_load::TestTarget::Full(_) => false,
        crate::unit_load::TestTarget::Partial(dut) => !has_modules && dut.is_none(),
    };
    out.extend(crate::check_tests_with_files(
        &sources,
        ast,
        assume_external,
        test_files,
    ));

    // ── Zamanlama kısıtları (ADR-0054): E0017 her zaman; W0022 yalnız
    // `--emit=sdc,xdc`'de ──
    let constraints = crate::collect_constraints(ast);
    out.extend(constraints.diagnostics.iter().cloned());

    stages.typeck = Some(typeck);
    stages.domain = Some(domain);
    stages.constraints = Some(constraints);
    stages.delays = Some(timing.delays);
    stages
}
