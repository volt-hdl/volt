//! Saat alanı (domain) çıkarımı ve CDC kontrolü
//! (docs/spec/domain-inference.md K1-K9).
//!
//! Domain her sinyalde VAR ama çoğu zaman YAZILMIYOR (UX Anayasası):
//! tek saatli modülde anotasyonsuz her sinyal o saatin alanına atanır,
//! kullanıcı 'domain' kelimesini hiç görmez (K2). Kullanıcı domain
//! kavramıyla yalnız gerçek bir sorun olduğunda karşılaşır: çoklu saatte
//! belirsizlik (E3010) veya CDC ihlali (E3001).
//!
//! Geçiş tip kontrolünden SONRA koşar; saat portları `Ty::Clock`
//! üzerinden bulunur, sync() genişliği `expr_types`'tan okunur.
//!
//! Modüller K kurallarını ve ADR'leri izler:
//!
//! | modül              | sorumluluk                                         | kural / ADR        |
//! |--------------------|----------------------------------------------------|--------------------|
//! | `mod`              | public API, `Inferencer`, dosya seviyesi akış      | §1, §3             |
//! | `tables`           | domain tablosu, örtük saat alanı, ortak sorgular   | K2, §1             |
//! | `join`             | `join_domains`, kısıt değişkenleri, E3001 (komb.)  | K5                 |
//! | `annotation`       | `@Ad` / `reg(clk)` anotasyonu, E3002               | K1                 |
//! | `trust_alias`      | trust_level'lı anotasyon saat alanı açmaz          | K11, ADR-0052      |
//! | `module`           | modül akışı: saat taraması, port ataması, E3010    | K1, K2, K3, §3     |
//! | `bundle`           | bundle portu tek alanda, E3013                     | K10, ADR-0039      |
//! | `bidir`            | çift yönlü port okuması, W3007                     | ADR-0051           |
//! | `extern_decl`      | extern sınırının domain sözleşmesi                 | ADR-0047           |
//! | `reg`              | register alanı, yazıcı taraması, E3011, W3001      | K4                 |
//! | `walk`             | deyim ve blok yürüyüşü                             | K5-K9, §3          |
//! | `assign`           | atama uyumu, 'on' bloğu, E3001 (atama), E3012      | K6, K7             |
//! | `expr`             | ifade alanı, kombinasyonel yayılım                 | K5                 |
//! | `sync`             | `sync()` köprüsü, W3002, W3003                     | K9                 |
//! | `instance`         | örnekleme haritası, E3014                          | K8, ADR-0047       |
//! | `builtin_instance` | yerleşik CDC primitifleri, W3005, W3006            | ADR-0027/0029/0049 |

mod annotation;
mod assign;
mod bidir;
mod builtin_instance;
mod bundle;
mod expr;
mod extern_decl;
mod instance;
mod join;
mod module;
mod reg;
mod sync;
mod tables;
mod trust_alias;
mod walk;

use std::collections::{HashMap, HashSet};

use volt_ast::{ClockEdge, Expr, Idx, ItemKind, ResetSpec, SourceFile, TrustLevel};
use volt_diagnostics::Diagnostic;
use volt_span::Span;

use crate::resolve::{DefId, ResolveResult};
use crate::typeck::TypeckResult;

/// Çözülmemiş domain değişkeni — çıkarım sırasında kısıt biriktirir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InferVar(pub u32);

/// domain-inference.md §1 — sinyalin ait olduğu saat alanı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DomainId {
    /// Belirli bir saat alanı (`DomainResult::domains` indeksi).
    Explicit(u32),
    /// Saatten bağımsız — sabitler, saf kombinasyonel.
    Timeless,
    /// Henüz çözülmemiş (çıkarım sırasında).
    Unresolved(InferVar),
    /// Hata kurtarma — her domainle uyumlu.
    Error,
}

/// Saat kenarı bilgisi (domain-inference.md §1 ClockSpec).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockSpec {
    pub edge: ClockEdge,
}

/// Bir saat alanının tanım bilgisi.
#[derive(Debug, Clone)]
pub struct DomainInfo {
    pub name: String,
    pub clock: ClockSpec,
    pub reset: ResetSpec,
    /// Tanım satırı — E3001/E3010 ikincil etiketleri buraya bağlanır.
    pub span: Span,
    pub source: DomainSource,
    /// `trust_level = ...` (ADR-0052); yazılmamışsa `None` = sınıflandırılmamış.
    pub trust: Option<TrustLevel>,
    /// `trust_level` alanının span'i — E3009 "trust level here" etiketi.
    pub trust_span: Option<Span>,
}

/// Domain nereden geliyor: açık `domain` bildirimi veya anotasyonsuz
/// clock portunun ürettiği örtük alan (K2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainSource {
    Decl(DefId),
    ClockPort(DefId),
}

/// Domain çıkarımı çıktısı.
#[derive(Debug, Default)]
pub struct DomainResult {
    pub domains: Vec<DomainInfo>,
    /// Sinyal tanımı → çıkarılan saat alanı.
    pub signal_domains: HashMap<DefId, DomainId>,
    /// `domain Ad { ... }` bildirimi → `domains` indeksi (güven geçidi
    /// anotasyonun bildirimine buradan ulaşır, ADR-0052).
    pub decl_domains: HashMap<DefId, u32>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Dosyadaki tüm modüllerin saat alanlarını çıkarır ve CDC denetler.
pub fn infer_domains(ast: &SourceFile, res: &ResolveResult, tyck: &TypeckResult) -> DomainResult {
    let mut inf = Inferencer::new(ast, res, tyck);
    inf.collect_domain_decls();
    for &item_idx in &ast.items {
        match &ast.items_arena[item_idx].kind {
            ItemKind::Module(m) => inf.infer_module(m),
            ItemKind::Extern(x) => inf.check_extern_decl(x),
            _ => {}
        }
    }
    DomainResult {
        domains: inf.domains,
        signal_domains: inf.signal_domains,
        decl_domains: inf.by_decl,
        diagnostics: inf.diagnostics,
    }
}

struct Inferencer<'a> {
    ast: &'a SourceFile,
    res: &'a ResolveResult,
    tyck: &'a TypeckResult,
    domains: Vec<DomainInfo>,
    /// `domain Ad { ... }` bildirimi → domain indeksi.
    by_decl: HashMap<DefId, u32>,
    /// Anotasyonsuz clock portu → örtük domain indeksi.
    by_clock_port: HashMap<DefId, u32>,
    signal_domains: HashMap<DefId, DomainId>,
    expr_domains: HashMap<Idx<Expr>, DomainId>,
    /// Unresolved değişken bağlamaları (kısıt çözümü).
    vars: Vec<Option<DomainId>>,
    /// Instance tanımı → hedef port adı → beklenen domain (K8).
    instance_ports: HashMap<DefId, HashMap<String, DomainId>>,
    diagnostics: Vec<Diagnostic>,

    // ── Modül bağlamı ──
    default_domain: DomainId,
    multi_clock: bool,
    /// E3010 aday listesi: (port span, domain görünen adı).
    clock_candidates: Vec<(Span, String)>,
    /// Çift yönlü portlar (ADR-0051): okuması harici sayılır (W3007).
    bidir_ports: HashSet<DefId>,
    /// `sync()` kaynağı çözümleniyor (W3007 bastırılır).
    in_sync_source: bool,
    /// Kontrat ifadesi çözümleniyor (W3007 bastırılır).
    in_contract: bool,
    /// Bu modülün bir clock portunun taşıdığı domain bildirimleri (K11,
    /// ADR-0052): taşınmayan, trust_level yazılmış anotasyon yeni saat
    /// alanı açmaz.
    anchored: HashSet<DefId>,
}

impl<'a> Inferencer<'a> {
    fn new(ast: &'a SourceFile, res: &'a ResolveResult, tyck: &'a TypeckResult) -> Self {
        Inferencer {
            ast,
            res,
            tyck,
            domains: Vec::new(),
            by_decl: HashMap::new(),
            by_clock_port: HashMap::new(),
            signal_domains: HashMap::new(),
            expr_domains: HashMap::new(),
            vars: Vec::new(),
            instance_ports: HashMap::new(),
            diagnostics: Vec::new(),
            default_domain: DomainId::Timeless,
            multi_clock: false,
            clock_candidates: Vec::new(),
            bidir_ports: HashSet::new(),
            in_sync_source: false,
            in_contract: false,
            anchored: HashSet::new(),
        }
    }
}

#[cfg(test)]
mod testutil;

#[cfg(test)]
mod tests {
    use super::testutil::inferred;
    use super::{DomainId, DomainSource};

    #[test]
    fn every_module_in_the_file_is_inferred() {
        let r = inferred(
            "module A {\n    in clk : clock\n    in a : u8\n}\n\
             module B {\n    in clk2 : clock\n    in b : u8\n}\n",
        );
        assert_eq!(r.domain_name_of("a"), Some("clk"));
        assert_eq!(r.domain_name_of("b"), Some("clk2"));
        assert!(r.codes().is_empty(), "{:?}", r.codes());
    }

    #[test]
    fn decl_domains_maps_declarations_to_indices() {
        let r = inferred(
            "domain D { clock = posedge, reset = sync active_high }\n\
             module M {\n    in clk : clock @D\n}\n",
        );
        let (def, _) = r.res.def_by_name("D").expect("D");
        let id = r.dom.decl_domains[&def];
        assert_eq!(r.dom.domains[id as usize].source, DomainSource::Decl(def));
        assert_eq!(r.domain_of("clk"), DomainId::Explicit(id));
    }

    #[test]
    fn extern_declarations_are_checked_too() {
        let r = inferred("extern module X {\n    in a : u8 @Sym\n}\n");
        assert_eq!(r.codes(), ["E3002"]);
    }
}
