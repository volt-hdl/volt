//! Otomatik FSM ve sayaç kontratları (ADR-0066).
//!
//! Handshake<T> (ADR-0050) ve `@mmio` (ADR-0044) gibi bu da bir parser
//! desugar'ıdır: tanınan yapılar için modül kontratlarına `invariant` /
//! `cover` eklenir. Kontratlar yalnız SVA'ya ve `volt test` izleyicisine
//! gider; üretilen RTL değişmez. Tanıma SÖZDİZİMSELDİR ve muhafazakârdır:
//! bir register'ın herhangi bir yazması tanınan biçimin dışındaysa o
//! register için kontrat üretilmez (az ve doğru, çok ve gürültülü değil).
//!
//! Birim sonu desugar zincirinin (mono, `for` açılımı, bundle, çift yönlü
//! port) SONUNDA koşar: somut modülleri ve `@mmio` gövdesini görür.
//! Kontratlar ilk saat portunda örneklendiğinden yalnız o saatin `on`
//! bloklarında yazılan register'lar aday olur.
//!
//! Kapatma: modül ya da `reg` deyimi üzerinde `@no_auto_contracts`.

mod counter;
mod fsm;
mod gen;
mod print;
mod scan;

use std::collections::HashSet;

use volt_ast::{ContractKind, Expr, ExprKind, Idx, ItemKind, SourceFile, UnOp};

use super::handshake::has_attr;
use super::mono::unroll::collect_consts;
use super::Parser;

/// FSM ve sayaç kontratlarını kapatan nitelik (modül ya da `reg`).
pub(super) const NO_AUTO_CONTRACTS: &str = "no_auto_contracts";

impl Parser<'_> {
    pub(crate) fn add_auto_contracts(&mut self) {
        add_auto_contracts(&mut self.ast);
    }
}

fn add_auto_contracts(ast: &mut SourceFile) {
    let consts = collect_consts(ast);
    let mut used = gen::used_zero_spans(ast);
    let items: Vec<_> = ast.items.clone();
    for item in items {
        let it = &ast.items_arena[item];
        if has_attr(&it.attrs, NO_AUTO_CONTRACTS) {
            continue;
        }
        let ItemKind::Module(m) = &it.kind else {
            continue;
        };
        let Some(scan) = scan::scan_module(ast, &consts, m) else {
            continue;
        };
        let user = user_contracts(ast, m);
        let mut specs = Vec::new();
        for reg in &scan.regs {
            if reg.opted_out || scan.tainted.contains(&reg.name) {
                continue;
            }
            specs.extend(fsm::specs(ast, &consts, &scan, reg));
            specs.extend(counter::specs(ast, &consts, &scan, reg));
        }
        if specs.is_empty() {
            continue;
        }
        let mut spans = gen::Spans {
            used: &mut used,
            item: it.span,
            cursor: it.span.start,
        };
        let mut contracts = Vec::new();
        for spec in &specs {
            let Some(c) = gen::build(ast, &mut spans, spec) else {
                continue;
            };
            let text = c.auto.as_ref().map(|a| a.text.clone()).unwrap_or_default();
            // Kullanıcı aynı kontratı zaten yazdıysa tekrar üretilmez.
            if !user.contains(&(c.kind, text)) {
                contracts.push(c);
            }
        }
        if let ItemKind::Module(m) = &mut ast.items_arena[item].kind {
            m.contracts.extend(contracts);
        }
    }
}

/// Kullanıcının elle yazdığı kontratlar (tür, metin) — tekilleştirme.
fn user_contracts(ast: &SourceFile, m: &volt_ast::ModuleDecl) -> HashSet<(ContractKind, String)> {
    m.contracts
        .iter()
        .filter(|c| c.auto.is_none())
        .filter_map(|c| Some((c.kind, print::expr(ast, c.expr)?)))
        .collect()
}

/// `width` bitlik işaretsiz tipin en büyük değeri (width ≤ 64).
pub(super) fn max_value(width: u32) -> i128 {
    (1i128 << width) - 1
}

/// Derleme zamanı sabiti mi (literal, modül içi olmayan ad, aritmetik)?
/// Değer `eval_const` ile ayrıca hesaplanır; bu süzgeç yerel adların
/// (port, reg, let) aynı adlı `const`'u gölgelemesini dışlar.
pub(super) fn is_const_expr(ast: &SourceFile, locals: &HashSet<String>, e: Idx<Expr>) -> bool {
    match &ast.exprs[e].kind {
        ExprKind::IntLit { .. } => true,
        ExprKind::Path(p) => p.segments.len() == 1 && !locals.contains(&p.segments[0].text),
        ExprKind::Binary { lhs, rhs, .. } => {
            is_const_expr(ast, locals, *lhs) && is_const_expr(ast, locals, *rhs)
        }
        ExprKind::Unary {
            op: UnOp::Neg,
            operand,
        } => is_const_expr(ast, locals, *operand),
        _ => false,
    }
}
