//! Hover: sinyal/port için tip + domain + doc; anahtar kelimeler için
//! öğretici metin; stdlib modülleri için imza + açıklama.

use volt_ast::{ClockEdge, PortDir};
use volt_hir::{DefId, DefKind, DomainId, DomainSource};
use volt_span::Span;

use crate::analysis::Analysis;
use crate::docs;

/// Offsetteki öğe için markdown hover içeriği ve kapsanan span.
pub fn hover(analysis: &Analysis, offset: u32) -> Option<(String, Span)> {
    let tokens = volt_syntax::tokenize(analysis.file_id, analysis.source());
    let token = tokens
        .tokens
        .iter()
        .find(|t| t.span.start <= offset && offset < t.span.end)?;

    if token.kind == volt_syntax::TokenKind::Ident {
        let text = &analysis.source()[token.span.start as usize..token.span.end as usize];
        if let Some(def) = analysis.def_at(offset) {
            return Some((def_hover(analysis, def), token.span));
        }
        if let Some(entry) = docs::stdlib_doc(text) {
            return Some((stdlib_hover(entry), token.span));
        }
        return None;
    }

    docs::keyword_doc(token.kind).map(|doc| (doc.to_string(), token.span))
}

fn stdlib_hover(entry: &docs::StdlibDoc) -> String {
    format!("```volt\n{}\n```\n{}", entry.signature, entry.doc)
}

/// "count_r : u8" + "@SysDomain (posedge clk)" + doc yorumu.
fn def_hover(analysis: &Analysis, def: DefId) -> String {
    let res = analysis
        .resolve
        .as_ref()
        .expect("def_at yalnız resolve varken eşleşir");
    let data = &res.defs[def.0 as usize];

    let ty = analysis
        .typeck
        .as_ref()
        .and_then(|t| t.def_types.get(&def).map(|id| t.types.display(*id)));
    let header = match ty {
        Some(ty) => format!("{} : {}", data.name, ty),
        None => data.name.clone(),
    };

    let mut md = format!("```volt\n{header}\n```\n{}", kind_label(data.kind));
    if let Some(line) = domain_line(analysis, def) {
        md.push_str(&format!(" — {line}"));
    }

    let doc = analysis
        .item_doc(def)
        .or_else(|| analysis.port_doc(data.span));
    if let Some(doc) = doc {
        md.push_str(&format!("\n\n{doc}"));
    }
    md
}

/// "@SysDomain (posedge clk)" biçiminde domain satırı.
fn domain_line(analysis: &Analysis, def: DefId) -> Option<String> {
    let domain = analysis.domain.as_ref()?;
    let res = analysis.resolve.as_ref()?;
    match domain.signal_domains.get(&def)? {
        DomainId::Explicit(idx) => {
            let info = &domain.domains[*idx as usize];
            let edge = match info.clock.edge {
                ClockEdge::Posedge => "posedge",
                ClockEdge::Negedge => "negedge",
                ClockEdge::None => "no edge",
            };
            match info.source {
                DomainSource::ClockPort(port) => {
                    let clk = &res.defs[port.0 as usize].name;
                    Some(format!("@{} ({edge} {clk})", info.name))
                }
                DomainSource::Decl(_) => Some(format!("@{} ({edge})", info.name)),
            }
        }
        DomainId::Timeless => Some("timeless (constant / combinational)".to_string()),
        DomainId::Unresolved(_) | DomainId::Error => None,
    }
}

fn kind_label(kind: DefKind) -> &'static str {
    match kind {
        DefKind::Module => "module",
        DefKind::Domain => "clock domain",
        DefKind::Function => "function",
        DefKind::Struct => "struct",
        DefKind::Enum => "enum",
        DefKind::Const => "const",
        DefKind::TypeAlias => "type alias",
        DefKind::ExternModule => "extern module",
        DefKind::Port { dir: PortDir::In } => "input port",
        DefKind::Port { dir: PortDir::Out } => "output port",
        DefKind::Port {
            dir: PortDir::InOut,
        } => "inout port",
        DefKind::Register => "register",
        DefKind::Wire => "wire",
        DefKind::Instance => "module instance",
        DefKind::LocalBinding => "local binding",
        DefKind::LoopVar => "loop variable",
        DefKind::PatternBinding => "pattern binding",
        DefKind::GenericParam => "generic parameter",
        DefKind::EnumVariant { .. } => "enum variant",
        DefKind::Builtin(_) => "builtin",
        DefKind::Import => "import",
        DefKind::Error => "unresolved",
    }
}
