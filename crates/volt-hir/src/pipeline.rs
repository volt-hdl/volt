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

use volt_ast::SourceFile;
use volt_diagnostics::{Diagnostic, Severity};

use crate::{ConstEvaluator, ConstraintResult, DomainResult, ResolveResult, TestFileLoader};
use crate::{TypeckResult, UnenforcedLint};

/// Aşama ürünleri: `typeck`/`domain`/`constraints` yalnız önceki
/// aşamalar hatasızsa doludur.
pub struct SemanticStages {
    pub resolve: ResolveResult,
    pub typeck: Option<TypeckResult>,
    pub domain: Option<DomainResult>,
    pub constraints: Option<ConstraintResult>,
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
    crate::check_attributes(ast, lint)
}

/// Aşama 2-4 + çözümleme sonrası denetimler. `resolve` çağıran
/// tarafından üretilir (sürücü: birim modu `resolve_unit`, LSP: tek
/// dosya `resolve_file`). Tanılar `out`'a eklenir.
pub fn run_semantic_stages(
    ast: &SourceFile,
    resolve: ResolveResult,
    test_files: Option<&dyn TestFileLoader>,
    out: &mut Vec<Diagnostic>,
) -> SemanticStages {
    let mut stages = SemanticStages {
        resolve,
        typeck: None,
        domain: None,
        constraints: None,
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
    let stage_failed =
        count_errors(&evaluator.diagnostics) > 0 || count_errors(&typeck.diagnostics) > 0;
    out.extend(evaluator.diagnostics.iter().cloned());
    out.extend(typeck.diagnostics.iter().cloned());
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
    out.extend(crate::check_timing(ast, resolve));
    // ── Handshake protokolü (ADR-0050) ──
    out.extend(crate::check_handshakes(ast, resolve));

    // ── Test blokları (ADR-0033): dosyada hiç modül yoksa testler kardeş
    // dosyanın modüllerini kullanıyordur; modül-varlık denetimi atlanır.
    let has_modules = ast
        .items
        .iter()
        .any(|i| matches!(ast.items_arena[*i].kind, volt_ast::ItemKind::Module(_)));
    out.extend(crate::check_tests_with_files(
        &[ast],
        ast,
        !has_modules,
        test_files,
    ));

    // ── Zamanlama kısıtları (ADR-0054): E0017 her zaman; W0022 yalnız
    // `--emit=sdc,xdc`'de ──
    let constraints = crate::collect_constraints(ast);
    out.extend(constraints.diagnostics.iter().cloned());

    stages.typeck = Some(typeck);
    stages.domain = Some(domain);
    stages.constraints = Some(constraints);
    stages
}
