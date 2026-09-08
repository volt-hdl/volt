//! Tek dosya analizi — aşama sırası ve kapılama volt-driver'daki
//! `compile()` ile birebir aynıdır: parse → resolve → const+typeck →
//! domain. Hatalı aşamadan sonrakiler koşmaz (kaskad tanı önlemi),
//! ama parser hata kurtarma yaptığı için AST HER girdide üretilir —
//! tamamlama ve sembol ağacı yarım kodda da çalışır.

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
        let resolve = volt_hir::resolve_file(&analysis.ast);
        let resolve_failed = count_errors(&resolve.diagnostics) > 0;
        diagnostics.extend(resolve.diagnostics.iter().cloned());
        if !resolve_failed {
            let mut evaluator = volt_hir::ConstEvaluator::new(&analysis.ast, &resolve);
            evaluator.eval_all_consts();
            evaluator.check_type_positions();
            let typeck = volt_hir::typecheck(&analysis.ast, &resolve, &mut evaluator);
            let stage_failed =
                count_errors(&evaluator.diagnostics) > 0 || count_errors(&typeck.diagnostics) > 0;
            diagnostics.extend(evaluator.diagnostics.iter().cloned());
            diagnostics.extend(typeck.diagnostics.iter().cloned());
            if !stage_failed {
                let domain = volt_hir::infer_domains(&analysis.ast, &resolve, &typeck);
                diagnostics.extend(domain.diagnostics.iter().cloned());
                analysis.domain = Some(domain);
            }
            analysis.typeck = Some(typeck);
        }
        analysis.resolve = Some(resolve);
    }

    analysis.diagnostics = diagnostics;
    analysis.build_refs();
    analysis
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
