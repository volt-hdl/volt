//! Kontrat denetimi (F4a): her kontrat ifadesi Bool olmalı (E5004) ve
//! kontrat türünün kapsamı dışındaki sinyallere erişemez (E1001).

use volt_ast::{
    ArrayLitKind, Contract, ContractKind, Expr, ExprKind, Idx, MatchArmBody, SourceFile,
};
use volt_diagnostics::{lstr, ErrorCode};

use super::TypeChecker;
use crate::resolve::DefKind;
use crate::ty::Ty;

impl TypeChecker<'_, '_> {
    /// Her kontrat ifadesi Bool olmalı (E5004); tür bazlı kapsam:
    /// requires/ensures → port, invariant → port + register,
    /// cover/assert/assume → hepsi (ihlal E1001).
    pub(super) fn check_contract(&mut self, c: &Contract) {
        let ty = self.synth(c.expr);
        if !self.types.is_error(ty) && !matches!(self.types.ty(ty), Ty::Bool) {
            let shown = self.show(ty);
            let span = self.ast.exprs[c.expr].span;
            self.error(
                ErrorCode::E5004,
                span,
                lstr!(en: "contract expression is not Bool"; tr: "kontrat ifadesi Bool değil"),
                lstr!(en: "this expression has type '{shown}'"; tr: "bu ifadenin tipi '{shown}'"),
                lstr!(
                    en: "write a condition such as a comparison (x <= 2) or a bool signal";
                    tr: "karşılaştırma (x <= 2) ya da bool sinyal gibi bir koşul yazın"
                ),
            );
        }
        self.check_contract_scope(c);
    }

    fn check_contract_scope(&mut self, c: &Contract) {
        if matches!(
            c.kind,
            ContractKind::Cover | ContractKind::Assert | ContractKind::Assume
        ) {
            return; // her sinyale erişebilir
        }
        let mut paths = Vec::new();
        collect_path_exprs(self.ast, c.expr, &mut paths);
        for p in paths {
            // Çözülemeyen isim E1001'i isim çözümlemede zaten aldı.
            let Some(&def) = self.res.resolutions.get(&p) else {
                continue;
            };
            let out_of_scope = match self.res.def_kind(def) {
                DefKind::Register => c.kind != ContractKind::Invariant,
                DefKind::Wire | DefKind::LocalBinding | DefKind::Instance => true,
                // Port, const, enum varyantı vb. her kontratta serbest.
                _ => false,
            };
            if out_of_scope {
                self.err_contract_scope(c.kind, p);
            }
        }
    }

    /// E1001 — `path` bu kontrat türünün kapsamı dışında.
    fn err_contract_scope(&mut self, kind: ContractKind, path: Idx<Expr>) {
        let kw = contract_keyword(kind);
        let name = match &self.ast.exprs[path].kind {
            ExprKind::Path(path) => path
                .segments
                .last()
                .map(|n| n.text.clone())
                .unwrap_or_default(),
            _ => String::new(),
        };
        let span = self.ast.exprs[path].span;
        let help = match kind {
            ContractKind::Invariant => lstr!(
                en: "'invariant' may only reference module ports and registers";
                tr: "'invariant' yalnız modül portlarına ve register'lara erişebilir"
            ),
            _ => lstr!(
                en: "'requires' and 'ensures' may only reference module ports";
                tr: "'requires' ve 'ensures' yalnız modül portlarına erişebilir"
            ),
        };
        self.error(
            ErrorCode::E1001,
            span,
            lstr!(
                en: "'{name}' cannot be referenced in a '{kw}' contract";
                tr: "'{name}' bir '{kw}' kontratında kullanılamaz"
            ),
            lstr!(
                en: "out of scope for this contract kind";
                tr: "bu kontrat türünün kapsamı dışında"
            ),
            help,
        );
    }
}

/// Kontrat kapsam denetimi için ifade ağacındaki Path düğümlerini
/// kaynak sırasıyla (önce-kök) toplar.
fn collect_path_exprs(ast: &SourceFile, expr: Idx<Expr>, out: &mut Vec<Idx<Expr>>) {
    let kind = &ast.exprs[expr].kind;
    if matches!(kind, ExprKind::Path(_)) {
        out.push(expr);
        return;
    }
    for child in child_exprs(kind) {
        collect_path_exprs(ast, child, out);
    }
}

/// İfadenin doğrudan alt ifadeleri, kaynak sırasıyla. Blok gövdeli
/// match kolları ifade taşımaz (yalnız koruma ifadesi sayılır).
fn child_exprs(kind: &ExprKind) -> Vec<Idx<Expr>> {
    match kind {
        ExprKind::Binary { lhs, rhs, .. } => vec![*lhs, *rhs],
        ExprKind::Unary { operand, .. } => vec![*operand],
        ExprKind::Index { base, index } => vec![*base, *index],
        ExprKind::Range { base, hi, lo } => vec![*base, *hi, *lo],
        ExprKind::PartSelect {
            base, start, width, ..
        } => vec![*base, *start, *width],
        ExprKind::Field { base, .. } => vec![*base],
        ExprKind::Call { callee, args } => std::iter::once(*callee)
            .chain(args.iter().copied())
            .collect(),
        ExprKind::Cast { expr: inner, .. } => vec![*inner],
        ExprKind::If {
            cond,
            then_expr,
            else_expr,
        } => vec![*cond, *then_expr, *else_expr],
        ExprKind::Match { scrutinee, arms } => {
            let mut children = vec![*scrutinee];
            for arm in arms {
                children.extend(arm.guard);
                if let MatchArmBody::Expr(e) = &arm.body {
                    children.push(*e);
                }
            }
            children
        }
        ExprKind::StructLit { fields, .. } => fields.iter().filter_map(|f| f.value).collect(),
        ExprKind::ArrayLit(ArrayLitKind::List(items)) | ExprKind::TupleLit(items) => items.to_vec(),
        ExprKind::ArrayLit(ArrayLitKind::Repeat { value, count }) => vec![*value, *count],
        ExprKind::Path(_)
        | ExprKind::IntLit { .. }
        | ExprKind::BoolLit(_)
        | ExprKind::StringLit(_)
        | ExprKind::Todo { .. }
        | ExprKind::Concat(_)
        | ExprKind::Error => Vec::new(),
    }
}

/// Tanı metinlerinde kontrat anahtar kelimesi.
fn contract_keyword(kind: ContractKind) -> &'static str {
    match kind {
        ContractKind::Requires => "requires",
        ContractKind::Ensures => "ensures",
        ContractKind::Invariant => "invariant",
        ContractKind::Cover => "cover",
        ContractKind::Assert => "assert",
        ContractKind::Assume => "assume",
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{codes, diagnostics};
    use super::*;

    fn module(contracts: &str) -> String {
        format!(
            "module Uart {{\n    in  clk   : clock\n    in  speed : u8\n    \
             in  start : bool\n    out busy  : bool\n\n{contracts}\n    \
             reg busy_r : bool = false\n\n    on clk {{\n        \
             busy_r <= start\n    }}\n\n    busy = busy_r\n}}\n"
        )
    }

    #[test]
    fn non_bool_contract_is_e5004() {
        assert!(codes(&module("    requires: speed <= 2\n")).is_empty());
        assert!(codes(&module("    requires: speed + 1\n")).contains(&"E5004"));
    }

    #[test]
    fn register_is_out_of_scope_for_requires_but_not_invariant() {
        assert!(codes(&module("    requires: busy_r\n")).contains(&"E1001"));
        assert!(!codes(&module("    invariant: busy_r || !busy_r\n")).contains(&"E1001"));
    }

    #[test]
    fn scope_violations_are_reported_in_source_order() {
        let diags = diagnostics(&module("    ensures: busy_r == (busy_r && start)\n"));
        let spans: Vec<_> = diags
            .iter()
            .filter(|d| d.code.as_str() == "E1001")
            .filter_map(|d| d.primary_span().map(|s| s.span.start))
            .collect();
        assert_eq!(spans.len(), 2);
        assert!(spans[0] < spans[1], "{spans:?}");
    }

    #[test]
    fn contract_keyword_covers_every_kind() {
        assert_eq!(contract_keyword(ContractKind::Requires), "requires");
        assert_eq!(contract_keyword(ContractKind::Assume), "assume");
    }
}
