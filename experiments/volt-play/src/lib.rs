//! Volt oyun alanı çekirdeği (fizibilite denemesi).
//!
//! Bellekteki tek bir kaynak metinden, dosya sistemine dokunmadan, tanılar
//! ve SystemVerilog üretir. Aşamalar `volt-driver`'ın `compile_all`
//! fonksiyonunun tek dosyalık kopyasıdır (parse → ön denetimler → import →
//! extern kaynak → anlamsal aşamalar → emit); sürücüden farkı yalnız G/Ç:
//! birim yükleyicisi, Volt.toml, `@source` ve `read_hex` dosyaları yok.

#![forbid(unsafe_code)]

use serde_json::{json, Value};
use volt_diagnostics::{lstr, Diagnostic, Lang, Severity};
use volt_hir::unit_load::TestTarget;
use volt_hir::{SourceLocator, TestFileError, TestFileLoader, UnenforcedLint, UnitInfo};
use volt_span::{FileId, SourceMap};
use volt_sv_emit::{ConstArrayStyle, SvaMode};

#[cfg(target_arch = "wasm32")]
mod web;

/// Ana dosyanın görünen adı (tanılarda ve SV başlığında).
pub const SOURCE_NAME: &str = "main.volt";

/// Sürücünün varsayılanı (`--max-diagnostics`).
const MAX_DIAGNOSTICS: usize = 1000;

/// Derleme sonucu.
pub struct Output {
    pub map: SourceMap,
    pub diagnostics: Vec<Diagnostic>,
    /// Hata yoksa üretilen SV (tüm modüller tek metinde).
    pub sv: Option<String>,
}

impl Output {
    pub fn errors(&self) -> usize {
        self.count(Severity::Error)
    }

    pub fn warnings(&self) -> usize {
        self.count(Severity::Warning)
    }

    fn count(&self, severity: Severity) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == severity)
            .count()
    }

    /// cli-contract.md §5 zarfı (`volt build --format json` ile aynı alanlar;
    /// `duration_ms` ve `artifacts` yok) + `sv` ve her tanının insan metni.
    pub fn to_json(&self) -> Value {
        let mut ordered: Vec<&Diagnostic> = self.diagnostics.iter().collect();
        ordered.sort_by_key(|d| d.severity != Severity::Error);
        json!({
            "version": "1",
            "command": "play",
            "success": self.errors() == 0,
            "diagnostics": ordered
                .iter()
                .map(|d| volt_diagnostics::to_json_value(d, &self.map))
                .collect::<Vec<_>>(),
            "rendered": ordered
                .iter()
                .map(|d| volt_diagnostics::render_human(d, &self.map))
                .collect::<Vec<_>>(),
            "summary": { "errors": self.errors(), "warnings": self.warnings() },
            "sv": self.sv,
        })
    }
}

/// Oyun alanında dosya yok: `@source` ve `read_hex` yolları bulunamaz.
struct NoFiles;

impl SourceLocator for NoFiles {
    fn locate(&self, _file: FileId, _rel_path: &str) -> Result<std::path::PathBuf, TestFileError> {
        Err(TestFileError::NotFound(lstr!(
            en: "the playground has no files";
            tr: "oyun alanında dosya yok"
        )))
    }
}

impl TestFileLoader for NoFiles {
    fn load(&self, _rel_path: &str) -> Result<String, TestFileError> {
        Err(TestFileError::NotFound(lstr!(
            en: "the playground has no files";
            tr: "oyun alanında dosya yok"
        )))
    }
}

fn count_errors(diags: &[Diagnostic]) -> usize {
    diags
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count()
}

/// `source`'u derler. `lang`: tanı dili (süreç geneli ayar, sürücüdeki
/// `--lang` gibi).
pub fn compile(source: &str, lang: Lang) -> Output {
    volt_diagnostics::set_lang(lang);
    let mut out = volt_syntax::with_compiler_stack(|| compile_on_stack(source));
    volt_diagnostics::cap_diagnostics(
        &mut out.diagnostics,
        MAX_DIAGNOSTICS,
        &lstr!(en: "fix the reported diagnostics first"; tr: "önce raporlanan tanıları düzeltin"),
    );
    out
}

fn compile_on_stack(source: &str) -> Output {
    // ── Aşama 0+1: tek dosyalık birim (sürücüde `unit_load::load_unit`) ──
    let mut map = SourceMap::new();
    let fid = map.add_file(SOURCE_NAME, source.to_string());
    let parsed = volt_syntax::parse_unit(&[(fid, source)]);
    volt_hir::unit_load::register_generated(&mut map, &parsed.generated);
    let mut info = UnitInfo::default();
    let declared: Option<Vec<String>> = parsed
        .ast
        .packages
        .first()
        .map(|p| p.path.segments.iter().map(|s| s.text.clone()).collect());
    info.add(fid, declared.unwrap_or_else(|| vec!["main".to_string()]));

    let mut diagnostics = parsed.diagnostics.clone();
    let fail = |map, diagnostics, ast: &volt_ast::SourceFile| Output {
        map,
        diagnostics: volt_hir::annotate_generate(ast, diagnostics),
        sv: None,
    };
    if count_errors(&diagnostics) > 0 {
        return fail(map, diagnostics, &parsed.ast);
    }

    // ── Aşama 1b: uygulanmayan nitelikler (Volt.toml yok: varsayılan warn) ──
    diagnostics.extend(volt_hir::pre_resolve_checks(
        &parsed.ast,
        UnenforcedLint::Warn,
    ));

    // ── Aşama 2a: importlar. Yükleyici yok: `use` edilen paket dosyası
    // aranmaz, E1011 de üretilmez (sürücüde yükleyicinin işi). ──
    let imports = volt_hir::check_imports(&parsed.ast, &info);
    diagnostics.extend(imports.diagnostics);
    // ── Aşama 2b: `@source` — her yol E1012 ──
    let (_extern, extern_diags) = volt_hir::resolve_extern_sources(&parsed.ast, &NoFiles);
    diagnostics.extend(extern_diags);
    if count_errors(&diagnostics) > 0 {
        return fail(map, diagnostics, &parsed.ast);
    }

    // ── Aşama 2-4: paylaşılan kapılı boru hattı (ADR-0070) ──
    let resolve = volt_hir::resolve_unit(&parsed.ast, &imports.scopes);
    let tests = TestTarget::Partial(None);
    volt_hir::run_semantic_stages(
        &parsed.ast,
        resolve,
        Some(&NoFiles),
        &tests,
        &mut diagnostics,
    );
    if count_errors(&diagnostics) > 0 {
        return fail(map, diagnostics, &parsed.ast);
    }

    // ── Aşama 5: emit ──
    let names = vec![(fid, SOURCE_NAME.to_string())];
    let sources = volt_sv_emit::unit_source_texts(&names, &map);
    let emitted = volt_sv_emit::emit_unit(
        &parsed.ast,
        SOURCE_NAME,
        &sources,
        SvaMode::None,
        ConstArrayStyle::default(),
    );
    diagnostics.extend(emitted.diagnostics);
    let diagnostics = volt_hir::annotate_generate(&parsed.ast, diagnostics);
    // Modülsüz dosya (kütüphane): `volt build` SV yazmaz (sürücüde bir not).
    let sv = (count_errors(&diagnostics) == 0 && !emitted.modules.is_empty()).then_some(emitted.sv);
    Output {
        map,
        diagnostics,
        sv,
    }
}

/// Dil adı → `Lang` ("tr" dışı her şey İngilizce).
pub fn parse_lang(s: &str) -> Lang {
    Lang::parse(s).unwrap_or_default()
}
