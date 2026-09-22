//! RDC — reset alanı denetimi (ADR-0065 §1, §3).
//!
//! Volt'ta reset bir değer değil, alan özelliğidir: R2/R3/R4/R7 yapısal
//! olarak imkânsızdır (ADR-0065 "Yapısal olarak imkânsız"). Geriye kalan
//! gerçek boşluk reset BIRAKMASININ hangi saate senkron olduğudur:
//!
//! | modül      | kural                                                   | kod          |
//! |------------|---------------------------------------------------------|--------------|
//! | `facts`    | modül olguları: saatler, etkin reset, ham portlar, kök  | —            |
//! | `binding`  | ham port → alan bağlama: belirsizlik, tür, ad çakışması | E3010, E3003 |
//! | `share`    | tek otomatik portun iki saatte paylaşımı (R5/R5'), kök  | E3003, W3010, W3009 |
//! | `converge` | aynı ham reset'in bir saatte iki zinciri (R6)           | E3003        |
//!
//! Geçiş saat çıkarımından SONRA, ayrı koşar (`check_trust` gibi):
//! saat portlarının alanını `DomainResult`'tan okur, çıkarım durumuna
//! dokunmaz. Etkin reset sv-emit'in ürettiğidir (sv-mapping.md §7):
//! `reset` yazılmamış alan ve örtük saat alanı `sync active_high`.

mod binding;
mod converge;
mod facts;
mod share;

use volt_ast::{ItemKind, Port, PortDir, SourceFile, TypeRefKind};
use volt_diagnostics::{Diagnostic, ErrorCode};

use super::{DomainId, DomainResult, Inferencer};
use crate::resolve::{DefId, ResolveResult};

use facts::UnitFacts;

/// Birimdeki bütün modüllerin reset sözleşmesini denetler (ADR-0065).
pub fn check_rdc(ast: &SourceFile, res: &ResolveResult, dom: &DomainResult) -> Vec<Diagnostic> {
    let facts = UnitFacts::collect(ast, res, dom);
    let mut rdc = Rdc {
        res,
        facts: &facts,
        diagnostics: Vec::new(),
        release_memo: Default::default(),
    };
    for m in 0..facts.modules.len() {
        rdc.check_module(m);
    }
    rdc.diagnostics
}

/// Ham reset portu adıyla okunmaz: bırakma senkronizörü onu örtük okur
/// (ADR-0065 §2). Kullanım takibi (`resolve/`, W1001) bunu bilemez;
/// tüketiciler resolve tanılarını bu süzgeçten geçirir.
pub fn without_raw_reset_unused(ast: &SourceFile, diags: &[Diagnostic]) -> Vec<Diagnostic> {
    let raw_spans: Vec<volt_span::Span> = ast
        .items
        .iter()
        .filter_map(|&i| match &ast.items_arena[i].kind {
            ItemKind::Module(m) => Some(m),
            _ => None,
        })
        .flat_map(|m| m.ports.iter())
        .filter(|p| is_raw_reset(ast, p))
        .map(|p| p.name.span)
        .collect();
    diags
        .iter()
        .filter(|d| {
            d.code != ErrorCode::W1001
                || !d
                    .primary_span()
                    .is_some_and(|s| raw_spans.contains(&s.span))
        })
        .cloned()
        .collect()
}

/// Ham reset portu (ADR-0065 §1): `in r : reset(...)`. Yalnız giriş
/// yönü; çıkış ve çift yönlü reset portu sv-emit'te E0003 kalır.
pub(crate) fn is_raw_reset(ast: &SourceFile, port: &Port) -> bool {
    port.direction == PortDir::In && matches!(ast.types[port.ty].kind, TypeRefKind::Reset(_))
}

struct Rdc<'a> {
    res: &'a ResolveResult,
    facts: &'a UnitFacts<'a>,
    diagnostics: Vec<Diagnostic>,
    /// (modül, ham port) → bırakmanın senkronlandığı saat portları (R6).
    release_memo: std::collections::HashMap<(usize, DefId), Vec<DefId>>,
}

impl Rdc<'_> {
    fn check_module(&mut self, m: usize) {
        let reported = self.diagnostics.len();
        let feeds = self.check_raw_bindings(m);
        // Belirsiz ham port (E3010) hiçbir alanı beslemez: paylaşım ve kök
        // denetimi kaskad üretirdi — kullanıcının niyeti ham porttur.
        let ambiguous = self.diagnostics[reported..]
            .iter()
            .any(|d| d.code == ErrorCode::E3010);
        if !ambiguous && !self.check_shared_ports(m, &feeds) {
            self.check_root_contract(m, &feeds);
        }
        for raw in 0..self.facts.modules[m].raws.len() {
            let def = self.facts.modules[m].raws[raw].def;
            self.release_clocks(m, def);
        }
    }
}

impl Inferencer<'_> {
    /// Ham reset portu mu (ADR-0065 §1) — saat çıkarımı onu veri portu
    /// gibi alana atamaz.
    pub(super) fn is_raw_reset_port(&self, port: &Port, _def: DefId) -> bool {
        is_raw_reset(self.ast, port)
    }

    /// Ham reset portu hiçbir saat alanına ait değildir: bırakması
    /// hiçbir saate senkron gelmez, beslediği alanlar `rdc` geçidinde
    /// belirlenir. Çoklu saatte anotasyonsuz olması E3010 DEĞİLDİR
    /// (bütün alanları besler); anotasyon yine çözülür (E3002).
    pub(super) fn assign_raw_reset_domain(&mut self, port: &Port, def: DefId) {
        if let Some(ann) = &port.domain {
            self.signal_annotation_domain(&ann.clone());
        }
        self.signal_domains.insert(def, DomainId::Timeless);
    }
}

#[cfg(test)]
mod tests;
