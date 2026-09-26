//! Belge analizi (ADR-0070). İki ürün:
//!
//! * Tanılar — `volt check` ile AYNI yol: birim yükleyicisi
//!   (`volt_hir::unit_load`, ana dosya metni editör tamponundan) →
//!   ortak kapılı boru hattı (`volt_hir::run_semantic_stages`) → çıktısız
//!   emit doğrulaması (`volt_sv_emit::validate_unit`) → toplayıcı
//!   (`annotate_generate`: katlama + yineleme notu) → editör üst sınırı.
//!   Yalnız bu belgeye düşen tanılar yayımlanır.
//! * Editör verisi (hover, tanım, tamamlama, semboller) — tek dosya
//!   ayrıştırması + aynı boru hattı; parser hata kurtarma yaptığı için
//!   AST HER girdide üretilir, tamamlama yarım kodda da çalışır.

use std::path::Path;
use volt_ast::{ItemKind, ModuleDecl, SourceFile, StmtKind, TypeRefKind};
use volt_diagnostics::{Diagnostic, Severity};
use volt_hir::{DefId, DefKind, DomainResult, ResolveResult, TypeckResult};

use volt_span::{FileId, SourceMap, Span};

/// Bir belgenin tam analiz durumu. `resolve`/`typeck`/`domain` yalnız
/// önceki aşamalar hatasızsa doldurulur (driver kapılaması).
pub struct Analysis {
    pub map: SourceMap,
    pub file_id: FileId,
    pub ast: SourceFile,
    pub resolve: Option<ResolveResult>,
    pub typeck: Option<TypeckResult>,
    pub domain: Option<DomainResult>,
    pub diagnostics: Vec<Diagnostic>,
    /// Referans dizini: kullanım/bildirim span'i → tanım. Hover ve
    /// go-to-definition span kapsama sorgusuyla arar.
    refs: Vec<(Span, DefId)>,
}

fn count_errors(diags: &[Diagnostic]) -> usize {
    diags
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count()
}

/// Kaynağı driver ile aynı aşama sırasında analiz eder.
pub fn analyze(path: &str, text: &str) -> Analysis {
    let mut map = SourceMap::new();
    let file_id = map.add_file(path.to_string(), text.to_string());
    let parsed = volt_syntax::parse(file_id, text);
    // ADR-0044: üretilen @mmio kaynakları da haritaya girer ki sentetik
    // dosyaya düşen bir tanı konum çevrilirken panik etmesin.
    for g in &parsed.generated {
        if map.len() == g.file.0 as usize {
            map.add_file(g.name.clone(), g.text.clone());
        }
    }
    let mut diagnostics = parsed.diagnostics;

    let mut analysis = Analysis {
        map,
        file_id,
        ast: parsed.ast,
        resolve: None,
        typeck: None,
        domain: None,
        diagnostics: Vec::new(),
        refs: Vec::new(),
    };

    if count_errors(&diagnostics) == 0 {
        let lint = volt_hir::UnenforcedLint::discover(Path::new(path).parent());
        diagnostics.extend(volt_hir::pre_resolve_checks(&analysis.ast, lint));
        let resolve = volt_hir::resolve_file(&analysis.ast);
        let stages = volt_hir::run_semantic_stages(&analysis.ast, resolve, None, &mut diagnostics);
        analysis.resolve = Some(stages.resolve);
        analysis.typeck = stages.typeck;
        analysis.domain = stages.domain;
    }

    // Tanılar CLI yolundan; birim yüklenemezse (bağımlılık okunamadı)
    // tek dosya boru hattının tanıları kalır.
    let diagnostics = unit_diagnostics(Path::new(path), text, file_id)
        .unwrap_or_else(|| volt_hir::annotate_generate(&analysis.ast, diagnostics));
    analysis.diagnostics = cap_for_editor(diagnostics);
    analysis.build_refs();
    analysis
}

/// Editör üst sınırı (ADR-0070 §3): katlamadan sonra bile sınırı aşan
/// FARKLI tanılar; W0023 çözümü `volt check`'i gösterir.
fn cap_for_editor(mut diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    volt_diagnostics::cap_diagnostics(
        &mut diagnostics,
        volt_diagnostics::LSP_MAX_DIAGNOSTICS,
        &volt_diagnostics::lstr!(
            en: "fix the reported diagnostics first; `volt check` lists them all (--max-diagnostics=0)";
            tr: "önce raporlanan tanıları düzeltin; hepsi için `volt check` (--max-diagnostics=0)"
        ),
    );
    diagnostics
}

/// `volt check` ile aynı aşamalar (sürücü `compile_all`'ın tanı yolu):
/// birim yükleme → ön denetim → import → ortak boru hattı → çıktısız
/// emit → toplayıcı. Dönen tanılar yalnız ana dosyaya düşenlerdir ve
/// span'leri tek dosya haritasına (`file_id`) taşınmıştır.
fn unit_diagnostics(path: &Path, text: &str, file_id: FileId) -> Option<Vec<Diagnostic>> {
    let unit = volt_hir::unit_load::load_unit_with_text(path, Some(text.to_string())).ok()?;
    let names = unit.source_names();
    let main = unit.files.last().map(|(fid, _)| *fid)?;
    let lint = unit
        .manifest
        .as_ref()
        .map_or_else(Default::default, |m| m.lint_unenforced);
    let ast = &unit.parsed.ast;
    let mut diags = unit.parsed.diagnostics.clone();
    if count_errors(&diags) == 0 {
        diags.extend(volt_hir::pre_resolve_checks(ast, lint));
        diags.extend(unit.diagnostics.iter().cloned());
        let imports = volt_hir::check_imports(ast, &unit.info);
        diags.extend(imports.diagnostics);
        // `@source` dosyaları (ADR-0076) — `volt check` ile aynı E1012.
        let locator = volt_hir::FsSourceLocator::new(&unit.files);
        diags.extend(volt_hir::resolve_extern_sources(ast, &locator).1);
        if count_errors(&diags) == 0 {
            let resolve = volt_hir::resolve_unit(ast, &imports.scopes);
            // Test veri dosyaları (ADR-0058) editörde okunmaz: içerik
            // denetimleri (E8508/E8510) yalnız `volt check`/`build`'de.
            volt_hir::run_semantic_stages(ast, resolve, None, &mut diags);
            if count_errors(&diags) == 0 {
                let sources = volt_sv_emit::unit_source_texts(&names, &unit.map);
                let main_name = names.last().map_or("", |(_, n)| n.as_str());
                diags.extend(volt_sv_emit::validate_unit(ast, main_name, &sources));
            }
        }
    }
    let diags = volt_hir::annotate_generate(ast, diags);
    Some(
        diags
            .into_iter()
            .filter_map(|d| to_main_file(d, main, file_id, &unit.map))
            .collect(),
    )
}

/// Birim tanısını tek dosya koordinatlarına taşır: birincil span'i ana
/// dosyada değilse `None` (o dosya açılınca kendi tanısı görünür); başka
/// dosyadaki ikincil span'ler "dosya:satır:sütun" notuna dönüşür.
fn to_main_file(
    mut d: Diagnostic,
    main: FileId,
    file_id: FileId,
    map: &SourceMap,
) -> Option<Diagnostic> {
    if d.primary_span()?.span.file != main {
        return None;
    }
    let mut elsewhere = Vec::new();
    d.spans.retain_mut(|s| {
        if s.span.file == main {
            s.span.file = file_id;
            true
        } else {
            let (line, col) = map.line_col(s.span);
            let at = format!("{}:{}:{}", map.path(s.span.file).display(), line, col);
            elsewhere.push(if s.label.is_empty() {
                at
            } else {
                format!("{at}: {}", s.label)
            });
            false
        }
    });
    for text in elsewhere {
        d = d.with_note(volt_diagnostics::NoteKind::Note, text);
    }
    Some(d)
}

impl Analysis {
    pub fn source(&self) -> &str {
        self.map.source(self.file_id)
    }

    /// Verilen bayt offsetini kapsayan en dar referansın tanımı.
    pub fn def_at(&self, offset: u32) -> Option<DefId> {
        self.refs
            .iter()
            .filter(|(s, _)| s.start <= offset && offset < s.end)
            .min_by_key(|(s, _)| s.end - s.start)
            .map(|(_, d)| *d)
    }

    /// Offseti kapsayan modül bildirimi (tamamlama bağlamı için).
    /// Hata kurtarma kapatılmamış modülün span'ini erken bitirir —
    /// gövdesi açık kalan modül dosya sonuna kadar sayılır ki yarım
    /// kodda da (`on `, satır başı) doğru bağlam bulunsun.
    pub fn module_at(&self, offset: u32) -> Option<&ModuleDecl> {
        let src = self.source();
        let mut unclosed: Option<&ModuleDecl> = None;
        for &idx in &self.ast.items {
            let item = &self.ast.items_arena[idx];
            let ItemKind::Module(m) = &item.kind else {
                continue;
            };
            if item.span.start > offset {
                continue;
            }
            if offset <= item.span.end {
                return Some(m);
            }
            let text = &src[item.span.start as usize..(item.span.end as usize).min(src.len())];
            if text.matches('{').count() > text.matches('}').count() {
                unclosed = Some(m);
            }
        }
        unclosed
    }

    /// Dosyadaki `domain` bildirimlerinin adları ('@' tamamlaması).
    pub fn domain_names(&self) -> Vec<&str> {
        self.ast
            .items
            .iter()
            .filter_map(|&idx| match &self.ast.items_arena[idx].kind {
                ItemKind::Domain(d) => Some(d.name.text.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Modülün clock tipli port adları (`on ` tamamlaması).
    pub fn clock_ports<'a>(&'a self, module: &'a ModuleDecl) -> Vec<&'a str> {
        module
            .ports
            .iter()
            .filter(|p| matches!(self.ast.types[p.ty].kind, TypeRefKind::Clock))
            .map(|p| p.name.text.as_str())
            .collect()
    }

    /// Port bildiriminin doc yorumu (varsa) — DefData span'i port adı
    /// span'ine eşittir, eşleşen portu AST'de arar.
    pub fn port_doc(&self, def_span: Span) -> Option<&str> {
        self.ast.items.iter().find_map(|&idx| {
            let ItemKind::Module(m) = &self.ast.items_arena[idx].kind else {
                return None;
            };
            m.ports
                .iter()
                .find(|p| p.name.span == def_span)
                .and_then(|p| p.doc.as_deref())
        })
    }

    /// fn imzası (ADR-0081 Karar 13): `fn imm(instr: u32, f: Fmt) -> u32`.
    /// Tipler kaynakta yazıldığı gibi (takma ad, `bits<W>` korunur).
    pub fn fn_signature(&self, def: DefId) -> Option<String> {
        let res = self.resolve.as_ref()?;
        let idx = res.item_of_def.get(&def)?;
        let ItemKind::Fn(f) = &self.ast.items_arena[*idx].kind else {
            return None;
        };
        let text = |span: Span| -> String {
            let src = self.map.source(span.file);
            src.get(span.start as usize..span.end as usize)
                .unwrap_or_default()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        };
        let params: Vec<String> = f
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name.text, text(self.ast.types[p.ty].span)))
            .collect();
        let generics = match (f.generics.first(), f.generics.last()) {
            (Some(first), Some(last)) => format!(
                "<{}>",
                text(Span::new(first.span.file, first.span.start, last.span.end))
            ),
            _ => String::new(),
        };
        let ret = f
            .return_ty
            .map(|t| format!(" -> {}", text(self.ast.types[t].span)))
            .unwrap_or_default();
        Some(format!(
            "fn {}{generics}({}){ret}",
            f.name.text,
            params.join(", ")
        ))
    }

    /// Öğe (modül/struct/enum/...) doc yorumu.
    pub fn item_doc(&self, def: DefId) -> Option<&str> {
        let res = self.resolve.as_ref()?;
        let idx = res.item_of_def.get(&def)?;
        self.ast.items_arena[*idx].doc.as_deref()
    }

    /// resolve haritaları + AST'deki isim konumlarından referans dizini.
    fn build_refs(&mut self) {
        let Some(res) = &self.resolve else { return };
        let mut refs: Vec<(Span, DefId)> = Vec::new();
        refs.extend(res.decl_spans.iter().map(|(s, d)| (*s, *d)));
        refs.extend(res.use_spans.iter().map(|(s, d)| (*s, *d)));
        refs.extend(
            res.pattern_resolutions
                .iter()
                .map(|(idx, d)| (self.ast.patterns[*idx].span, *d)),
        );
        for (idx, def) in &res.resolutions {
            refs.push((self.ast.exprs[*idx].span, *def));
        }
        for (idx, def) in &res.type_resolutions {
            refs.push((self.ast.types[*idx].span, *def));
        }

        // resolve haritalarında yer almayan isim konumları: @Domain
        // anotasyonları, `reg(clk)` domain'i ve `on clk` tetikleyicisi.
        let find_domain = |name: &str| {
            res.defs
                .iter()
                .position(|d| matches!(d.kind, DefKind::Domain) && d.name == name)
                .map(|i| DefId(i as u32))
        };
        let find_named = |name: &str| {
            res.defs
                .iter()
                .position(|d| d.name == name && !matches!(d.kind, DefKind::Error))
                .map(|i| DefId(i as u32))
        };
        for &item_idx in &self.ast.items {
            let ItemKind::Module(m) = &self.ast.items_arena[item_idx].kind else {
                continue;
            };
            for port in &m.ports {
                if let Some(dn) = &port.domain {
                    if let Some(def) = find_domain(&dn.text) {
                        refs.push((dn.span, def));
                    }
                }
            }
            for &stmt_idx in &m.body {
                match &self.ast.stmts[stmt_idx].kind {
                    StmtKind::Reg(r) => {
                        if let Some(dn) = &r.domain {
                            if let Some(def) = find_named(&dn.text) {
                                refs.push((dn.span, def));
                            }
                        }
                    }
                    StmtKind::On(on) => {
                        let name = match &on.trigger {
                            volt_ast::OnTrigger::Clock(n) | volt_ast::OnTrigger::Reset(n) => {
                                Some(n)
                            }
                            volt_ast::OnTrigger::Error => None,
                        };
                        if let Some(n) = name {
                            if let Some(def) = find_named(&n.text) {
                                refs.push((n.span, def));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        refs.sort_by_key(|(s, _)| (s.start, s.end));
        self.refs = refs;
    }
}
