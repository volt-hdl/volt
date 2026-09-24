//! Struct değerli ifadeler: tip çıkarımı, yaprağa izdüşüm ve yerinde
//! yeniden yazma (ADR-0077 Karar 5 kuralları 5–12).

use volt_ast::{
    BinOp, Block, BlockStmt, ElseBranch, Expr, ExprKind, Idx, IfStmt, ItemKind, MatchArmBody,
    ModuleDecl, Name, NumBase, Path, SourceFile, Stmt, StmtKind, UnOp,
};

use super::Lowerer;

/// Sabit tamsayı (yaprak genişlikleri için): literal, `const` adı,
/// aritmetik. Monomorfizasyon generic genişlikleri literale indirdi.
pub(super) fn const_int(ast: &SourceFile, e: Idx<Expr>, depth: u32) -> Option<i128> {
    if depth > 32 {
        return None;
    }
    match &ast.exprs[e].kind {
        ExprKind::IntLit { value, .. } => i128::try_from(*value).ok(),
        ExprKind::Path(p) if p.segments.len() == 1 => {
            let name = &p.segments[0].text;
            ast.items
                .iter()
                .find_map(|&i| match &ast.items_arena[i].kind {
                    ItemKind::Const(c) if c.name.text == *name => {
                        const_int(ast, c.value, depth + 1)
                    }
                    _ => None,
                })
        }
        ExprKind::Unary {
            op: UnOp::Neg,
            operand,
        } => const_int(ast, *operand, depth + 1).map(|v| -v),
        ExprKind::Binary { op, lhs, rhs } => {
            let (l, r) = (
                const_int(ast, *lhs, depth + 1)?,
                const_int(ast, *rhs, depth + 1)?,
            );
            match op {
                BinOp::Add => l.checked_add(r),
                BinOp::Sub => l.checked_sub(r),
                BinOp::Mul => l.checked_mul(r),
                BinOp::Div => l.checked_div(r),
                BinOp::Rem => l.checked_rem(r),
                BinOp::Shl => u32::try_from(r).ok().and_then(|r| l.checked_shl(r)),
                BinOp::Shr => u32::try_from(r).ok().and_then(|r| l.checked_shr(r)),
                _ => None,
            }
        }
        _ => None,
    }
}

/// İfadenin doğrudan çocukları.
pub(super) fn children(kind: &ExprKind) -> Vec<Idx<Expr>> {
    match kind {
        ExprKind::Binary { lhs, rhs, .. } => vec![*lhs, *rhs],
        ExprKind::Unary { operand, .. } => vec![*operand],
        ExprKind::Index { base, index } => vec![*base, *index],
        ExprKind::Range { base, hi, lo } => vec![*base, *hi, *lo],
        ExprKind::PartSelect {
            base, start, width, ..
        } => vec![*base, *start, *width],
        ExprKind::Field { base, .. } => vec![*base],
        ExprKind::Call { callee, args } => {
            let mut v = vec![*callee];
            v.extend(args.iter().copied());
            v
        }
        ExprKind::Cast { expr, .. } => vec![*expr],
        ExprKind::If {
            cond,
            then_expr,
            else_expr,
        } => vec![*cond, *then_expr, *else_expr],
        ExprKind::Match { scrutinee, arms } => {
            let mut v = vec![*scrutinee];
            for a in arms {
                v.extend(a.guard);
                if let MatchArmBody::Expr(e) = a.body {
                    v.push(e);
                }
            }
            v
        }
        ExprKind::StructLit { fields, .. } => fields.iter().filter_map(|f| f.value).collect(),
        ExprKind::ArrayLit(volt_ast::ArrayLitKind::List(items)) | ExprKind::TupleLit(items) => {
            items.clone()
        }
        ExprKind::ArrayLit(volt_ast::ArrayLitKind::Repeat { value, count }) => {
            vec![*value, *count]
        }
        ExprKind::Concat(parts) => parts.iter().map(|&(p, _)| p).collect(),
        ExprKind::IntLit { .. }
        | ExprKind::BoolLit(_)
        | ExprKind::StringLit(_)
        | ExprKind::Path(_)
        | ExprKind::Todo { .. }
        | ExprKind::Error => Vec::new(),
    }
}

/// İfade ağacının her düğümü (önce düğüm, sonra çocuklar).
pub(super) fn walk_expr(ast: &SourceFile, e: Idx<Expr>, f: &mut dyn FnMut(&SourceFile, Idx<Expr>)) {
    f(ast, e);
    for c in children(&ast.exprs[e].kind) {
        walk_expr(ast, c, f);
    }
}

fn walk_block(ast: &SourceFile, b: Idx<Block>, f: &mut dyn FnMut(&SourceFile, Idx<Expr>)) {
    for s in &ast.blocks[b].stmts {
        walk_block_stmt(ast, s, f);
    }
    if let Some(t) = ast.blocks[b].tail {
        walk_expr(ast, t, f);
    }
}

fn walk_lvalue(ast: &SourceFile, lv: &volt_ast::LValue, f: &mut dyn FnMut(&SourceFile, Idx<Expr>)) {
    for s in &lv.suffixes {
        match s {
            volt_ast::LValueSuffix::Index(e) => walk_expr(ast, *e, f),
            volt_ast::LValueSuffix::Range { hi, lo } => {
                walk_expr(ast, *hi, f);
                walk_expr(ast, *lo, f);
            }
            volt_ast::LValueSuffix::PartSelect { start, width, .. } => {
                walk_expr(ast, *start, f);
                walk_expr(ast, *width, f);
            }
            volt_ast::LValueSuffix::Field(_) => {}
        }
    }
}

fn walk_if(ast: &SourceFile, i: &IfStmt, f: &mut dyn FnMut(&SourceFile, Idx<Expr>)) {
    walk_expr(ast, i.cond, f);
    walk_block(ast, i.then_block, f);
    match &i.else_branch {
        Some(ElseBranch::Block(b)) => walk_block(ast, *b, f),
        Some(ElseBranch::If(n)) => walk_if(ast, n, f),
        None => {}
    }
}

fn walk_block_stmt(ast: &SourceFile, s: &BlockStmt, f: &mut dyn FnMut(&SourceFile, Idx<Expr>)) {
    match s {
        BlockStmt::NonBlockAssign { lhs, rhs, .. } | BlockStmt::BlockAssign { lhs, rhs, .. } => {
            walk_lvalue(ast, lhs, f);
            walk_expr(ast, *rhs, f);
        }
        BlockStmt::If(i) => walk_if(ast, i, f),
        BlockStmt::Match(m) => {
            walk_expr(ast, m.scrutinee, f);
            for a in &m.arms {
                if let Some(g) = a.guard {
                    walk_expr(ast, g, f);
                }
                match a.body {
                    MatchArmBody::Block(b) => walk_block(ast, b, f),
                    MatchArmBody::Expr(e) => walk_expr(ast, e, f),
                }
            }
        }
        BlockStmt::Let(l) => walk_expr(ast, l.value, f),
        BlockStmt::For(fr) => {
            walk_expr(ast, fr.start, f);
            walk_expr(ast, fr.end, f);
            walk_block(ast, fr.body, f);
        }
        BlockStmt::Error => {}
    }
}

/// Modül seviyesi bir deyimin bütün ifadeleri (atama hedefi adları hariç).
pub(super) fn walk_stmt_exprs(
    ast: &SourceFile,
    s: Idx<Stmt>,
    f: &mut dyn FnMut(&SourceFile, Idx<Expr>),
) {
    match &ast.stmts[s].kind {
        StmtKind::Reg(r) => walk_expr(ast, r.init, f),
        StmtKind::Let(l) => walk_expr(ast, l.value, f),
        StmtKind::Instance(i) => {
            for b in &i.bindings {
                if let Some(v) = b.value {
                    walk_expr(ast, v, f);
                }
            }
        }
        StmtKind::On(on) => walk_block(ast, on.body, f),
        StmtKind::Comb(b) => walk_block(ast, *b, f),
        StmtKind::Assign(a) => {
            walk_lvalue(ast, &a.lhs, f);
            walk_expr(ast, a.rhs, f);
        }
        StmtKind::For(fr) => {
            walk_expr(ast, fr.start, f);
            walk_expr(ast, fr.end, f);
            walk_block(ast, fr.body, f);
        }
        StmtKind::Expr(e) => walk_expr(ast, *e, f),
        StmtKind::Wire(_) | StmtKind::Error => {}
    }
}

/// Modülün gövde ve kontrat ifadeleri.
pub(super) fn walk_module_exprs(
    ast: &SourceFile,
    m: &ModuleDecl,
    f: &mut dyn FnMut(&SourceFile, Idx<Expr>),
) {
    for &s in &m.body {
        walk_stmt_exprs(ast, s, f);
    }
    for c in &m.contracts {
        walk_expr(ast, c.expr, f);
    }
}

fn callee_name(ast: &SourceFile, callee: Idx<Expr>) -> Option<&str> {
    match &ast.exprs[callee].kind {
        ExprKind::Path(p) if p.segments.len() == 1 => Some(p.segments[0].text.as_str()),
        _ => None,
    }
}

impl Lowerer {
    /// Taban bir kullanıcı modülü örneğiyse hedef modülün adı.
    fn instance_target(&self, base: Idx<Expr>) -> Option<&String> {
        match &self.ast.exprs[base].kind {
            ExprKind::Path(p) if p.segments.len() == 1 => self.env.insts.get(&p.segments[0].text),
            _ => None,
        }
    }

    /// İfadenin struct tipi (adı); struct değilse `None`. Tip denetimi
    /// geçmiştir — yalnız struct değerli biçimler tanınır.
    pub(super) fn struct_of_expr(&self, e: Idx<Expr>) -> Option<String> {
        match &self.ast.exprs[e].kind {
            ExprKind::Path(p) if p.segments.len() == 1 => {
                let n = &p.segments[0].text;
                self.env
                    .sigs
                    .get(n)
                    .cloned()
                    .or_else(|| self.consts.get(n).map(|(s, _)| s.clone()))
            }
            ExprKind::Field { base, field } => {
                if let Some(target) = self.instance_target(*base) {
                    return self.module_ports.get(target)?.get(&field.text).cloned();
                }
                let s = self.struct_of_expr(*base)?;
                let decl = volt_ast::struct_layout::struct_named(&self.ast, &s)?;
                let f = decl.fields.iter().find(|f| f.name.text == field.text)?;
                self.struct_of_type(f.ty)
            }
            ExprKind::StructLit { path, .. } => {
                let name = &path.segments.last()?.text;
                self.layout(name).map(|_| name.clone())
            }
            ExprKind::Cast { ty, .. } => self.struct_of_type(*ty),
            ExprKind::If {
                then_expr,
                else_expr,
                ..
            } => self
                .struct_of_expr(*then_expr)
                .or_else(|| self.struct_of_expr(*else_expr)),
            ExprKind::Call { callee, args } => match callee_name(&self.ast, *callee) {
                Some("prev" | "sync" | "sync3") => self.struct_of_expr(*args.first()?),
                _ => None,
            },
            _ => None,
        }
    }

    fn alloc(&mut self, kind: ExprKind, span: volt_span::Span) -> Idx<Expr> {
        self.ast.exprs.alloc(Expr { span, kind })
    }

    fn path_expr(&mut self, text: String, span: volt_span::Span) -> Idx<Expr> {
        self.alloc(
            ExprKind::Path(Path {
                span,
                segments: vec![Name { text, span }],
            }),
            span,
        )
    }

    fn int_lit(&mut self, value: u32, span: volt_span::Span) -> Idx<Expr> {
        self.alloc(
            ExprKind::IntLit {
                value: u128::from(value),
                suffix: None,
                base: NumBase::Dec,
            },
            span,
        )
    }

    /// Struct değerli `e`'nin `sub` alan yolundaki parçası (kural 5–12).
    /// Boş yol ifadenin kendisidir. SV karşılığı olmayan biçimde `None`.
    pub(super) fn project(&mut self, e: Idx<Expr>, sub: &[String]) -> Option<Idx<Expr>> {
        if sub.is_empty() {
            return Some(e);
        }
        let span = self.ast.exprs[e].span;
        match self.ast.exprs[e].kind.clone() {
            ExprKind::Path(p) if p.segments.len() == 1 => {
                let n = p.segments[0].text.clone();
                if self.env.sigs.contains_key(&n) {
                    return Some(self.path_expr(Self::leaf_name(&n, sub), span));
                }
                let (_, value) = self.consts.get(&n)?.clone();
                self.project(value, sub)
            }
            ExprKind::Field { base, field } => {
                if self.instance_target(base).is_some() {
                    let field = Name {
                        text: Self::leaf_name(&field.text, sub),
                        span: field.span,
                    };
                    return Some(self.alloc(ExprKind::Field { base, field }, span));
                }
                let mut path = vec![field.text.clone()];
                path.extend(sub.iter().cloned());
                self.project(base, &path)
            }
            ExprKind::StructLit { fields, .. } => {
                let f = fields.iter().find(|f| f.name.text == sub[0])?;
                let v = match f.value {
                    Some(v) => v,
                    None => self.path_expr(f.name.text.clone(), f.name.span),
                };
                self.project(v, &sub[1..])
            }
            ExprKind::Cast { expr: raw, ty } => {
                let s = self.struct_of_type(ty)?;
                let leaf = self
                    .layout(&s)?
                    .leaves
                    .iter()
                    .find(|l| l.path == sub)?
                    .clone();
                // Dilim yalnız adlandırılmış bir değerde yazılabilir (SV'de
                // `(a + b)[3:0]` yok): sinyal, örnek portu, dizi elemanı.
                if !matches!(
                    self.ast.exprs[raw].kind,
                    ExprKind::Path(_) | ExprKind::Field { .. } | ExprKind::Index { .. }
                ) {
                    return None;
                }
                if leaf.width == 1 {
                    let index = self.int_lit(leaf.lsb, span);
                    Some(self.alloc(ExprKind::Index { base: raw, index }, span))
                } else {
                    let hi = self.int_lit(leaf.msb(), span);
                    let lo = self.int_lit(leaf.lsb, span);
                    Some(self.alloc(ExprKind::Range { base: raw, hi, lo }, span))
                }
            }
            ExprKind::If {
                cond,
                then_expr,
                else_expr,
            } => {
                let t = self.project(then_expr, sub)?;
                let e2 = self.project(else_expr, sub)?;
                Some(self.alloc(
                    ExprKind::If {
                        cond,
                        then_expr: t,
                        else_expr: e2,
                    },
                    span,
                ))
            }
            ExprKind::Call { callee, args }
                if matches!(
                    callee_name(&self.ast, callee),
                    Some("prev" | "sync" | "sync3")
                ) =>
            {
                let first = *args.first()?;
                let x = self.project(first, sub)?;
                let mut args = args.clone();
                args[0] = x;
                Some(self.alloc(ExprKind::Call { callee, args }, span))
            }
            _ => None,
        }
    }

    /// Dizi yaprağına verilen dizi literali paketlenmiş vektöre iner
    /// (ADR-0056: eleman 0 en düşük bitlerde): `[a, b]` → `{b, a}`,
    /// `[v; N]` → `{v, …, v}`. Diğer değerler aynen döner.
    pub(super) fn pack_array(
        &mut self,
        v: Idx<Expr>,
        leaf_ty: Idx<volt_ast::TypeRef>,
    ) -> Idx<Expr> {
        let target = crate::alias::resolve(&self.ast, leaf_ty);
        let volt_ast::TypeRefKind::Array { elem, .. } = self.ast.types[target].kind else {
            return v;
        };
        let span = self.ast.exprs[v].span;
        let items: Vec<Idx<Expr>> = match &self.ast.exprs[v].kind {
            ExprKind::ArrayLit(volt_ast::ArrayLitKind::List(items)) => items.clone(),
            ExprKind::ArrayLit(volt_ast::ArrayLitKind::Repeat { value, count }) => {
                let Some(n) = const_int(&self.ast, *count, 0).and_then(|n| usize::try_from(n).ok())
                else {
                    return v;
                };
                vec![*value; n]
            }
            _ => return v,
        };
        let parts = items.into_iter().rev().map(|i| (i, elem)).collect();
        self.alloc(ExprKind::Concat(parts), span)
    }

    /// Bütün struct değerinin yaprak birleştirmesi (MSB'den, Karar 3).
    fn concat_of(&mut self, e: Idx<Expr>, s: &str) -> Option<Idx<Expr>> {
        let leaves = self.layout(s)?.leaves.clone();
        let mut parts = Vec::with_capacity(leaves.len());
        for leaf in &leaves {
            let p = self.project(e, &leaf.path)?;
            let p = self.pack_array(p, leaf.ty);
            self.rewrite_expr(p);
            parts.push((p, leaf.ty));
        }
        let span = self.ast.exprs[e].span;
        Some(self.alloc(ExprKind::Concat(parts), span))
    }

    /// Alan zinciri `r.a.b` → (kök, [a, b]); kök struct değerli bir ifade
    /// ya da struct portlu bir örnek portudur.
    fn field_chain(&self, e: Idx<Expr>) -> Option<(Idx<Expr>, Vec<String>, String)> {
        let mut path = Vec::new();
        let mut cur = e;
        while let ExprKind::Field { base, field } = &self.ast.exprs[cur].kind {
            if self.instance_target(*base).is_some() {
                // `s0.q` kökün kendisi (struct portu).
                break;
            }
            path.push(field.text.clone());
            cur = *base;
        }
        path.reverse();
        let s = self.struct_of_expr(cur)?;
        (!path.is_empty()).then_some((cur, path, s))
    }

    /// İfadeyi yerinde yeniden yazar (çocuklar önce): yaprak alan
    /// erişimi sinyal adına, bütün-struct `==`/`!=` ve `p as uN`
    /// birleştirmeye iner.
    pub(super) fn rewrite_expr(&mut self, e: Idx<Expr>) {
        if !self.rewritten.insert(e) {
            return;
        }
        for c in children(&self.ast.exprs[e].kind) {
            self.rewrite_expr(c);
        }
        match self.ast.exprs[e].kind.clone() {
            ExprKind::Field { .. } => {
                let Some((root, path, s)) = self.field_chain(e) else {
                    return;
                };
                let is_leaf = self
                    .layout(&s)
                    .is_some_and(|l| l.leaves.iter().any(|x| x.path == path));
                if !is_leaf {
                    return; // alt struct: bağlamı (==, atama …) açar
                }
                if let Some(p) = self.project(root, &path) {
                    self.rewrite_expr(p);
                    let kind = self.ast.exprs[p].kind.clone();
                    self.ast.exprs[e].kind = kind;
                } else {
                    self.unsupported_value(root, &s);
                }
            }
            ExprKind::Binary {
                op: op @ (BinOp::Eq | BinOp::Ne),
                lhs,
                rhs,
            } => {
                let Some(s) = self
                    .struct_of_expr(lhs)
                    .or_else(|| self.struct_of_expr(rhs))
                else {
                    return;
                };
                match (self.concat_of(lhs, &s), self.concat_of(rhs, &s)) {
                    (Some(l), Some(r)) => {
                        self.ast.exprs[e].kind = ExprKind::Binary { op, lhs: l, rhs: r };
                    }
                    (None, _) => self.unsupported_value(lhs, &s),
                    (_, None) => self.unsupported_value(rhs, &s),
                }
            }
            ExprKind::Cast { expr, ty } if self.struct_of_type(ty).is_none() => {
                let Some(s) = self.struct_of_expr(expr) else {
                    return;
                };
                match self.concat_of(expr, &s) {
                    Some(c) => self.ast.exprs[e].kind = ExprKind::Cast { expr: c, ty },
                    None => self.unsupported_value(expr, &s),
                }
            }
            _ => {}
        }
    }
}
