//! Kontrat ifadesi üretimi (ADR-0066): tanıyıcıların küçük ara
//! biçimi (`G`) AST'ye açılır. Her ad BENZERSİZ, sıfır uzunluklu bir
//! span alır — isim çözümleme kullanımları span ile anahtarlar
//! (`use_spans`) ve kaynaktaki gerçek adlar hiçbir zaman sıfır uzunlukta
//! değildir. Ifade düğümlerinin span'i kökendir: E5001 ve SVA yorumu
//! kontratı doğuran yapıyı gösterir.

use std::collections::HashSet;

use volt_ast::{
    AutoOrigin, AutoRule, BinOp, BlockStmt, Contract, ContractKind, Expr, ExprKind, Idx, Name,
    Path, SourceFile, StmtKind,
};
use volt_span::Span;

use super::print;

const PREV: &str = "prev";

/// Ara ifade biçimi.
pub(super) enum G {
    /// Modül içi ad (register).
    Name(String),
    /// `prev(ad)`.
    Prev(String),
    /// Kullanıcının yazdığı sabit ifade / literal (derin kopya).
    Copy(Idx<Expr>),
    Bin(BinOp, Box<G>, Box<G>),
}

impl G {
    pub fn bin(op: BinOp, l: G, r: G) -> G {
        G::Bin(op, Box::new(l), Box::new(r))
    }
}

/// Üretilecek kontrat.
pub(super) struct Spec {
    pub kind: ContractKind,
    pub rule: AutoRule,
    pub expr: G,
    pub subject: String,
    pub from: Span,
}

/// Birimde zaten kullanılan sıfır uzunluklu ad span'leri (başka
/// desugar'ların ürettiği adlar: Handshake, çift yönlü port, ...).
pub(super) fn used_zero_spans(ast: &SourceFile) -> HashSet<Span> {
    let mut used = HashSet::new();
    let mut add = |s: Span| {
        if s.start == s.end {
            used.insert(s);
        }
    };
    for e in ast.exprs.iter() {
        if let ExprKind::Path(p) = &e.kind {
            p.segments.iter().for_each(|n| add(n.span));
        }
    }
    for b in ast.blocks.iter() {
        for s in &b.stmts {
            match s {
                BlockStmt::NonBlockAssign { lhs, .. } | BlockStmt::BlockAssign { lhs, .. } => {
                    add(lhs.base.span)
                }
                BlockStmt::Let(l) => add(l.name.span),
                _ => {}
            }
        }
    }
    for s in ast.stmts.iter() {
        match &s.kind {
            StmtKind::Reg(r) => add(r.name.span),
            StmtKind::Let(l) => add(l.name.span),
            StmtKind::Wire(w) => add(w.name.span),
            StmtKind::Instance(i) => add(i.name.span),
            StmtKind::Assign(a) => add(a.lhs.base.span),
            _ => {}
        }
    }
    used
}

/// Modül başına ad span'i dağıtıcısı: öğe span'i içinde soldan sağa.
pub(super) struct Spans<'a> {
    pub used: &'a mut HashSet<Span>,
    pub item: Span,
    pub cursor: u32,
}

impl Spans<'_> {
    fn fresh(&mut self) -> Option<Span> {
        while self.cursor <= self.item.end {
            let s = Span::new(self.item.file, self.cursor, self.cursor).with_ctx(self.item.ctx);
            self.cursor += 1;
            if self.used.insert(s) {
                return Some(s);
            }
        }
        None
    }
}

/// `Spec` → kontrat (metniyle). Ad span'i biterse ya da kopyalanamayan
/// bir ifade varsa None — kontrat üretilmez.
pub(super) fn build(ast: &mut SourceFile, spans: &mut Spans, spec: &Spec) -> Option<Contract> {
    let mut b = Builder {
        ast,
        spans,
        from: spec.from,
    };
    let expr = b.g(&spec.expr)?;
    let text = print::expr(b.ast, expr)?;
    Some(Contract {
        span: spec.from,
        kind: spec.kind,
        expr,
        auto: Some(AutoOrigin {
            rule: spec.rule,
            text,
            subject: spec.subject.clone(),
            from: spec.from,
        }),
    })
}

struct Builder<'a, 'b> {
    ast: &'a mut SourceFile,
    spans: &'a mut Spans<'b>,
    from: Span,
}

impl Builder<'_, '_> {
    fn alloc(&mut self, kind: ExprKind) -> Idx<Expr> {
        self.ast.exprs.alloc(Expr {
            span: self.from,
            kind,
        })
    }

    fn path(&mut self, segments: &[String]) -> Option<Idx<Expr>> {
        let segments = segments
            .iter()
            .map(|t| {
                Some(Name {
                    text: t.clone(),
                    span: self.spans.fresh()?,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        let span = self.from;
        Some(self.alloc(ExprKind::Path(Path { span, segments })))
    }

    fn g(&mut self, g: &G) -> Option<Idx<Expr>> {
        match g {
            G::Name(n) => self.path(std::slice::from_ref(n)),
            G::Prev(n) => {
                let callee = self.path(&[PREV.to_string()])?;
                let arg = self.path(std::slice::from_ref(n))?;
                Some(self.alloc(ExprKind::Call {
                    callee,
                    args: vec![arg],
                }))
            }
            G::Copy(e) => self.copy(*e),
            G::Bin(op, l, r) => {
                let lhs = self.g(l)?;
                let rhs = self.g(r)?;
                Some(self.alloc(ExprKind::Binary { op: *op, lhs, rhs }))
            }
        }
    }

    /// Sabit ifadenin derin kopyası (literal, ad, ikili/tekli işlem).
    fn copy(&mut self, e: Idx<Expr>) -> Option<Idx<Expr>> {
        let kind = match &self.ast.exprs[e].kind {
            ExprKind::IntLit {
                value,
                suffix,
                base,
            } => ExprKind::IntLit {
                value: *value,
                suffix: *suffix,
                base: *base,
            },
            ExprKind::BoolLit(v) => ExprKind::BoolLit(*v),
            ExprKind::Path(p) => {
                let names: Vec<String> = p.segments.iter().map(|s| s.text.clone()).collect();
                return self.path(&names);
            }
            &ExprKind::Binary { op, lhs, rhs } => {
                let lhs = self.copy(lhs)?;
                let rhs = self.copy(rhs)?;
                ExprKind::Binary { op, lhs, rhs }
            }
            &ExprKind::Unary { op, operand } => {
                let operand = self.copy(operand)?;
                ExprKind::Unary { op, operand }
            }
            _ => return None,
        };
        Some(self.alloc(kind))
    }
}
