//! Modül olguları (ADR-0065 §1): saat portları ve etkin reset'leri,
//! ham reset portları, birim kökü. Denetimler bu tabloyu okur; AST'ye ve
//! çözümleme sonucuna yalnız burada bakılır.

use std::collections::{HashMap, HashSet};

use volt_ast::{
    builtin::BuiltinPrim, ClockEdge, DomainKey, DomainValue, InstanceDecl, ItemKind, ModuleDecl,
    Port, ResetPolarity, ResetSpec, ResetSync, SourceFile, StmtKind, TypeRefKind,
};
use volt_span::Span;

use super::super::{DomainId, DomainResult, DomainSource};
use super::is_raw_reset;
use crate::resolve::{DefId, ResolveResult};

/// Alanında `reset` yazılmamış saatin reset'i — sv-emit'in ürettiği
/// (sv-mapping.md §7): `sync active_high`, port `rst`.
pub(super) const DEFAULT_RESET: ResetSpec = ResetSpec {
    sync: ResetSync::Sync,
    polarity: ResetPolarity::ActiveHigh,
};

pub(super) struct UnitFacts<'a> {
    pub(super) ast: &'a SourceFile,
    pub(super) modules: Vec<ModuleFacts<'a>>,
    /// Modül tanımı → `modules` indeksi (örnekleme hedefi araması).
    pub(super) by_def: HashMap<DefId, usize>,
}

pub(super) struct ModuleFacts<'a> {
    pub(super) decl: &'a ModuleDecl,
    pub(super) clocks: Vec<ClockFact<'a>>,
    pub(super) raws: Vec<RawFact<'a>>,
    /// Birimde hiçbir modül bu modülü örneklemiyor (W3009 kapsamı).
    pub(super) is_root: bool,
}

pub(super) struct ClockFact<'a> {
    pub(super) port: &'a Port,
    pub(super) def: DefId,
    /// Alan bir `domain` bildirimi ise onun tanımı (ham port `@D` eşlemesi).
    pub(super) decl: Option<DefId>,
    /// Etkin reset; `None` = `reset = none` (reset'ten etkilenmez).
    pub(super) reset: Option<ResetSpec>,
    /// `reset = ...` alanı; yazılmamışsa alan adı (örtük alanda port adı).
    pub(super) reset_span: Span,
    /// Saat modülde reset'li flop sürebilecek bir yerde kullanılıyor
    /// (flop'u olmayan saat reset örneklemez — R5 yalnız kullanılan
    /// saatleri sayar). Reset'siz yerleşik saat bağlaması
    /// (`AsyncDualPortRam.wr_clk`) ve extern örneği bağlaması kullanım
    /// sayılmaz.
    pub(super) used: bool,
}

pub(super) struct RawFact<'a> {
    pub(super) port: &'a Port,
    pub(super) def: DefId,
    /// `@D` anotasyonunun çözüldüğü alan bildirimi.
    pub(super) ann: Option<DefId>,
    /// `reset(sync|async, polarite)`; yazılmamışsa beslediği alandan.
    pub(super) spec: Option<ResetSpec>,
}

impl<'a> UnitFacts<'a> {
    pub(super) fn collect(ast: &'a SourceFile, res: &ResolveResult, dom: &DomainResult) -> Self {
        let instantiated: HashSet<DefId> = res.instance_module.values().copied().collect();
        let mut modules = Vec::new();
        let mut by_def = HashMap::new();
        for &item in &ast.items {
            let ItemKind::Module(m) = &ast.items_arena[item].kind else {
                continue;
            };
            let Some(&def) = res.decl_spans.get(&m.name.span) else {
                continue;
            };
            by_def.insert(def, modules.len());
            modules.push(ModuleFacts {
                decl: m,
                clocks: clock_facts(ast, res, dom, m),
                raws: raw_facts(ast, res, m),
                is_root: !instantiated.contains(&def),
            });
        }
        UnitFacts {
            ast,
            modules,
            by_def,
        }
    }
}

fn clock_facts<'a>(
    ast: &SourceFile,
    res: &ResolveResult,
    dom: &DomainResult,
    m: &'a ModuleDecl,
) -> Vec<ClockFact<'a>> {
    let reset_free = reset_free_bindings(ast, res, m);
    let mut out = Vec::new();
    for port in &m.ports {
        if !matches!(ast.types[port.ty].kind, TypeRefKind::Clock) {
            continue;
        }
        let Some(&def) = res.decl_spans.get(&port.name.span) else {
            continue;
        };
        let Some(DomainId::Explicit(id)) = dom.signal_domains.get(&def).copied() else {
            continue; // hata kurtarma: alan çözülemedi (E3002 zaten var)
        };
        let info = &dom.domains[id as usize];
        let (decl, reset, reset_span) = match info.source {
            DomainSource::Decl(d) => {
                let (reset, span) =
                    declared_reset(ast, res, d).unwrap_or((Some(DEFAULT_RESET), info.span));
                (Some(d), reset, span)
            }
            DomainSource::ClockPort(_) => (None, Some(DEFAULT_RESET), port.name.span),
        };
        out.push(ClockFact {
            port,
            def,
            decl,
            reset,
            reset_span,
            used: res
                .use_spans
                .iter()
                .any(|(s, d)| *d == def && !reset_free.iter().any(|b| contains(b, s))),
        });
    }
    out
}

/// `domain D { reset = ... }` alanı: `(etkin reset, alan span'i)`;
/// alan yazılmamışsa `None` (varsayılan reset geçerli).
fn declared_reset(
    ast: &SourceFile,
    res: &ResolveResult,
    decl: DefId,
) -> Option<(Option<ResetSpec>, Span)> {
    let &item = res.item_of_def.get(&decl)?;
    let ItemKind::Domain(d) = &ast.items_arena[item].kind else {
        return None;
    };
    // sv-emit ile aynı: son yazılan alan geçerli; `reset = none` reset'siz.
    d.fields
        .iter()
        .rev()
        .find_map(|f| match (&f.key, &f.value) {
            (DomainKey::Reset, DomainValue::Reset(spec)) => Some((Some(*spec), f.span)),
            (DomainKey::Reset, DomainValue::ClockEdge(ClockEdge::None)) => Some((None, f.span)),
            _ => None,
        })
}

fn raw_facts<'a>(ast: &SourceFile, res: &ResolveResult, m: &'a ModuleDecl) -> Vec<RawFact<'a>> {
    m.ports
        .iter()
        .filter(|p| is_raw_reset(ast, p))
        .filter_map(|port| {
            let def = *res.decl_spans.get(&port.name.span)?;
            let spec = match ast.types[port.ty].kind {
                TypeRefKind::Reset(spec) => spec,
                _ => None,
            };
            let ann = port
                .domain
                .as_ref()
                .and_then(|a| res.use_spans.get(&a.span).copied());
            Some(RawFact {
                port,
                def,
                ann,
                spec,
            })
        })
        .collect()
}

/// Modülün reset'ini hiçbir flop'a taşımayan örnek bağlamalarının
/// konumları (`wr_clk: sys_clk` bütünü; ADR-0065 R5'): reset'siz yerleşik
/// saat portu (`BuiltinPrim::clock_resets_flops`) ve extern örneğinin her
/// bağlaması (extern'in reset portu yok, ADR-0065 durum tespiti). Bu
/// bağlamalardaki saat kullanımı saati reset örnekleyen yapmaz.
pub(super) fn reset_free_bindings(
    ast: &SourceFile,
    res: &ResolveResult,
    m: &ModuleDecl,
) -> Vec<Span> {
    let mut out = Vec::new();
    for &s in &m.body {
        let StmtKind::Instance(inst) = &ast.stmts[s].kind else {
            continue;
        };
        let prim = match inst.module_path.segments.as_slice() {
            [seg] => BuiltinPrim::from_name(&seg.text),
            _ => None,
        };
        match prim {
            Some(prim) => out.extend(
                inst.bindings
                    .iter()
                    .filter(|b| !prim.clock_resets_flops(&b.port_name.text))
                    .map(|b| b.span),
            ),
            None if is_extern_instance(ast, res, inst) => {
                out.extend(inst.bindings.iter().map(|b| b.span));
            }
            None => {}
        }
    }
    out
}

/// Örneğin hedefi bir `extern module` mü (ADR-0047).
fn is_extern_instance(ast: &SourceFile, res: &ResolveResult, inst: &InstanceDecl) -> bool {
    res.decl_spans
        .get(&inst.name.span)
        .and_then(|d| res.instance_module.get(d))
        .and_then(|t| res.item_of_def.get(t))
        .is_some_and(|&i| matches!(ast.items_arena[i].kind, ItemKind::Extern(_)))
}

/// `inner` konumu `outer`'ın içinde mi (aynı dosya ve açılım bağlamı).
pub(super) fn contains(outer: &Span, inner: &Span) -> bool {
    outer.file == inner.file
        && outer.ctx == inner.ctx
        && outer.start <= inner.start
        && inner.end <= outer.end
}

/// Otomatik reset portunun adı (sv-mapping.md §7): polariteye göre.
pub(super) fn auto_port_name(spec: ResetSpec) -> &'static str {
    match spec.polarity {
        ResetPolarity::ActiveHigh => "rst",
        ResetPolarity::ActiveLow => "rst_n",
    }
}

/// Kaynak yazımı: `reset(async, active_low)` — help satırları için.
pub(super) fn spec_text(spec: ResetSpec) -> String {
    let sync = match spec.sync {
        ResetSync::Async => "async",
        _ => "sync",
    };
    let polarity = match spec.polarity {
        ResetPolarity::ActiveHigh => "active_high",
        ResetPolarity::ActiveLow => "active_low",
    };
    format!("reset({sync}, {polarity})")
}

/// Saat başına besleyen ham port indeksi; `None` = otomatik port
/// (ADR-0065 §1 bağlama tablosu). Tek anotasyonsuz ham port reset'li
/// bütün alanları besler; aksi hâlde yalnız `@D` anotasyonu eşleşen
/// alanı. sv-emit aynı kuralı AST üzerinde yineler (`reset_sync.rs`).
pub(super) fn feeds_of(m: &ModuleFacts) -> Vec<Option<usize>> {
    m.clocks
        .iter()
        .map(|c| {
            c.reset?;
            match m.raws.as_slice() {
                [only] if only.ann.is_none() => Some(0),
                raws => raws.iter().position(|r| r.ann.is_some() && r.ann == c.decl),
            }
        })
        .collect()
}
