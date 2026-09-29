//! Inlay ipuçları (ADR-0091): tipsiz `let`/`reg`'in çıkarılan tipi, çok
//! saatli modülde saat alanı, `@strict_timing` modülünde gecikme; fn
//! gövdesindeki tipsiz `let`'in tipi.
//!
//! Bilgi `volt check`'in kendi birim analizinden okunur
//! (`analysis::analyze_hints`: tip denetimi `def_types`, domain çıkarımı
//! `signal_domains`, zamanlama geçidi `analyze_timing`); burada ikinci
//! bir çıkarım YOKTUR (ADR-0070). Çok dosyalı birimde AST birimin
//! tamamıdır; ipucu yalnız açık dosyanın kendi bildirimlerine konur.
//!
//! Güven kuralı: yanlış ipucu, ipucusuzluktan kötüdür.
//! * Boru hattında hata bulunan modülde hiç ipucu yok; hata modül
//!   dışındaysa (struct, enum, fn, ... ya da `use` ile gelen başka bir
//!   dosya) dosyada hiç ipucu yok.
//! * Tipi hata ya da boyutsuz literal içeren tanıma tip ipucu yok.
//! * Açılmış kopyaları (for, generic) farklı sonuç veren konumda ipucu yok.
//! * Kaynak metni adla uyuşmayan (desugar üretimi) bildirime ipucu yok.

use std::collections::{BTreeMap, HashSet};

use volt_ast::{
    Block, BlockStmt, ElseBranch, Idx, IfStmt, ItemKind, MatchArmBody, ModuleDecl, Name, StmtKind,
    TypeRef,
};
use volt_hir::{DefId, DomainId, Ty, TypeArena, TypeId};
use volt_span::Span;

use crate::analysis::Analysis;

/// İpucu türü — her biri ayrı ayarla kapatılabilir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HintKind {
    Type,
    Domain,
    Latency,
}

/// Açık ipucu türleri (varsayılan: hepsi açık).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HintConfig {
    pub types: bool,
    pub domains: bool,
    pub latency: bool,
}

impl Default for HintConfig {
    fn default() -> Self {
        Self {
            types: true,
            domains: true,
            latency: true,
        }
    }
}

impl HintConfig {
    fn allows(&self, kind: HintKind) -> bool {
        match kind {
            HintKind::Type => self.types,
            HintKind::Domain => self.domains,
            HintKind::Latency => self.latency,
        }
    }
}

/// Bayt offsetinde gösterilecek tek ipucu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hint {
    pub offset: u32,
    pub kind: HintKind,
    pub label: String,
}

/// İpucu adayı olan bildirim: ad, yazılmış tip, yazılmış alan.
struct Decl<'a> {
    name: &'a Name,
    ty: Option<Idx<TypeRef>>,
    has_domain: bool,
    /// Tip ipucu yalnız tipsiz `let`/`reg` için.
    typeable: bool,
}

/// `[start, end)` bayt aralığıyla kesişen bildirimlerin ipuçları.
pub fn inlay_hints(analysis: &Analysis, start: u32, end: u32, config: HintConfig) -> Vec<Hint> {
    let Some(res) = analysis.resolve.as_ref() else {
        return Vec::new();
    };
    // Modül dışındaki hata (struct/enum/fn/const/domain ya da üretilmiş
    // dosya): her modül bunlara bağımlı olabilir — dosyanın tamamında sus.
    if analysis
        .editor_errors
        .iter()
        .any(|e| !in_some_module(analysis, *e))
    {
        return Vec::new();
    }
    // (offset, tür) → etiketler; açılmış kopyalar aynı anahtara düşer.
    let mut found: BTreeMap<(u32, HintKind), Vec<String>> = BTreeMap::new();
    for &idx in &analysis.ast.items {
        let item = &analysis.ast.items_arena[idx];
        // Birimin diğer dosyalarındaki öğeler bu belgeye ipucu vermez.
        if item.span.file != analysis.file_id
            || !overlaps(item.span, start, end)
            || has_error_in(analysis, item.span)
        {
            continue;
        }
        // fn saf kombinasyoneldir (ADR-0081): alan ve gecikme ipucu yok.
        let (decls, strict, multi_clock) = match &item.kind {
            ItemKind::Module(m) => {
                let decls = module_decls(analysis, m);
                let multi_clock = distinct_clock_domains(analysis, &decls, res) >= 2;
                let strict = item.attrs.iter().any(|a| a.name.text == "strict_timing");
                (decls, strict, multi_clock)
            }
            ItemKind::Fn(f) => (fn_decls(analysis, f), false, false),
            _ => continue,
        };
        for decl in decls.iter().filter(|d| overlaps(d.name.span, start, end)) {
            let Some(&def) = res.decl_spans.get(&decl.name.span) else {
                continue;
            };
            if !written_as(analysis, decl.name) {
                continue;
            }
            let at = anchor(analysis, decl);
            let mut push = |kind: HintKind, label: String| {
                if config.allows(kind) {
                    found.entry((at, kind)).or_default().push(label);
                }
            };
            if decl.typeable && decl.ty.is_none() {
                if let Some(label) = type_label(analysis, def) {
                    push(HintKind::Type, label);
                }
            }
            if multi_clock && !decl.has_domain {
                if let Some(label) = domain_label(analysis, def) {
                    push(HintKind::Domain, label);
                }
            }
            if strict && !is_delay_annotated(analysis, decl) {
                if let Some(n) = analysis.delays.as_ref().and_then(|d| d.get(&def)) {
                    push(HintKind::Latency, format!("+{n}"));
                }
            }
        }
    }
    found
        .into_iter()
        .filter_map(|((offset, kind), labels)| {
            let first = labels.first()?;
            labels.iter().all(|l| l == first).then(|| Hint {
                offset,
                kind,
                label: first.clone(),
            })
        })
        .collect()
}

fn overlaps(span: Span, start: u32, end: u32) -> bool {
    span.start <= end && start <= span.end
}

fn in_some_module(analysis: &Analysis, error: Span) -> bool {
    error.file == analysis.file_id
        && analysis.ast.items.iter().any(|&i| {
            let item = &analysis.ast.items_arena[i];
            matches!(item.kind, ItemKind::Module(_))
                && item.span.start <= error.start
                && error.start <= item.span.end
        })
}

fn has_error_in(analysis: &Analysis, span: Span) -> bool {
    analysis
        .editor_errors
        .iter()
        .any(|e| span.start <= e.start && e.start <= span.end)
}

/// Bildirim kaynakta bu adla yazılmış mı (desugar üretimi değil mi).
fn written_as(analysis: &Analysis, name: &Name) -> bool {
    name.span.file == analysis.file_id
        && analysis
            .source()
            .get(name.span.start as usize..name.span.end as usize)
            == Some(name.text.as_str())
}

/// İpucu konumu: yazılmış tipin sonu, yoksa adın sonu — `let s : u9 @A`
/// sırası kaynaktaki ADR-0088 yazımıyla aynıdır.
fn anchor(analysis: &Analysis, decl: &Decl) -> u32 {
    decl.ty
        .map(|t| analysis.ast.types[t].span)
        .filter(|s| s.file == analysis.file_id && s.end >= decl.name.span.end)
        .map_or(decl.name.span.end, |s| s.end)
}

fn is_delay_annotated(analysis: &Analysis, decl: &Decl) -> bool {
    decl.ty
        .is_some_and(|t| analysis.ast.timing.delayed_types.contains_key(&t))
}

fn type_label(analysis: &Analysis, def: DefId) -> Option<String> {
    let res = analysis.resolve.as_ref()?;
    let tc = analysis.typeck.as_ref()?;
    let id = *tc.def_types.get(&def)?;
    is_reliable(&tc.types, id, 0).then(|| format!(": {}", tc.types.display_named(id, &res.defs)))
}

/// Tip, hata kurtarma ya da boyutsuz literal içermiyor mu.
fn is_reliable(types: &TypeArena, id: TypeId, depth: u8) -> bool {
    if depth > 16 {
        return false;
    }
    match types.ty(id) {
        Ty::Error | Ty::IntLit => false,
        Ty::Array { elem, .. } => is_reliable(types, *elem, depth + 1),
        Ty::Tuple(items) => items.iter().all(|t| is_reliable(types, *t, depth + 1)),
        Ty::Delayed { inner, .. } => is_reliable(types, *inner, depth + 1),
        Ty::Builtin { data, .. } => is_reliable(types, *data, depth + 1),
        _ => true,
    }
}

fn domain_label(analysis: &Analysis, def: DefId) -> Option<String> {
    let domain = analysis.domain.as_ref()?;
    match domain.signal_domains.get(&def)? {
        DomainId::Explicit(idx) => Some(format!("@{}", domain.domains.get(*idx as usize)?.name)),
        DomainId::Timeless | DomainId::Unresolved(_) | DomainId::Error => None,
    }
}

/// Modüldeki sinyallerin taşıdığı farklı açık saat alanı sayısı.
fn distinct_clock_domains(
    analysis: &Analysis,
    decls: &[Decl],
    res: &volt_hir::ResolveResult,
) -> usize {
    let Some(domain) = analysis.domain.as_ref() else {
        return 0;
    };
    decls
        .iter()
        .filter_map(|d| res.decl_spans.get(&d.name.span))
        .filter_map(|def| match domain.signal_domains.get(def) {
            Some(DomainId::Explicit(idx)) => Some(*idx),
            _ => None,
        })
        .collect::<HashSet<_>>()
        .len()
}

/// Portlar + gövdedeki `reg`/`let`/`wire` bildirimleri (bloklar dahil).
fn module_decls<'a>(analysis: &'a Analysis, m: &'a ModuleDecl) -> Vec<Decl<'a>> {
    let mut out: Vec<Decl> = m
        .ports
        .iter()
        .filter(|p| p.bundle.is_none())
        .map(|p| Decl {
            name: &p.name,
            ty: Some(p.ty),
            has_domain: p.domain.is_some(),
            typeable: false,
        })
        .collect();
    for &stmt in &m.body {
        match &analysis.ast.stmts[stmt].kind {
            StmtKind::Reg(r) => out.push(Decl {
                name: &r.name,
                ty: r.ty,
                has_domain: r.domain.is_some(),
                typeable: true,
            }),
            StmtKind::Let(l) => out.push(let_decl(l)),
            StmtKind::Wire(w) => out.push(Decl {
                name: &w.name,
                ty: Some(w.ty),
                has_domain: w.domain.is_some(),
                typeable: false,
            }),
            StmtKind::On(on) => block_decls(analysis, on.body, &mut out),
            StmtKind::Comb(b) => block_decls(analysis, *b, &mut out),
            StmtKind::For(f) => block_decls(analysis, f.body, &mut out),
            StmtKind::Assign(_) | StmtKind::Instance(_) | StmtKind::Expr(_) | StmtKind::Error => {}
        }
    }
    out
}

/// fn gövdesinin `let`'leri: parser gövdede yalnız `let`/son ifade
/// bırakır (ADR-0081); gövde tanımda bir kez tip denetlenir, `let`
/// tipleri `def_types`'a modül `let`'iyle aynı yoldan yazılır.
fn fn_decls<'a>(analysis: &'a Analysis, f: &'a volt_ast::FnDecl) -> Vec<Decl<'a>> {
    analysis.ast.blocks[f.body]
        .stmts
        .iter()
        .filter_map(|stmt| match stmt {
            BlockStmt::Let(l) => Some(let_decl(l)),
            _ => None,
        })
        .collect()
}

fn let_decl(l: &volt_ast::LetDecl) -> Decl<'_> {
    Decl {
        name: &l.name,
        ty: l.ty,
        has_domain: l.domain.is_some(),
        typeable: true,
    }
}

fn block_decls<'a>(analysis: &'a Analysis, block: Idx<Block>, out: &mut Vec<Decl<'a>>) {
    for stmt in &analysis.ast.blocks[block].stmts {
        match stmt {
            BlockStmt::Let(l) => out.push(let_decl(l)),
            BlockStmt::If(i) => if_decls(analysis, i, out),
            BlockStmt::Match(m) => {
                for arm in &m.arms {
                    if let MatchArmBody::Block(b) = &arm.body {
                        block_decls(analysis, *b, out);
                    }
                }
            }
            BlockStmt::For(f) => block_decls(analysis, f.body, out),
            BlockStmt::NonBlockAssign { .. } | BlockStmt::BlockAssign { .. } | BlockStmt::Error => {
            }
        }
    }
}

fn if_decls<'a>(analysis: &'a Analysis, i: &'a IfStmt, out: &mut Vec<Decl<'a>>) {
    block_decls(analysis, i.then_block, out);
    match &i.else_branch {
        Some(ElseBranch::Block(b)) => block_decls(analysis, *b, out),
        Some(ElseBranch::If(inner)) => if_decls(analysis, inner, out),
        None => {}
    }
}
