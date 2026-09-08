//! Go to definition: sinyal → bildirim, modül örneği yolu → modül
//! tanımı, domain referansı → domain bildirimi.

use volt_ast::{ItemKind, StmtKind};
use volt_span::Span;

use crate::analysis::Analysis;

/// Offsetteki referansın tanım span'i (bildirimdeki isim konumu).
pub fn definition(analysis: &Analysis, offset: u32) -> Option<Span> {
    let res = analysis.resolve.as_ref()?;

    if let Some(def) = analysis.def_at(offset) {
        let span = res.defs[def.0 as usize].span;
        // Prelude yerleşiklerinin (sync, clog2, stdlib) kaynak konumu
        // yoktur — boş span'e gidilmez.
        if span.is_empty() {
            return None;
        }
        return Some(span);
    }

    // `let u = Uart { .. }` içindeki Uart yolu resolve haritalarında
    // ayrı bir referans olarak durmaz; instance_module'den bulunur.
    for &item_idx in &analysis.ast.items {
        let ItemKind::Module(m) = &analysis.ast.items_arena[item_idx].kind else {
            continue;
        };
        for &stmt_idx in &m.body {
            let StmtKind::Instance(inst) = &analysis.ast.stmts[stmt_idx].kind else {
                continue;
            };
            let path = &inst.module_path;
            if !(path.span.start <= offset && offset < path.span.end) {
                continue;
            }
            let inst_def = res.decl_spans.get(&inst.name.span)?;
            let target = res.instance_module.get(inst_def)?;
            let span = res.defs[target.0 as usize].span;
            return (!span.is_empty()).then_some(span);
        }
    }
    None
}
