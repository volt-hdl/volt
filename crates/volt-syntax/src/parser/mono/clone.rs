//! Modül derin klonu + generic parametre ikamesi (ADR-0041).
//!
//! Şablon modülün her düğümü YENİ arena düğümlerine kopyalanır; span'ler
//! korunur ki tanılar kullanıcının yazdığı satırı göstersin. Tek
//! segmentli `Path` ifadesi bir const generic parametresini adlıyorsa
//! argüman değerinin `IntLit`'i ile değiştirilir. Tip konumunda çıplak
//! bir parametre adı (`Foo<TAPS>` → parser `GenericArg::Type`) da sabit
//! argümana dönüşür; iç içe generic örneklemeler böylece sonraki turda
//! literal argümanla yakalanır.

use std::collections::HashMap;

use volt_span::Span;

use super::budget::{AstWriter, ExpansionBudget};
use volt_ast::{
    ArrayLitKind, AttrArg, Attribute, Block, BlockStmt, BundleOrigin, Contract, ElseBranch, Expr,
    ExprKind, FieldInit, ForStmt, GenericArg, Idx, IfStmt, InstanceDecl, LValue, LValueSuffix,
    LetDecl, MatchStmt, ModuleDecl, Name, NumBase, OnBlock, OnTrigger, Path, Port, PortBinding,
    RegDecl, SourceFile, Stmt, StmtKind, TypeRef, TypeRefKind, WireDecl,
};

pub(super) struct Cloner<'a> {
    /// Yazma kapısı (ADR-0068 §6): her yazım açılım bütçesinden düşer.
    pub(super) ast: AstWriter<'a>,
    /// Monomorf bağlamı: klonlanan her span bu etiketi taşır (ADR-0041),
    /// resolve'un Span anahtarlı tabloları klonlar arasında çakışmaz.
    ctx: u16,
    /// const generic parametre (ya da açılan döngü değişkeni, ADR-0056)
    /// adı → değer. Negatif değer `-lit` olarak yazılır.
    subst: HashMap<String, i128>,
    /// Döngü açılımı (ADR-0056): gövdede bildirilen isim → yineleme
    /// sonekli adı (`pe` → `pe_0`). Bildirimlere ve tek segmentli yol /
    /// LValue tabanı referanslarına uygulanır; alan ve port adlarına
    /// dokunmaz.
    rename: HashMap<String, String>,
    /// Her iki operandı literale inen aritmetik katlanır (`0*4+1` → `1`);
    /// yalnız döngü açılımında açık — SV çıktısı okunur kalsın.
    fold: bool,
    /// Hata kurtarma düğümü (`Error` türü) klonlandı: şablon zaten
    /// tanılanmış, açıcı onu yeniden çoğaltmaz (ADR-0068 §6).
    saw_recovery: bool,
}

impl<'a> Cloner<'a> {
    pub(super) fn new(
        ast: &'a mut SourceFile,
        budget: &'a mut ExpansionBudget,
        subst: HashMap<String, i128>,
        ctx: u16,
    ) -> Self {
        Self {
            ast: AstWriter::new(ast, budget),
            subst,
            ctx,
            rename: HashMap::new(),
            fold: false,
            saw_recovery: false,
        }
    }

    /// Klonlanan ağaçta hata kurtarma düğümü vardı.
    pub(super) fn saw_recovery(&self) -> bool {
        self.saw_recovery
    }

    /// Kurtarma düğümü görüldü; türü aynen döner.
    pub(super) fn recovery<T>(&mut self, kind: T) -> T {
        self.saw_recovery = true;
        kind
    }

    /// Döngü açılımı kipi: isim yeniden yazma + sabit katlama (ADR-0056).
    pub(super) fn with_rename(mut self, rename: HashMap<String, String>) -> Self {
        self.rename = rename;
        self.fold = true;
        self
    }

    /// Gövdede bildirilen ismin bu yinelemedeki adı.
    fn declared_name(&mut self, n: &Name) -> Name {
        let mut name = self.tag_name(n);
        if let Some(new) = self.rename.get(&n.text) {
            name.text = new.clone();
            self.ast.charge_text(new.len() + n.text.len());
            // Kaynak adı tanılar için (ADR-0072). İç içe açılımda da `n`
            // şablondaki addır: iç gövde dış yinelemede yeniden
            // adlandırılmadan klonlanır.
            let source = n.text.clone();
            self.ast
                .write(|ast| ast.generate.source_names.insert(name.span, source));
        }
        name
    }

    /// İkame değerini literal (negatifse `-lit`) olarak yazar.
    fn subst_expr_kind(&mut self, v: i128, span: Span) -> ExprKind {
        if v >= 0 {
            return ExprKind::IntLit {
                value: v as u128,
                suffix: None,
                base: NumBase::Dec,
            };
        }
        let operand = self.int_lit(v.unsigned_abs(), span);
        ExprKind::Unary {
            op: volt_ast::UnOp::Neg,
            operand,
        }
    }

    /// `fold` açıkken iki literalli aritmetiği katlar; sonuç negatif ya
    /// da taşarsa dokunmaz (SV aynı ifadeyi kendisi hesaplar).
    fn fold_binary(&self, op: volt_ast::BinOp, lhs: Idx<Expr>, rhs: Idx<Expr>) -> Option<ExprKind> {
        use volt_ast::BinOp;
        if !self.fold {
            return None;
        }
        let lit = |e: Idx<Expr>| match &self.ast.exprs[e].kind {
            ExprKind::IntLit {
                value,
                suffix: None,
                ..
            } => i128::try_from(*value).ok(),
            _ => None,
        };
        let (l, r) = (lit(lhs)?, lit(rhs)?);
        let v = match op {
            BinOp::Add => l.checked_add(r),
            BinOp::Sub => l.checked_sub(r),
            BinOp::Mul => l.checked_mul(r),
            BinOp::Div if r != 0 => l.checked_div(r),
            BinOp::Rem if r != 0 => l.checked_rem(r),
            _ => None,
        }?;
        (v >= 0).then_some(ExprKind::IntLit {
            value: v as u128,
            suffix: None,
            base: NumBase::Dec,
        })
    }

    /// Span'i bu monomorfun bağlamıyla etiketler; satır/sütun değişmez.
    pub(super) fn tag(&self, span: Span) -> Span {
        span.with_ctx(self.ctx)
    }

    /// Ad span'ini etiketler; pipeline `pinned` girişi varsa etiketli
    /// anahtarla da eklenir (ADR-0038 yan tablosu Span anahtarlıdır).
    pub(super) fn tag_name(&mut self, n: &Name) -> Name {
        let span = self.tag(n.span);
        if let Some(&pin) = self.ast.timing.pinned.get(&n.span) {
            self.ast.write(|ast| ast.timing.pinned.insert(span, pin));
        }
        self.ast.charge_text(n.text.len());
        Name {
            text: n.text.clone(),
            span,
        }
    }

    pub(super) fn tag_path(&mut self, p: &Path) -> Path {
        Path {
            span: self.tag(p.span),
            segments: p.segments.iter().map(|s| self.tag_name(s)).collect(),
        }
    }

    /// Şablonu `mangled` adıyla klonlar; generics boşalır.
    pub(super) fn clone_module(&mut self, m: &ModuleDecl, mangled: String) -> ModuleDecl {
        let ports = m.ports.iter().map(|p| self.clone_port(p)).collect();
        let contracts = m
            .contracts
            .iter()
            .map(|c| Contract {
                span: self.tag(c.span),
                kind: c.kind,
                expr: self.clone_expr(c.expr),
                auto: c.auto.as_ref().map(|a| volt_ast::AutoOrigin {
                    from: self.tag(a.from),
                    ..a.clone()
                }),
            })
            .collect();
        let body = m.body.iter().map(|&s| self.clone_stmt(s)).collect();
        ModuleDecl {
            name: Name {
                text: mangled,
                span: self.tag(m.name.span),
            },
            generics: Vec::new(),
            ports,
            contracts,
            body,
            closing_name: None,
            // @mmio desugar'ı monomorfizasyondan önce koştu; liste boş.
            mmio_regs: Vec::new(),
        }
    }

    fn clone_port(&mut self, p: &Port) -> Port {
        Port {
            span: self.tag(p.span),
            attrs: self.clone_attrs(&p.attrs),
            doc: {
                self.ast.charge_text(p.doc.as_ref().map_or(0, String::len));
                p.doc.clone()
            },
            direction: p.direction,
            name: self.tag_name(&p.name),
            ty: self.clone_type(p.ty),
            domain: p.domain.as_ref().map(|d| self.tag_name(d)),
            bundle: p.bundle.as_ref().map(|b| BundleOrigin {
                port: self.tag_name(&b.port),
                bundle: b.bundle.clone(),
                path: b.path.clone(),
                declared: b.declared,
                flipped: b.flipped,
            }),
        }
    }

    pub(super) fn clone_attrs(&mut self, attrs: &[Attribute]) -> Vec<Attribute> {
        attrs
            .iter()
            .map(|a| Attribute {
                span: self.tag(a.span),
                name: self.tag_name(&a.name),
                args: a
                    .args
                    .iter()
                    .map(|arg| match arg {
                        AttrArg::Named { name, value } => AttrArg::Named {
                            name: self.tag_name(name),
                            value: self.clone_expr(*value),
                        },
                        AttrArg::Positional(e) => AttrArg::Positional(self.clone_expr(*e)),
                    })
                    .collect(),
            })
            .collect()
    }

    // ═══ Tipler ═══════════════════════════════════════════════════

    fn clone_type(&mut self, ty: Idx<TypeRef>) -> Idx<TypeRef> {
        let span = self.tag(self.ast.types[ty].span);
        let kind = match &self.ast.types[ty].kind {
            TypeRefKind::Bool => TypeRefKind::Bool,
            TypeRefKind::Clock => TypeRefKind::Clock,
            TypeRefKind::Reset(spec) => TypeRefKind::Reset(*spec),
            TypeRefKind::UInt(w) => TypeRefKind::UInt(*w),
            TypeRefKind::SInt(w) => TypeRefKind::SInt(*w),
            TypeRefKind::Trit => TypeRefKind::Trit,
            TypeRefKind::Error => self.recovery(TypeRefKind::Error),
            TypeRefKind::Bits(e) => {
                let e = *e;
                TypeRefKind::Bits(self.clone_expr(e))
            }
            TypeRefKind::UIntN(e) => {
                let e = *e;
                TypeRefKind::UIntN(self.clone_expr(e))
            }
            TypeRefKind::SIntN(e) => {
                let e = *e;
                TypeRefKind::SIntN(self.clone_expr(e))
            }
            TypeRefKind::Array { elem, len } => {
                let (elem, len) = (*elem, *len);
                TypeRefKind::Array {
                    elem: self.clone_type(elem),
                    len: self.clone_expr(len),
                }
            }
            TypeRefKind::Tuple(items) => {
                let items = items.clone();
                TypeRefKind::Tuple(items.iter().map(|&t| self.clone_type(t)).collect())
            }
            TypeRefKind::Path { path, args } => {
                let path = path.clone();
                let args = self.copy_generic_args(args);
                let args = args.iter().map(|a| self.clone_generic_arg(a)).collect();
                TypeRefKind::Path { path, args }
            }
        };
        let new = self
            .ast
            .write(|ast| ast.types.alloc(TypeRef { span, kind }));
        // `Delayed<T, N>` yan tablosu (ADR-0037): klon tip için yeni giriş.
        if let Some((n, dspan)) = self.ast.timing.delayed_types.get(&ty).copied() {
            let n = self.clone_expr(n);
            let dspan = self.tag(dspan);
            self.ast
                .write(|ast| ast.timing.delayed_types.insert(new, (n, dspan)));
        }
        new
    }

    /// Generic argümanların indeks kopyası (ödünç almayı kırmak için).
    fn copy_generic_args(&self, args: &[GenericArg]) -> Vec<GenericArg> {
        args.iter()
            .map(|a| match a {
                GenericArg::Type(t) => GenericArg::Type(*t),
                GenericArg::Const(e) => GenericArg::Const(*e),
            })
            .collect()
    }

    /// Çıplak parametre adı tip sanılmışsa sabit argümana dönüşür.
    fn clone_generic_arg(&mut self, arg: &GenericArg) -> GenericArg {
        match arg {
            GenericArg::Const(e) => GenericArg::Const(self.clone_expr(*e)),
            GenericArg::Type(t) => {
                if let Some(lit) = self.bare_param_type(*t) {
                    GenericArg::Const(lit)
                } else {
                    GenericArg::Type(self.clone_type(*t))
                }
            }
        }
    }

    fn bare_param_type(&mut self, t: Idx<TypeRef>) -> Option<Idx<Expr>> {
        let span = self.tag(self.ast.types[t].span);
        let TypeRefKind::Path { path, args } = &self.ast.types[t].kind else {
            return None;
        };
        if !args.is_empty() || path.segments.len() != 1 {
            return None;
        }
        let value = *self.subst.get(&path.segments[0].text)?;
        let kind = self.subst_expr_kind(value, span);
        Some(self.ast.write(|ast| ast.exprs.alloc(Expr { span, kind })))
    }

    fn int_lit(&mut self, value: u128, span: volt_span::Span) -> Idx<Expr> {
        self.ast.write(|ast| {
            ast.exprs.alloc(Expr {
                span,
                kind: ExprKind::IntLit {
                    value,
                    suffix: None,
                    base: NumBase::Dec,
                },
            })
        })
    }

    // ═══ İfadeler ═════════════════════════════════════════════════

    pub(super) fn clone_expr(&mut self, e: Idx<Expr>) -> Idx<Expr> {
        let span = self.tag(self.ast.exprs[e].span);
        let kind = self.clone_expr_kind(e);
        let new = self.ast.write(|ast| ast.exprs.alloc(Expr { span, kind }));
        // Blok içi generic örnekleme argümanları (ADR-0056).
        if let Some(args) = self.ast.generate.block_generic_args.get(&e) {
            let args = self.copy_generic_args(args);
            let args = args.iter().map(|a| self.clone_generic_arg(a)).collect();
            self.ast
                .write(|ast| ast.generate.block_generic_args.insert(new, args));
        }
        // `delay<K>(x)` yan tablosu (ADR-0037).
        if let Some(entries) = self.ast.timing.delay_exprs.get(&e).cloned() {
            let cloned = entries
                .iter()
                .map(|&(k, kspan)| (self.clone_expr(k), self.tag(kspan)))
                .collect();
            self.ast
                .write(|ast| ast.timing.delay_exprs.insert(new, cloned));
        }
        new
    }

    fn clone_expr_kind(&mut self, e: Idx<Expr>) -> ExprKind {
        match &self.ast.exprs[e].kind {
            ExprKind::IntLit {
                value,
                suffix,
                base,
            } => ExprKind::IntLit {
                value: *value,
                suffix: *suffix,
                base: *base,
            },
            ExprKind::BoolLit(b) => ExprKind::BoolLit(*b),
            ExprKind::StringLit(s) => {
                let s = s.clone();
                self.ast.charge_text(s.len());
                ExprKind::StringLit(s)
            }
            ExprKind::Todo { message } => {
                let message = message.clone();
                self.ast
                    .charge_text(message.as_ref().map_or(0, |m| m.len()));
                ExprKind::Todo { message }
            }
            ExprKind::Error => self.recovery(ExprKind::Error),
            ExprKind::Path(p) => {
                // İkame: const generic parametre / döngü değişkeni → literal.
                if p.segments.len() == 1 {
                    if let Some(&v) = self.subst.get(&p.segments[0].text) {
                        let span = self.tag(self.ast.exprs[e].span);
                        return self.subst_expr_kind(v, span);
                    }
                }
                let p = p.clone();
                let mut p = self.tag_path(&p);
                // Yineleme sonekli isim (ADR-0056): `pe.out` → `pe_0.out`.
                if p.segments.len() == 1 {
                    if let Some(new) = self.rename.get(&p.segments[0].text) {
                        p.segments[0].text = new.clone();
                        self.ast.charge_text(new.len());
                    }
                }
                ExprKind::Path(p)
            }
            ExprKind::Binary { .. }
            | ExprKind::Unary { .. }
            | ExprKind::Index { .. }
            | ExprKind::Range { .. }
            | ExprKind::PartSelect { .. } => self.clone_operator_expr(e),
            ExprKind::Field { .. }
            | ExprKind::Call { .. }
            | ExprKind::Cast { .. }
            | ExprKind::If { .. } => self.clone_access_expr(e),
            ExprKind::Match { .. }
            | ExprKind::StructLit { .. }
            | ExprKind::ArrayLit(_)
            | ExprKind::TupleLit(_)
            | ExprKind::Concat(_) => self.clone_aggregate_expr(e),
        }
    }

    /// İkili/tekli/indeks/aralık/parça-seçim ifadeleri.
    fn clone_operator_expr(&mut self, e: Idx<Expr>) -> ExprKind {
        match &self.ast.exprs[e].kind {
            ExprKind::Binary { op, lhs, rhs } => {
                let (op, lhs, rhs) = (*op, *lhs, *rhs);
                let (lhs, rhs) = (self.clone_expr(lhs), self.clone_expr(rhs));
                self.fold_binary(op, lhs, rhs)
                    .unwrap_or(ExprKind::Binary { op, lhs, rhs })
            }
            ExprKind::Unary { op, operand } => {
                let (op, operand) = (*op, *operand);
                ExprKind::Unary {
                    op,
                    operand: self.clone_expr(operand),
                }
            }
            ExprKind::Index { base, index } => {
                let (base, index) = (*base, *index);
                ExprKind::Index {
                    base: self.clone_expr(base),
                    index: self.clone_expr(index),
                }
            }
            ExprKind::Range { base, hi, lo } => {
                let (base, hi, lo) = (*base, *hi, *lo);
                ExprKind::Range {
                    base: self.clone_expr(base),
                    hi: self.clone_expr(hi),
                    lo: self.clone_expr(lo),
                }
            }
            ExprKind::PartSelect {
                base,
                start,
                width,
                ascending,
            } => {
                let (base, start, width, ascending) = (*base, *start, *width, *ascending);
                ExprKind::PartSelect {
                    base: self.clone_expr(base),
                    start: self.clone_expr(start),
                    width: self.clone_expr(width),
                    ascending,
                }
            }
            _ => unreachable!("clone_expr_kind yalnız operatör ifadelerini yönlendirir"),
        }
    }

    /// Alan erişimi, çağrı, dönüşüm ve koşullu ifade.
    fn clone_access_expr(&mut self, e: Idx<Expr>) -> ExprKind {
        match &self.ast.exprs[e].kind {
            ExprKind::Field { base, field } => {
                let (base, field) = (*base, field.clone());
                let field = self.tag_name(&field);
                ExprKind::Field {
                    base: self.clone_expr(base),
                    field,
                }
            }
            ExprKind::Call { callee, args } => {
                let (callee, args) = (*callee, args.clone());
                ExprKind::Call {
                    callee: self.clone_expr(callee),
                    args: args.iter().map(|&a| self.clone_expr(a)).collect(),
                }
            }
            ExprKind::Cast { expr, ty } => {
                let (expr, ty) = (*expr, *ty);
                ExprKind::Cast {
                    expr: self.clone_expr(expr),
                    ty: self.clone_type(ty),
                }
            }
            ExprKind::If {
                cond,
                then_expr,
                else_expr,
            } => {
                let (cond, then_expr, else_expr) = (*cond, *then_expr, *else_expr);
                ExprKind::If {
                    cond: self.clone_expr(cond),
                    then_expr: self.clone_expr(then_expr),
                    else_expr: self.clone_expr(else_expr),
                }
            }
            _ => unreachable!("clone_expr_kind yalnız erişim ifadelerini yönlendirir"),
        }
    }

    /// match, yapı/dizi/demet literalleri.
    fn clone_aggregate_expr(&mut self, e: Idx<Expr>) -> ExprKind {
        match &self.ast.exprs[e].kind {
            ExprKind::Match { scrutinee, arms } => {
                let scrutinee = *scrutinee;
                let arms = self.copy_arms(arms);
                ExprKind::Match {
                    scrutinee: self.clone_expr(scrutinee),
                    arms: arms.iter().map(|a| self.clone_arm(a)).collect(),
                }
            }
            ExprKind::StructLit { path, fields } => {
                let path = path.clone();
                let fields: Vec<(volt_span::Span, Name, Option<Idx<Expr>>)> = fields
                    .iter()
                    .map(|f| (f.span, f.name.clone(), f.value))
                    .collect();
                let path = self.tag_path(&path);
                ExprKind::StructLit {
                    path,
                    fields: fields
                        .into_iter()
                        .map(|(span, name, value)| FieldInit {
                            span: self.tag(span),
                            name: self.tag_name(&name),
                            value: value.map(|v| self.clone_expr(v)),
                        })
                        .collect(),
                }
            }
            ExprKind::ArrayLit(ArrayLitKind::List(items)) => {
                let items = items.clone();
                ExprKind::ArrayLit(ArrayLitKind::List(
                    items.iter().map(|&i| self.clone_expr(i)).collect(),
                ))
            }
            ExprKind::ArrayLit(ArrayLitKind::Repeat { value, count }) => {
                let (value, count) = (*value, *count);
                ExprKind::ArrayLit(ArrayLitKind::Repeat {
                    value: self.clone_expr(value),
                    count: self.clone_expr(count),
                })
            }
            ExprKind::TupleLit(items) => {
                let items = items.clone();
                ExprKind::TupleLit(items.iter().map(|&i| self.clone_expr(i)).collect())
            }
            ExprKind::Concat(parts) => {
                let parts = parts.clone();
                ExprKind::Concat(
                    parts
                        .iter()
                        .map(|&(i, t)| (self.clone_expr(i), t))
                        .collect(),
                )
            }
            _ => unreachable!("clone_expr_kind yalnız bileşik literalleri yönlendirir"),
        }
    }

    // ═══ Deyimler ve bloklar ══════════════════════════════════════

    fn clone_stmt(&mut self, s: Idx<Stmt>) -> Idx<Stmt> {
        let span = self.tag(self.ast.stmts[s].span);
        let attrs = self.copy_attrs_shallow(s);
        let attrs = self.clone_attrs(&attrs);
        let kind = self.clone_stmt_kind(s);
        self.ast
            .write(|ast| ast.stmts.alloc(Stmt { span, attrs, kind }))
    }

    /// Nitelik listesinin sahipli kopyası — arena ödünç almasını kırar.
    fn copy_attrs_shallow(&self, s: Idx<Stmt>) -> Vec<Attribute> {
        self.ast.stmts[s]
            .attrs
            .iter()
            .map(|a| Attribute {
                span: a.span,
                name: a.name.clone(),
                args: a
                    .args
                    .iter()
                    .map(|arg| match arg {
                        AttrArg::Named { name, value } => AttrArg::Named {
                            name: name.clone(),
                            value: *value,
                        },
                        AttrArg::Positional(e) => AttrArg::Positional(*e),
                    })
                    .collect(),
            })
            .collect()
    }

    fn clone_stmt_kind(&mut self, s: Idx<Stmt>) -> StmtKind {
        match &self.ast.stmts[s].kind {
            StmtKind::Error => self.recovery(StmtKind::Error),
            StmtKind::Expr(e) => {
                let e = *e;
                StmtKind::Expr(self.clone_expr(e))
            }
            StmtKind::Reg(_) | StmtKind::Let(_) | StmtKind::Wire(_) => self.clone_decl_stmt(s),
            StmtKind::Instance(inst) => {
                let inst = self.copy_instance(inst);
                StmtKind::Instance(self.clone_instance(inst))
            }
            StmtKind::On(on) => {
                let trigger = match &on.trigger {
                    OnTrigger::Clock(n) => OnTrigger::Clock(n.clone()),
                    OnTrigger::Reset(n) => OnTrigger::Reset(n.clone()),
                    OnTrigger::Error => OnTrigger::Error,
                };
                let body = on.body;
                let trigger = match trigger {
                    OnTrigger::Clock(n) => OnTrigger::Clock(self.tag_name(&n)),
                    OnTrigger::Reset(n) => OnTrigger::Reset(self.tag_name(&n)),
                    OnTrigger::Error => self.recovery(OnTrigger::Error),
                };
                StmtKind::On(OnBlock {
                    trigger,
                    body: self.clone_block(body),
                })
            }
            StmtKind::Comb(b) => {
                let b = *b;
                StmtKind::Comb(self.clone_block(b))
            }
            StmtKind::Assign(a) => {
                let lhs = self.copy_lvalue(&a.lhs);
                let rhs = a.rhs;
                StmtKind::Assign(volt_ast::AssignStmt {
                    lhs: self.clone_lvalue(lhs),
                    rhs: self.clone_expr(rhs),
                })
            }
            StmtKind::For(f) => {
                let (var, start, end, body) = (f.var.clone(), f.start, f.end, f.body);
                StmtKind::For(self.clone_for(var, start, end, body))
            }
        }
    }

    /// reg / let / wire bildirimleri.
    fn clone_decl_stmt(&mut self, s: Idx<Stmt>) -> StmtKind {
        match &self.ast.stmts[s].kind {
            StmtKind::Reg(r) => {
                let (name, domain, ty, init) = (r.name.clone(), r.domain.clone(), r.ty, r.init);
                StmtKind::Reg(RegDecl {
                    name: self.tag_name(&name),
                    domain: domain.as_ref().map(|d| self.tag_name(d)),
                    ty: ty.map(|t| self.clone_type(t)),
                    init: self.clone_expr(init),
                })
            }
            StmtKind::Let(l) => {
                let (name, ty, value) = (l.name.clone(), l.ty, l.value);
                StmtKind::Let(self.clone_let(name, ty, value))
            }
            StmtKind::Wire(w) => {
                let (name, ty) = (w.name.clone(), w.ty);
                StmtKind::Wire(WireDecl {
                    name: self.tag_name(&name),
                    ty: self.clone_type(ty),
                })
            }
            _ => unreachable!("clone_stmt_kind yalnız bildirimleri yönlendirir"),
        }
    }

    fn clone_let(&mut self, name: Name, ty: Option<Idx<TypeRef>>, value: Idx<Expr>) -> LetDecl {
        LetDecl {
            name: self.declared_name(&name),
            ty: ty.map(|t| self.clone_type(t)),
            value: self.clone_expr(value),
        }
    }

    fn clone_for(
        &mut self,
        var: Name,
        start: Idx<Expr>,
        end: Idx<Expr>,
        body: Idx<Block>,
    ) -> ForStmt {
        ForStmt {
            var: self.tag_name(&var),
            start: self.clone_expr(start),
            end: self.clone_expr(end),
            body: self.clone_block(body),
        }
    }

    /// Örneklemenin sığ kopyası (indeksler Copy) — ödünç almayı kırar.
    fn copy_instance(&self, inst: &InstanceDecl) -> InstanceDecl {
        InstanceDecl {
            name: inst.name.clone(),
            module_path: inst.module_path.clone(),
            generic_args: self.copy_generic_args(&inst.generic_args),
            bindings: inst
                .bindings
                .iter()
                .map(|b| PortBinding {
                    span: b.span,
                    port_name: b.port_name.clone(),
                    value: b.value,
                })
                .collect(),
        }
    }

    fn clone_instance(&mut self, inst: InstanceDecl) -> InstanceDecl {
        InstanceDecl {
            name: self.declared_name(&inst.name),
            module_path: self.tag_path(&inst.module_path),
            generic_args: inst
                .generic_args
                .iter()
                .map(|a| self.clone_generic_arg(a))
                .collect(),
            bindings: inst
                .bindings
                .into_iter()
                .map(|b| PortBinding {
                    span: self.tag(b.span),
                    port_name: self.tag_name(&b.port_name),
                    value: b.value.map(|v| self.clone_expr(v)),
                })
                .collect(),
        }
    }

    /// LValue'nun sahipli kopyası (ekler Idx taşır — Copy).
    fn copy_lvalue(&self, l: &LValue) -> LValue {
        LValue {
            span: l.span,
            base: l.base.clone(),
            suffixes: l
                .suffixes
                .iter()
                .map(|s| match s {
                    LValueSuffix::Index(e) => LValueSuffix::Index(*e),
                    LValueSuffix::Range { hi, lo } => LValueSuffix::Range { hi: *hi, lo: *lo },
                    LValueSuffix::PartSelect {
                        start,
                        width,
                        ascending,
                    } => LValueSuffix::PartSelect {
                        start: *start,
                        width: *width,
                        ascending: *ascending,
                    },
                    LValueSuffix::Field(n) => LValueSuffix::Field(n.clone()),
                })
                .collect(),
        }
    }

    fn clone_lvalue(&mut self, l: LValue) -> LValue {
        let suffixes = l
            .suffixes
            .into_iter()
            .map(|s| match s {
                LValueSuffix::Index(e) => LValueSuffix::Index(self.clone_expr(e)),
                LValueSuffix::Range { hi, lo } => LValueSuffix::Range {
                    hi: self.clone_expr(hi),
                    lo: self.clone_expr(lo),
                },
                LValueSuffix::PartSelect {
                    start,
                    width,
                    ascending,
                } => LValueSuffix::PartSelect {
                    start: self.clone_expr(start),
                    width: self.clone_expr(width),
                    ascending,
                },
                LValueSuffix::Field(n) => LValueSuffix::Field(self.tag_name(&n)),
            })
            .collect();
        LValue {
            span: self.tag(l.span),
            base: self.declared_name(&l.base),
            suffixes,
        }
    }

    pub(super) fn clone_block(&mut self, b: Idx<Block>) -> Idx<Block> {
        let (span, tail, context) = {
            let blk = &self.ast.blocks[b];
            (self.tag(blk.span), blk.tail, blk.context)
        };
        let count = self.ast.blocks[b].stmts.len();
        let mut stmts = Vec::with_capacity(count);
        for i in 0..count {
            let cloned = self.clone_block_stmt(b, i);
            stmts.push(cloned);
        }
        let tail = tail.map(|t| self.clone_expr(t));
        self.ast.write(|ast| {
            ast.blocks.alloc(Block {
                span,
                stmts,
                tail,
                context,
            })
        })
    }

    fn clone_block_stmt(&mut self, b: Idx<Block>, i: usize) -> BlockStmt {
        match &self.ast.blocks[b].stmts[i] {
            BlockStmt::Error => self.recovery(BlockStmt::Error),
            BlockStmt::NonBlockAssign { lhs, rhs, span } => {
                let (lhs, rhs, span) = (self.copy_lvalue(lhs), *rhs, self.tag(*span));
                BlockStmt::NonBlockAssign {
                    lhs: self.clone_lvalue(lhs),
                    rhs: self.clone_expr(rhs),
                    span,
                }
            }
            BlockStmt::BlockAssign { lhs, rhs, span } => {
                let (lhs, rhs, span) = (self.copy_lvalue(lhs), *rhs, self.tag(*span));
                BlockStmt::BlockAssign {
                    lhs: self.clone_lvalue(lhs),
                    rhs: self.clone_expr(rhs),
                    span,
                }
            }
            BlockStmt::If(i) => {
                let i = self.copy_if(i);
                BlockStmt::If(self.clone_if(i))
            }
            BlockStmt::Match(m) => {
                let (span, scrutinee) = (self.tag(m.span), m.scrutinee);
                let arms = self.copy_arms(&m.arms);
                BlockStmt::Match(MatchStmt {
                    span,
                    scrutinee: self.clone_expr(scrutinee),
                    arms: arms.iter().map(|a| self.clone_arm(a)).collect(),
                })
            }
            BlockStmt::Let(l) => {
                let (name, ty, value) = (l.name.clone(), l.ty, l.value);
                BlockStmt::Let(self.clone_let(name, ty, value))
            }
            BlockStmt::For(f) => {
                let (var, start, end, body) = (f.var.clone(), f.start, f.end, f.body);
                BlockStmt::For(self.clone_for(var, start, end, body))
            }
        }
    }

    /// IfStmt'in sahipli sığ kopyası (blok/ifade indeksleri Copy).
    fn copy_if(&self, i: &IfStmt) -> IfStmt {
        IfStmt {
            span: i.span,
            cond: i.cond,
            then_block: i.then_block,
            else_branch: i.else_branch.as_ref().map(|e| match e {
                ElseBranch::Block(b) => ElseBranch::Block(*b),
                ElseBranch::If(nested) => ElseBranch::If(Box::new(self.copy_if(nested))),
            }),
        }
    }

    fn clone_if(&mut self, i: IfStmt) -> IfStmt {
        IfStmt {
            span: self.tag(i.span),
            cond: self.clone_expr(i.cond),
            then_block: self.clone_block(i.then_block),
            else_branch: i.else_branch.map(|e| match e {
                ElseBranch::Block(b) => ElseBranch::Block(self.clone_block(b)),
                ElseBranch::If(nested) => ElseBranch::If(Box::new(self.clone_if(*nested))),
            }),
        }
    }
}
