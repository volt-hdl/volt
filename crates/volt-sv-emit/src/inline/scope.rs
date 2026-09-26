//! fn gövdesinin ad bağlamaları (ADR-0081 Karar 12.1 — hijyen).
//!
//! Açılım adla değil çözümle yapılır: gövdedeki bir ad, çağıran modülün
//! kapsamında değil fn'in kapsamında çözülür. fn kapsamının kuralı
//! volt-hir'in `resolve_fn_body`'siyle aynıdır: ebeveyni kök kapsamdır
//! (modül sinyalleri görünmez); içinde parametreler ve sırayla bildirilen
//! `let`'ler (sonraki aynı ad öncekini gölgeler) vardır; geri kalan her
//! ad kök öğedir (const, enum varyantı, fn). Emitter çözüm tablosunu
//! görmediği için bu kural burada, gövde bir kez gezilerek kurulur.

use std::collections::HashMap;

use volt_ast::visit::expr_children;
use volt_ast::{BlockStmt, Expr, ExprKind, FnDecl, Idx, LetDecl, SourceFile};
use volt_span::Span;

/// Gövdedeki bir adın bağlandığı tanım.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Binding {
    Param(usize),
    Let(usize),
    /// Kök öğe (const, enum varyantı, fn): adı olduğu gibi kalır.
    Global,
}

/// Açılıma hazır fn: gövdenin bağlamaları ve kullanım sayıları.
pub(super) struct FnInfo<'a> {
    pub(super) name: String,
    pub(super) decl: &'a FnDecl,
    pub(super) item_span: Span,
    pub(super) lets: Vec<&'a LetDecl>,
    /// Tek segmentli yol ifadesi → bağlama.
    pub(super) bindings: HashMap<Idx<Expr>, Binding>,
    /// `P { a }` kısa alanı: (struct literali, alan sırası) → bağlama.
    pub(super) shorthand: HashMap<(Idx<Expr>, usize), Binding>,
    /// Parametre bir seçim/alan tabanı olarak kullanılıyor (`p[3:0]`,
    /// `p.a`): yerine SV'de seçilemeyen bir ifade yazılamaz.
    pub(super) param_select_base: Vec<bool>,
    /// `let` bir bit/aralık seçiminin tabanı (`s[8]`): ikame kipinde
    /// yerine seçilemeyen bir ifade yazılamaz.
    pub(super) let_select_base: Vec<bool>,
}

impl<'a> FnInfo<'a> {
    pub(super) fn new(ast: &'a SourceFile, decl: &'a FnDecl, item_span: Span) -> Self {
        let block = &ast.blocks[decl.body];
        let lets: Vec<&LetDecl> = block
            .stmts
            .iter()
            .filter_map(|s| match s {
                BlockStmt::Let(l) => Some(l),
                _ => None,
            })
            .collect();
        let mut info = FnInfo {
            name: decl.name.text.clone(),
            decl,
            item_span,
            bindings: HashMap::new(),
            shorthand: HashMap::new(),
            param_select_base: vec![false; decl.params.len()],
            let_select_base: vec![false; lets.len()],
            lets: Vec::new(),
        };
        let mut scope: HashMap<String, Binding> = decl
            .params
            .iter()
            .enumerate()
            .map(|(i, p)| (p.name.text.clone(), Binding::Param(i)))
            .collect();
        for (j, l) in lets.iter().enumerate() {
            info.bind(ast, l.value, &scope);
            scope.insert(l.name.text.clone(), Binding::Let(j));
        }
        if let Some(tail) = block.tail {
            info.bind(ast, tail, &scope);
        }
        info.lets = lets;
        info
    }

    fn bind(&mut self, ast: &SourceFile, root: Idx<Expr>, scope: &HashMap<String, Binding>) {
        let mut stack = vec![root];
        while let Some(e) = stack.pop() {
            match &ast.exprs[e].kind {
                ExprKind::Path(p) if p.segments.len() == 1 => {
                    let b = scope
                        .get(&p.segments[0].text)
                        .copied()
                        .unwrap_or(Binding::Global);
                    self.bindings.insert(e, b);
                }
                ExprKind::StructLit { fields, .. } => {
                    for (i, f) in fields.iter().enumerate() {
                        if f.value.is_none() {
                            let b = scope.get(&f.name.text).copied().unwrap_or(Binding::Global);
                            self.shorthand.insert((e, i), b);
                        }
                    }
                }
                ExprKind::Index { base, .. }
                | ExprKind::Range { base, .. }
                | ExprKind::PartSelect { base, .. }
                | ExprKind::Field { base, .. } => {
                    let is_field = matches!(ast.exprs[e].kind, ExprKind::Field { .. });
                    if let ExprKind::Path(p) = &ast.exprs[*base].kind {
                        let bound = p.segments.first().and_then(|s| scope.get(&s.text)).copied();
                        match bound {
                            Some(Binding::Param(i)) if p.segments.len() == 1 => {
                                self.param_select_base[i] = true;
                            }
                            // Struct alanı (`s.a`) struct indirgemesiyle
                            // yaprağa iner; yalnız bit seçimi sorun.
                            Some(Binding::Let(j)) if p.segments.len() == 1 && !is_field => {
                                self.let_select_base[j] = true;
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
            stack.extend(expr_children(&ast.exprs[e].kind));
        }
    }

    /// Gövdenin sonucu (son ifade) — E2015'li fn açılmaz.
    pub(super) fn tail(&self, ast: &SourceFile) -> Option<Idx<Expr>> {
        ast.blocks[self.decl.body].tail
    }

    /// Canlı `let`'ler ve parametreler: son ifadeden (dolaylı `let`
    /// başvuruları dahil) ulaşılanlar. Kullanılmayan `let` ya da parametre
    /// için tel üretilmez (Verilator UNUSEDSIGNAL; fn saf olduğundan
    /// okunmayan değerin anlamı yoktur).
    pub(super) fn liveness(&self, ast: &SourceFile) -> (Vec<bool>, Vec<bool>) {
        let mut lets = vec![false; self.lets.len()];
        let mut params = vec![false; self.decl.params.len()];
        let mut work: Vec<Idx<Expr>> = self.tail(ast).into_iter().collect();
        let mut mark = |b: Option<&Binding>, work: &mut Vec<Idx<Expr>>| match b {
            Some(Binding::Let(j)) if !lets[*j] => {
                lets[*j] = true;
                work.push(self.lets[*j].value);
            }
            Some(Binding::Param(i)) => params[*i] = true,
            _ => {}
        };
        while let Some(root) = work.pop() {
            let mut stack = vec![root];
            while let Some(e) = stack.pop() {
                mark(self.bindings.get(&e), &mut work);
                if let ExprKind::StructLit { fields, .. } = &ast.exprs[e].kind {
                    for i in 0..fields.len() {
                        mark(self.shorthand.get(&(e, i)), &mut work);
                    }
                }
                stack.extend(expr_children(&ast.exprs[e].kind));
            }
        }
        (lets, params)
    }
}
