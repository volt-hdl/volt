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
        // Bundle portu (struct port / Handshake) düz portlara açılmıştır;
        // kullanıcının yazdığı port adı hiçbir tanıma karşılık gelmez.
        if let Some(md) = bundle_hover(analysis, token.span) {
            return Some((md, token.span));
        }
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

/// `bus : Bus` + açılan portlar (ADR-0070 §3.2). Tip, kullanıcının
/// yazdığı biçimdir (`Handshake<u8>`); alan tipleri tip denetiminden.
fn bundle_hover(analysis: &Analysis, name_span: Span) -> Option<String> {
    let src = analysis.source();
    let flat: Vec<&volt_ast::Port> = analysis
        .ast
        .items
        .iter()
        .filter_map(|&i| match &analysis.ast.items_arena[i].kind {
            volt_ast::ItemKind::Module(m) => Some(m),
            _ => None,
        })
        .flat_map(|m| m.ports.iter())
        .filter(|p| p.bundle.as_ref().is_some_and(|b| b.port.span == name_span))
        .collect();
    let first = flat.first()?;
    let origin = first.bundle.as_ref()?;
    let decl = src
        .get(first.span.start as usize..first.span.end as usize)
        .unwrap_or("");
    let ty = decl
        .split_once(':')
        .map(|(_, t)| t.lines().next().unwrap_or("").trim())
        .filter(|t| !t.is_empty())
        .unwrap_or(&origin.bundle);
    let mut md = format!(
        "```volt
{} : {ty}
```
port bundle — flattened to:",
        origin.port.text
    );
    for p in flat {
        let dir = match p.direction {
            PortDir::In => "in",
            PortDir::Out => "out",
            PortDir::InOut => "inout",
            PortDir::OpenDrain => "opendrain",
        };
        let field_ty = analysis
            .resolve
            .as_ref()
            .zip(analysis.typeck.as_ref())
            .and_then(|(res, tc)| {
                let def = res.decl_spans.iter().find(|(s, _)| **s == p.name.span)?.1;
                tc.def_types
                    .get(def)
                    .map(|id| tc.types.display_named(*id, &res.defs))
            })
            .unwrap_or_else(|| "?".to_string());
        md.push_str(&format!(
            "
- `{dir} {} : {field_ty}`",
            p.name.text
        ));
    }
    Some(md)
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
    if let Some(md) = enum_hover(analysis, def) {
        return md;
    }

    let ty = analysis.typeck.as_ref().and_then(|t| {
        t.def_types
            .get(&def)
            .map(|id| t.types.display_named(*id, &res.defs))
    });
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

/// Enum ve varyant hover'ı (ADR-0074): varyantta
/// `State::Run = 2'd1` + `enum State (2 bit)`; enum'da kodlama tablosu.
fn enum_hover(analysis: &Analysis, def: DefId) -> Option<String> {
    let res = analysis.resolve.as_ref()?;
    let (enum_def, variant) = match res.def_kind(def) {
        DefKind::Enum => (def, None),
        DefKind::EnumVariant { parent } => (parent, Some(res.defs[def.0 as usize].name.clone())),
        _ => return None,
    };
    let item = *res.item_of_def.get(&enum_def)?;
    let volt_ast::ItemKind::Enum(decl) = &analysis.ast.items_arena[item].kind else {
        return None;
    };
    let ast = &analysis.ast;
    let layout =
        volt_ast::enum_layout::valid_layout(ast, decl, &mut |e| match &ast.exprs[e].kind {
            volt_ast::ExprKind::IntLit { value, .. } => i128::try_from(*value).ok(),
            _ => None,
        });
    let name = &decl.name.text;
    let summary = match &layout {
        Some(l) => format!("enum {name} ({} bit)", l.width),
        None => format!("enum {name}"),
    };
    let code = |i: usize| {
        layout.as_ref().map_or(String::new(), |l| {
            format!(" = {}'d{}", l.width, l.values[i])
        })
    };
    let mut md = match &variant {
        Some(v) => {
            let i = decl.variants.iter().position(|x| x.name.text == *v)?;
            format!(
                "```volt
{name}::{v}{}
```
{summary}",
                code(i)
            )
        }
        None => {
            let rows: Vec<String> = (0..decl.variants.len())
                .map(|i| format!("- `{}{}`", decl.variants[i].name.text, code(i)))
                .collect();
            format!(
                "```volt
{summary}
```
{}",
                rows.join(
                    "
"
                )
            )
        }
    };
    if let Some(doc) = analysis.item_doc(enum_def).filter(|_| variant.is_none()) {
        md.push_str(&format!(
            "

{doc}"
        ));
    }
    Some(md)
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
        DefKind::Port {
            dir: PortDir::OpenDrain,
        } => "open-drain port",
        DefKind::Register => "register",
        DefKind::Wire => "wire",
        DefKind::Instance => "module instance",
        DefKind::LocalBinding => "local binding",
        DefKind::LoopVar => "loop variable",
        DefKind::PatternBinding => "pattern binding",
        DefKind::GenericParam => "generic parameter",
        DefKind::DomainParam => "symbolic clock domain",
        DefKind::EnumVariant { .. } => "enum variant",
        DefKind::Builtin(_) => "builtin",
        DefKind::Import => "import",
        DefKind::Error => "unresolved",
    }
}
