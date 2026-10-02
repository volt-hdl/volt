//! Nitelik tüketimi (ADR-0098): kaynaktaki hiçbir nitelik sessizce atılmaz.
//!
//! İki ayrı kayıp yolu vardı:
//!
//! 1. **Bağlanmayan nitelik.** `parse_attributes` her `@ad(...)` dizisini
//!    ayrıştırır; ardından gelen yapı nitelik taşımıyorsa (kontrat,
//!    `stage`/`stall`/`flush`, `use`, `package`, gövdenin kapanış `}`'i,
//!    dosya sonu, bozuk alan) liste düşerdi. Her ayrıştırılan nitelik bir
//!    deftere (`attr_ledger`) yazılır; öğeler ayrıştırıldıktan sonra AST'de
//!    bulunmayanlar W0024 alır. Yeni bir ayrıştırma yolu nitelikleri
//!    unutursa da defter onu yakalar.
//! 2. **Okunmayan yerdeki nitelik.** Nitelik AST'ye bağlanır ama o düğümde
//!    hiçbir geçit onu okumaz (`@strict_timing` bir portta, `@offset` tek
//!    başına). [`placement`] her tanınan nitelik için okunduğu (ya da
//!    yanlış yerini kendi geçidinin zaten raporladığı) yerleri listeler;
//!    dışındaki her yer W0024. Tanınan her nitelik tabloda olmalı (test).
//!
//! Bilinmeyen nitelik zaten W0020 ("yok sayılır") alır; ikinci tanı yok.

use std::collections::HashSet;

use volt_ast::{Attribute, ItemKind, StmtKind};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use super::Parser;
use crate::token::TokenKind;

/// Nitelik dizisinin ardından gelen yapı — bağlanmayan niteliğin
/// iletisini o yapıya göre seçer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Follows {
    Contract,
    /// `stage`, `stall`, `flush` (pipeline gövdesi).
    PipelineClause,
    Use,
    Package,
    /// Gövdenin kapanış `}`'i ya da dosya sonu.
    End,
    Other,
}

impl Follows {
    pub(crate) fn of(token: Option<TokenKind>) -> Self {
        use TokenKind::*;
        match token {
            Some(k) if super::item::contract_kind(k).is_some() => Follows::Contract,
            Some(KwStage | KwStall | KwFlush) => Follows::PipelineClause,
            Some(KwUse) => Follows::Use,
            Some(KwPackage) => Follows::Package,
            Some(RBrace) | None => Follows::End,
            Some(_) => Follows::Other,
        }
    }
}

/// Defter kaydı: ayrıştırılan bir nitelik.
#[derive(Debug, Clone)]
pub(crate) struct AttrMark {
    span: Span,
    name_span: Span,
    name: String,
    follows: Follows,
}

impl AttrMark {
    pub(crate) fn new(attr: &Attribute, follows: Follows) -> Self {
        AttrMark {
            span: attr.span,
            name_span: attr.name.span,
            name: attr.name.text.clone(),
            follows,
        }
    }
}

/// Niteliğin bağlandığı yer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Site {
    Module,
    Extern,
    /// fn, struct, enum, const, type, domain, test.
    OtherItem,
    Port,
    ExternPort,
    StructField,
    /// `@reg(...) ad : { ... }` register bildirimi (ADR-0044).
    MmioReg,
    /// `reg` bildirimi deyimi.
    RegStmt,
    /// Diğer modül deyimleri (`let`, `wire`, `on`, atama, örnek...).
    OtherStmt,
}

impl Site {
    fn describe(self) -> String {
        match self {
            Site::Module => lstr!(en: "a module"; tr: "bir modül"),
            Site::Extern => lstr!(en: "an extern module"; tr: "bir extern modül"),
            Site::OtherItem => lstr!(en: "this item"; tr: "bu öğe"),
            Site::Port => lstr!(en: "a port"; tr: "bir port"),
            Site::ExternPort => lstr!(en: "an extern module port"; tr: "bir extern modül portu"),
            Site::StructField => lstr!(en: "a struct field"; tr: "bir struct alanı"),
            Site::MmioReg => lstr!(en: "an @reg register"; tr: "bir @reg register'ı"),
            Site::RegStmt => lstr!(en: "a reg declaration"; tr: "bir reg bildirimi"),
            Site::OtherStmt => lstr!(en: "this statement"; tr: "bu deyim"),
        }
    }
}

use Site::*;

/// Her tanınan niteliğin okunduğu yerler. Bir yer listedeyse ya bir
/// geçit niteliği orada okur ya da yanlış kullanımını kendi tanısıyla
/// raporlar (çift tanı olmasın):
///
/// * uygulanmayanlar (`attrs.rs` `UNENFORCED_ATTRIBUTES`) her öğe, port,
///   struct alanı ve deyimde W0021 alır; `@reg` bildirimleri o geçidin
///   dışında kalır;
/// * `@allow` aynı yerlerde `attrs.rs`'te okunur;
/// * `@timing`/`@false_path`/`@multicycle` modül, port ve deyimde
///   `constraints` geçidinde okunur ya da E0017 alır;
/// * `@source` öğe, port, struct alanı ve deyimde `extern_source`'ta
///   okunur ya da E0009 alır.
///
/// Register alanındaki (`@reg` gövdesi) her nitelik `mmio` desugar'ında
/// okunur ya da E0009 alır; o yer bu denetimin dışındadır.
///
/// `None`: tanınmayan nitelik (W0020).
pub(crate) fn placement(name: &str) -> Option<&'static [Site]> {
    const EVERYWHERE_BUT_MMIO_REG: &[Site] = &[
        Module,
        Extern,
        OtherItem,
        Port,
        ExternPort,
        StructField,
        RegStmt,
        OtherStmt,
    ];
    Some(match name {
        "domain" | "budget" | "version" | "abi_version" | "dft" | "debug_visible"
        | "debug_trace" | "synthesis_target" | "allow" | "source" => EVERYWHERE_BUT_MMIO_REG,
        "mmio" => &[Module],
        "reg" => &[MmioReg],
        // Yalnız register alanında (o yer denetim dışı) ya da @reg argümanı.
        "offset" | "access" | "reserved" | "self_clearing" | "w1c" => &[],
        "timing" | "false_path" | "multicycle" => &[Module, Port, RegStmt, OtherStmt],
        "strict_timing" => &[Module],
        "no_protocol_check" => &[Module, Port],
        "no_auto_contracts" => &[Module, RegStmt],
        _ => return None,
    })
}

/// W0024 yardımında gösterilen doğru yer.
fn proper_place(name: &str) -> String {
    match name {
        "mmio" => {
            lstr!(en: "write it before 'module' (ADR-0044)"; tr: "'module' önüne yazın (ADR-0044)")
        }
        "reg" => lstr!(
            en: "it declares a memory-mapped register: @reg(offset = 0x00, access = ReadWrite) name : {{ field : bool }} in an @mmio module";
            tr: "bellek eşlemeli register bildirir: @mmio modülünde @reg(offset = 0x00, access = ReadWrite) ad : {{ alan : bool }}"
        ),
        "offset" | "access" => lstr!(
            en: "it is an argument of @reg: @reg(offset = 0x04, access = ReadWrite)";
            tr: "@reg'in argümanıdır: @reg(offset = 0x04, access = ReadWrite)"
        ),
        "reserved" | "self_clearing" | "w1c" => lstr!(
            en: "it marks a field of an @reg register (ADR-0044)";
            tr: "bir @reg register'ının alanını işaretler (ADR-0044)"
        ),
        "timing" => lstr!(en: "write it before 'module'"; tr: "'module' önüne yazın"),
        "false_path" | "multicycle" => lstr!(
            en: "write it before 'module', a module port or a 'reg' declaration (ADR-0054)";
            tr: "'module', bir modül portu ya da bir 'reg' bildirimi önüne yazın (ADR-0054)"
        ),
        "strict_timing" => {
            lstr!(en: "write it before 'module' (ADR-0037)"; tr: "'module' önüne yazın (ADR-0037)")
        }
        "no_protocol_check" => lstr!(
            en: "write it before 'module' or a Handshake port (ADR-0050)";
            tr: "'module' ya da bir Handshake portu önüne yazın (ADR-0050)"
        ),
        "no_auto_contracts" => lstr!(
            en: "write it before 'module' or a 'reg' declaration (ADR-0066)";
            tr: "'module' ya da bir 'reg' bildirimi önüne yazın (ADR-0066)"
        ),
        _ => lstr!(
            en: "write it before an item, port, struct field or module statement";
            tr: "bir öğe, port, struct alanı ya da modül deyimi önüne yazın"
        ),
    }
}

impl Parser<'_> {
    /// `first_item`'dan sonra eklenen öğelerin niteliklerini denetler
    /// (desugar ÖNCESİ: yer kaynaktaki yerdir).
    pub(crate) fn check_attribute_use(&mut self, first_item: usize) {
        let mut attached: Vec<(&Attribute, Site)> = Vec::new();
        // Bağlı ama yeri başka geçitte denetlenen nitelikler.
        let mut bound: Vec<Span> = Vec::new();
        let ast = &self.ast;
        for &item_idx in &ast.items[first_item..] {
            let item = &ast.items_arena[item_idx];
            let site = match &item.kind {
                ItemKind::Module(_) => Module,
                ItemKind::Extern(_) => Extern,
                // Kurtarılan öğe zaten hata aldı; nitelik gürültüsü eklenmez.
                ItemKind::Error => {
                    continue;
                }
                ItemKind::Domain(_)
                | ItemKind::Fn(_)
                | ItemKind::Struct(_)
                | ItemKind::Enum(_)
                | ItemKind::Const(_)
                | ItemKind::TypeAlias(_)
                | ItemKind::Test(_) => OtherItem,
            };
            attached.extend(item.attrs.iter().map(|a| (a, site)));
            match &item.kind {
                ItemKind::Module(m) => {
                    attached.extend(
                        m.ports
                            .iter()
                            .flat_map(|p| p.attrs.iter().map(|a| (a, Port))),
                    );
                    for &s in &m.body {
                        let stmt = &ast.stmts[s];
                        let site = match stmt.kind {
                            StmtKind::Reg(_) => RegStmt,
                            StmtKind::Let(_)
                            | StmtKind::Wire(_)
                            | StmtKind::Instance(_)
                            | StmtKind::On(_)
                            | StmtKind::Comb(_)
                            | StmtKind::Assign(_)
                            | StmtKind::For(_)
                            | StmtKind::Expr(_)
                            | StmtKind::Error => OtherStmt,
                        };
                        attached.extend(stmt.attrs.iter().map(|a| (a, site)));
                    }
                    for r in &m.mmio_regs {
                        attached.extend(r.attrs.iter().map(|a| (a, MmioReg)));
                        // Alan nitelikleri bağlıdır; yerini mmio denetler.
                        bound.extend(r.fields.iter().flat_map(|f| f.attrs.iter().map(|a| a.span)));
                    }
                }
                ItemKind::Extern(x) => {
                    attached.extend(
                        x.ports
                            .iter()
                            .flat_map(|p| p.attrs.iter().map(|a| (a, ExternPort))),
                    );
                }
                ItemKind::Struct(s) => {
                    attached.extend(
                        s.fields
                            .iter()
                            .flat_map(|f| f.attrs.iter().map(|a| (a, StructField))),
                    );
                }
                ItemKind::Domain(_)
                | ItemKind::Fn(_)
                | ItemKind::Enum(_)
                | ItemKind::Const(_)
                | ItemKind::TypeAlias(_)
                | ItemKind::Test(_)
                | ItemKind::Error => {}
            }
        }

        let spans: HashSet<Span> = attached.iter().map(|(a, _)| a.span).chain(bound).collect();
        let mut diags: Vec<Diagnostic> = std::mem::take(&mut self.attr_ledger)
            .into_iter()
            .filter(|m| !spans.contains(&m.span) && placement(&m.name).is_some())
            .map(|m| unattached(&m))
            .collect();
        for (attr, site) in attached {
            let Some(sites) = placement(&attr.name.text) else {
                continue;
            };
            if !sites.contains(&site) {
                diags.push(misplaced(attr, site));
            }
        }
        diags.sort_by_key(|d| d.spans.first().map(|s| (s.span.file.0, s.span.start)));
        self.diagnostics.extend(diags);
    }
}

/// `@ad` (argümanlar hariç) — neyin yok sayıldığı bir bakışta görünür.
fn name_span(span: Span, name: Span) -> Span {
    Span {
        end: name.end.min(span.end),
        ..span
    }
}

/// W0024 — bağlanmayan nitelik.
fn unattached(m: &AttrMark) -> Diagnostic {
    let name = &m.name;
    let span = name_span(m.span, m.name_span);
    let (label, help) = match m.follows {
        Follows::Contract => (
            lstr!(en: "a contract takes no attributes"; tr: "kontrat nitelik almaz"),
            lstr!(
                en: "remove it, or move it before 'module' if it is meant for the whole module";
                tr: "kaldırın ya da bütün modül içinse 'module' önüne taşıyın"
            ),
        ),
        Follows::PipelineClause => (
            lstr!(en: "'stage', 'stall' and 'flush' take no attributes"; tr: "'stage', 'stall' ve 'flush' nitelik almaz"),
            lstr!(
                en: "remove it, or move it before 'pipeline' if it is meant for the whole pipeline";
                tr: "kaldırın ya da bütün pipeline içinse 'pipeline' önüne taşıyın"
            ),
        ),
        Follows::Use | Follows::Package => (
            lstr!(en: "'use' and 'package' take no attributes"; tr: "'use' ve 'package' nitelik almaz"),
            lstr!(en: "remove it, or move it before the item it is meant for"; tr: "kaldırın ya da ait olduğu öğenin önüne taşıyın"),
        ),
        Follows::End => (
            lstr!(en: "nothing follows the attribute"; tr: "nitelikten sonra hiçbir şey gelmiyor"),
            lstr!(en: "remove it, or move it before the item, port or statement it is meant for"; tr: "kaldırın ya da ait olduğu öğe, port ya da deyimin önüne taşıyın"),
        ),
        Follows::Other => (
            lstr!(en: "nothing here takes attributes"; tr: "burada hiçbir şey nitelik almaz"),
            proper_place(name),
        ),
    };
    Diagnostic::warning(
        ErrorCode::W0024,
        lstr!(en: "attribute '@{name}' is not attached to anything and is ignored";
              tr: "'@{name}' niteliği hiçbir şeye bağlanmıyor, yok sayılıyor"),
        LabeledSpan::primary(span, label),
        help,
    )
    .with_note(NoteKind::Note, attachable_note())
}

/// W0024 — okunmayan yerdeki nitelik.
fn misplaced(attr: &Attribute, site: Site) -> Diagnostic {
    let name = &attr.name.text;
    let span = name_span(attr.span, attr.name.span);
    let place = site.describe();
    Diagnostic::warning(
        ErrorCode::W0024,
        lstr!(en: "attribute '@{name}' has no effect on {place} and is ignored";
              tr: "'@{name}' niteliğinin {place} üzerinde etkisi yok, yok sayılıyor"),
        LabeledSpan::primary(
            span,
            lstr!(en: "no compiler pass reads it here"; tr: "hiçbir derleyici geçidi onu burada okumuyor"),
        ),
        proper_place(name),
    )
    .with_note(NoteKind::Note, attachable_note())
}

fn attachable_note() -> String {
    lstr!(
        en: "attributes attach to the item, port, struct field or module statement that follows them (grammar-full.ebnf §2)";
        tr: "nitelik ardından gelen öğeye, porta, struct alanına ya da modül deyimine bağlanır (grammar-full.ebnf §2)"
    )
}

#[cfg(test)]
mod tests {
    use super::super::item::KNOWN_ATTRIBUTES;
    use super::*;

    /// Yeni bir nitelik eklenince nerede okunduğu da yazılmalı: aksi hâlde
    /// her yerde W0024 alır ya da (yazılmazsa) bu test düşer.
    #[test]
    fn every_known_attribute_has_a_placement() {
        for name in KNOWN_ATTRIBUTES {
            assert!(placement(name).is_some(), "@{name} için yer tablosu yok");
        }
    }

    #[test]
    fn every_placement_entry_is_a_known_attribute() {
        for name in [
            "domain",
            "budget",
            "version",
            "abi_version",
            "dft",
            "debug_visible",
            "debug_trace",
            "synthesis_target",
            "allow",
            "source",
            "mmio",
            "reg",
            "offset",
            "access",
            "reserved",
            "self_clearing",
            "w1c",
            "timing",
            "false_path",
            "multicycle",
            "strict_timing",
            "no_protocol_check",
            "no_auto_contracts",
        ] {
            assert!(
                KNOWN_ATTRIBUTES.contains(&name),
                "@{name} tanınan nitelik değil"
            );
        }
        assert_eq!(
            KNOWN_ATTRIBUTES.len(),
            23,
            "nitelik eklendi: tabloyu güncelle"
        );
    }
}
