//! Document symbols: outline paneli için modül → port / register /
//! wire / örnek ağacı; üst düzeyde domain, struct, enum, const, fn.

use tower_lsp::lsp_types::{DocumentSymbol, SymbolKind};
use volt_ast::{ItemKind, ModuleDecl, PortDir, StmtKind};
use volt_span::{SourceMap, Span};

use crate::analysis::Analysis;
use crate::convert::span_to_range;

// DocumentSymbol::deprecated alanı LSP'de kullanımdan kalktı ama yapı
// alanı hâlâ zorunlu — kurulumda izin veriyoruz.
#[allow(deprecated)]
fn symbol(
    map: &SourceMap,
    name: &str,
    detail: Option<String>,
    kind: SymbolKind,
    full: Span,
    selection: Span,
    children: Vec<DocumentSymbol>,
) -> DocumentSymbol {
    DocumentSymbol {
        name: name.to_string(),
        detail,
        kind,
        tags: None,
        deprecated: None,
        range: span_to_range(map, full),
        selection_range: span_to_range(map, selection),
        children: (!children.is_empty()).then_some(children),
    }
}

/// Dosyanın sembol ağacı.
pub fn document_symbols(analysis: &Analysis) -> Vec<DocumentSymbol> {
    let map = &analysis.map;
    let mut out = Vec::new();
    for &item_idx in &analysis.ast.items {
        let item = &analysis.ast.items_arena[item_idx];
        match &item.kind {
            ItemKind::Module(m) => {
                let children = module_children(analysis, m);
                out.push(symbol(
                    map,
                    &m.name.text,
                    None,
                    SymbolKind::MODULE,
                    item.span,
                    m.name.span,
                    children,
                ));
            }
            ItemKind::Domain(d) => out.push(symbol(
                map,
                &d.name.text,
                Some("domain".to_string()),
                SymbolKind::NAMESPACE,
                item.span,
                d.name.span,
                Vec::new(),
            )),
            ItemKind::Struct(s) => out.push(symbol(
                map,
                &s.name.text,
                None,
                SymbolKind::STRUCT,
                item.span,
                s.name.span,
                Vec::new(),
            )),
            ItemKind::Enum(e) => out.push(symbol(
                map,
                &e.name.text,
                None,
                SymbolKind::ENUM,
                item.span,
                e.name.span,
                Vec::new(),
            )),
            ItemKind::Const(c) => out.push(symbol(
                map,
                &c.name.text,
                None,
                SymbolKind::CONSTANT,
                item.span,
                c.name.span,
                Vec::new(),
            )),
            ItemKind::Fn(f) => out.push(symbol(
                map,
                &f.name.text,
                None,
                SymbolKind::FUNCTION,
                item.span,
                f.name.span,
                Vec::new(),
            )),
            _ => {}
        }
    }
    out
}

/// Modül çocukları: portlar (in/out ayrı ikonla), register'lar,
/// wire'lar, yerel bağlar ve örnekler.
fn module_children(analysis: &Analysis, m: &ModuleDecl) -> Vec<DocumentSymbol> {
    let map = &analysis.map;
    let mut children = Vec::new();
    for port in &m.ports {
        // Outline'da in/out ayrımı ikonla: girişler PROPERTY, çıkışlar
        // FIELD, inout INTERFACE.
        let (kind, detail) = match port.direction {
            PortDir::In => (SymbolKind::PROPERTY, "in"),
            PortDir::Out => (SymbolKind::FIELD, "out"),
            PortDir::InOut => (SymbolKind::INTERFACE, "inout"),
        };
        children.push(symbol(
            map,
            &port.name.text,
            Some(detail.to_string()),
            kind,
            port.span,
            port.name.span,
            Vec::new(),
        ));
    }
    for &stmt_idx in &m.body {
        let stmt = &analysis.ast.stmts[stmt_idx];
        match &stmt.kind {
            StmtKind::Reg(r) => children.push(symbol(
                map,
                &r.name.text,
                Some("reg".to_string()),
                SymbolKind::VARIABLE,
                stmt.span,
                r.name.span,
                Vec::new(),
            )),
            StmtKind::Wire(w) => children.push(symbol(
                map,
                &w.name.text,
                Some("wire".to_string()),
                SymbolKind::VARIABLE,
                stmt.span,
                w.name.span,
                Vec::new(),
            )),
            StmtKind::Let(l) => children.push(symbol(
                map,
                &l.name.text,
                Some("let".to_string()),
                SymbolKind::VARIABLE,
                stmt.span,
                l.name.span,
                Vec::new(),
            )),
            StmtKind::Instance(inst) => children.push(symbol(
                map,
                &inst.name.text,
                Some(
                    inst.module_path
                        .segments
                        .last()
                        .map(|s| s.text.clone())
                        .unwrap_or_default(),
                ),
                SymbolKind::OBJECT,
                stmt.span,
                inst.name.span,
                Vec::new(),
            )),
            _ => {}
        }
    }
    children
}
