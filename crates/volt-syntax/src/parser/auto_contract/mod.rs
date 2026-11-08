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

/// İfadenin kaynak biçimi (ad, tam sayı, ikili/tekli işlem, çağrı);
/// başka biçimde `None`. Tanı önerileri de kullanır (ADR-0106).
pub(crate) fn expr_text(ast: &SourceFile, e: Idx<Expr>) -> Option<String> {
    print::expr_with_selects(ast, e)
}
mod scan;

use std::collections::HashSet;

use volt_ast::{enum_layout, ContractKind, Expr, ExprKind, Idx, ItemKind, SourceFile, UnOp};

use super::handshake::has_attr;
use super::mono::scope::Scopes;
use super::mono::unroll::{collect_consts, visible_const, Consts};
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
        let Some(scan) = scan::scan_module(ast, &consts, item, m) else {
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

/// Sabit değer: `eval_const` + enum varyant yolu (`State::Run` → kodu,
/// ADR-0074) ve enum tipli `const` (`START` → `State::Idle`).
pub(super) fn value_of(ast: &SourceFile, consts: &Consts, e: Idx<Expr>) -> Option<i128> {
    const MAX_DEPTH: u32 = 64;
    let mut cur = e;
    for _ in 0..MAX_DEPTH {
        match &ast.exprs[cur].kind {
            ExprKind::Path(p) if p.segments.len() >= 2 => {
                let (decl, idx) = enum_variant(ast, p)?;
                let layout = enum_layout::valid_layout(ast, decl, &mut |x| {
                    super::mono::unroll::eval_const(ast, consts, x, 0)
                })?;
                return i128::try_from(layout.values[idx]).ok();
            }
            ExprKind::Path(p) if p.segments.len() == 1 => {
                match visible_const(ast, consts, cur, &p.segments[0].text) {
                    Some(v) if matches!(&ast.exprs[v].kind, ExprKind::Path(q) if q.segments.len() >= 2) =>
                    {
                        cur = v;
                    }
                    _ => return super::mono::unroll::eval_const(ast, consts, cur, 0),
                }
            }
            _ => return super::mono::unroll::eval_const(ast, consts, cur, 0),
        }
    }
    None
}

/// `A::B` yolunun adlandırdığı enum bildirimi ve varyant sırası.
pub(super) fn enum_variant<'a>(
    ast: &'a SourceFile,
    p: &volt_ast::Path,
) -> Option<(&'a volt_ast::EnumDecl, usize)> {
    let n = p.segments.len();
    let decl = enum_layout::enum_named(ast, &p.segments[n.checked_sub(2)?].text)?;
    let idx = decl
        .variants
        .iter()
        .position(|v| v.name.text == p.segments[n - 1].text)?;
    Some((decl, idx))
}

/// `width` bitlik işaretsiz tipin en büyük değeri (width ≤ 64).
pub(super) fn max_value(width: u32) -> i128 {
    (1i128 << width) - 1
}

/// Derleme zamanı sabiti mi (literal, kök kapsamın adı, aritmetik)?
/// Değer `eval_const` ile ayrıca hesaplanır; bu süzgeç, ifadenin
/// bulunduğu yerin kapsam zincirinde (`scope`, #138) kök const'u gölgeleyen
/// adları dışlar: port, reg, modül `let`'i/`wire`'ı/örneği, blok `let`'i,
/// döngü değişkeni (name-resolution.md §2, §6).
pub(super) fn is_const_expr(ast: &SourceFile, scope: &Scopes, e: Idx<Expr>) -> bool {
    match &ast.exprs[e].kind {
        ExprKind::IntLit { .. } => true,
        ExprKind::Path(p) if p.segments.len() >= 2 => enum_variant(ast, p).is_some(),
        ExprKind::Path(p) => p.segments.len() == 1 && !scope.bound_above_root(&p.segments[0].text),
        ExprKind::Binary { lhs, rhs, .. } => {
            is_const_expr(ast, scope, *lhs) && is_const_expr(ast, scope, *rhs)
        }
        ExprKind::Unary {
            op: UnOp::Neg,
            operand,
        } => is_const_expr(ast, scope, *operand),
        _ => false,
    }
}
