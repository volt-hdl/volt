//! Modül seviyesi `let p = P { a: x, b: y }` yeniden sınıflandırması
//! (ADR-0077).
//!
//! `let ad = Ad { ... }` sözdizimi örnekleme ile struct literalini ayırt
//! etmez (grammar-full.ebnf §19 [N3]); parser onu örnekleme olarak
//! okur. Birimin bütün öğeleri okunduktan sonra hedef adı bir modül,
//! extern ya da yerleşik primitif değil de düz bir `struct` ise deyim
//! tipsiz bir `let`'e çevrilir: değeri struct literalidir. Böylece tip
//! denetimi, sürücü analizi ve SV üretimi onu blok içi `let p = P {..}`
//! ile aynı yoldan görür (bugüne dek tipsiz kalıyor, emit'te E0003
//! alıyordu).

use volt_ast::builtin::BuiltinPrim;
use volt_ast::{Expr, ExprKind, FieldInit, ItemKind, LetDecl, StmtKind};
use volt_span::Span;

use super::Parser;

impl Parser<'_> {
    pub(crate) fn reclassify_struct_literals(&mut self) {
        let plain_structs: Vec<String> = self
            .ast
            .items
            .iter()
            .filter_map(|&i| match &self.ast.items_arena[i].kind {
                ItemKind::Struct(s) if !s.is_port => Some(s.name.text.clone()),
                _ => None,
            })
            .collect();
        if plain_structs.is_empty() {
            return;
        }
        let modules: Vec<String> = self
            .ast
            .items
            .iter()
            .filter_map(|&i| match &self.ast.items_arena[i].kind {
                ItemKind::Module(m) => Some(m.name.text.clone()),
                ItemKind::Extern(x) => Some(x.name.text.clone()),
                _ => None,
            })
            .collect();
        let bodies: Vec<_> = self
            .ast
            .items
            .iter()
            .filter_map(|&i| match &self.ast.items_arena[i].kind {
                ItemKind::Module(m) => Some(m.body.clone()),
                _ => None,
            })
            .collect();
        for stmt in bodies.into_iter().flatten() {
            let span = self.ast.stmts[stmt].span;
            let StmtKind::Instance(inst) = &self.ast.stmts[stmt].kind else {
                continue;
            };
            let [seg] = inst.module_path.segments.as_slice() else {
                continue;
            };
            let target = seg.text.as_str();
            if !inst.generic_args.is_empty()
                || !plain_structs.iter().any(|s| s == target)
                || modules.iter().any(|m| m == target)
                || BuiltinPrim::from_name(target).is_some()
            {
                continue;
            }
            let fields: Vec<FieldInit> = inst
                .bindings
                .iter()
                .map(|b| FieldInit {
                    span: b.span,
                    name: b.port_name.clone(),
                    value: b.value,
                })
                .collect();
            let path = inst.module_path.clone();
            let name = inst.name.clone();
            let lit_span = Span {
                start: path.span.start,
                ..span
            };
            let value = self.ast.exprs.alloc(Expr {
                span: lit_span,
                kind: ExprKind::StructLit { path, fields },
            });
            self.ast.stmts[stmt].kind = StmtKind::Let(LetDecl {
                name,
                ty: None,
                value,
            });
        }
    }
}
