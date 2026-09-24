//! Modül taraması (ADR-0066): register'lar, her register'a yapılan
//! sıralı yazmalar (yol koşullarıyla) ve register üzerindeki `match`
//! deyimleri. Tanıyıcılar (fsm, counter) yalnız bu özeti okur.

use std::collections::{HashMap, HashSet};

use volt_ast::{
    Block, BlockStmt, ElseBranch, Expr, ExprKind, Idx, IfStmt, MatchArmBody, ModuleDecl, OnTrigger,
    PatternKind, SourceFile, StmtKind, TypeRefKind,
};
use volt_span::Span;

use super::super::handshake::has_attr;
use super::super::mono::unroll::eval_const;
use super::NO_AUTO_CONTRACTS;

/// Sabit değerlendirme özyineleme başlangıç derinliği.
const DEPTH0: u32 = 0;

/// Enum tipli register'ın kodlaması (ADR-0074).
pub(super) struct EnumInfo {
    pub name: String,
    /// Varyantlar bildirim sırasıyla: (ad, kod).
    pub variants: Vec<(String, i128)>,
    /// Bütün `2^W` kodlar bir varyant (F1 totoloji).
    pub dense: bool,
}

/// Tanıyıcılara aday register.
pub(super) struct RegInfo {
    pub name: String,
    /// Bit genişliği (`uN` / `uint<N>` / geçerli kodlamalı enum).
    pub width: u32,
    /// Enum tipli register'ın kodlaması.
    pub enum_info: Option<EnumInfo>,
    pub init: Idx<Expr>,
    /// `@no_auto_contracts reg ...`
    pub opted_out: bool,
}

/// Bir yazmayı kuşatan `if` dalı: koşul ve hangi kolda olduğu.
#[derive(Clone, Copy)]
pub(super) enum Cond {
    Then(Idx<Expr>),
    Else(Idx<Expr>),
}

/// Yazmayı kuşatan `match` kolu.
#[derive(Clone)]
pub(super) struct ArmCtx {
    pub scrutinee: Option<String>,
    pub match_id: usize,
    pub arm: usize,
}

/// `r <= e` — ilk saat alanındaki sıralı bloktan, soneksiz.
pub(super) struct Write {
    pub rhs: Idx<Expr>,
    pub span: Span,
    /// Dıştan içe kuşatan koşullar.
    pub conds: Vec<Cond>,
    /// Dıştan içe kuşatan match kolları.
    pub arms: Vec<ArmCtx>,
}

/// Desendeki bir değerin kaynak yazımı.
#[derive(Clone)]
pub(super) enum Lit {
    /// Literal ifade (`2`).
    Expr(Idx<Expr>),
    /// Enum varyant deseni (`State::Run`).
    Path(Vec<String>),
}

impl Lit {
    pub fn g(&self) -> super::gen::G {
        match self {
            Lit::Expr(e) => super::gen::G::Copy(*e),
            Lit::Path(p) => super::gen::G::Path(p.clone()),
        }
    }
}

/// Match kolunun deseni (FSM için literal, enum varyantı ve joker).
pub(super) enum ArmPat {
    /// `1`, `1 | 2`, `State::Run` — değerler ve kaynak yazımları.
    Values(Vec<(i128, Lit)>),
    Wildcard,
}

/// Register üzerinde (`match r { ... }`) bir deyim.
pub(super) struct RegMatch {
    pub scrutinee: String,
    pub span: Span,
    /// Tüm kollar literal/joker ve muhafızsızsa Some.
    pub arms: Option<Vec<ArmPat>>,
}

impl RegMatch {
    /// Literal kollarda adı geçen tüm değerler (kaynak sırasıyla).
    pub fn literal_values(&self) -> Vec<(i128, Lit)> {
        let mut out: Vec<(i128, Lit)> = Vec::new();
        for arm in self.arms.iter().flatten() {
            if let ArmPat::Values(vs) = arm {
                for (v, e) in vs {
                    if !out.iter().any(|(x, _)| x == v) {
                        out.push((*v, e.clone()));
                    }
                }
            }
        }
        out
    }
}

/// Modül özeti.
pub(super) struct Scan {
    pub regs: Vec<RegInfo>,
    pub writes: HashMap<String, Vec<Write>>,
    /// Tanınamayan biçimde yazılan register'lar (kombinasyonel atama,
    /// sonekli hedef, `for` içi, başka saat alanı).
    pub tainted: HashSet<String>,
    pub matches: Vec<RegMatch>,
    /// Modül içi adlar (port, reg, let, wire, örnek): sınır ifadesindeki
    /// ad bunlardan biriyse sabit sayılmaz.
    pub locals: HashSet<String>,
}

/// Modülü tarar; saat portu yoksa None (kontratlar ilk saatte örneklenir).
pub(super) fn scan_module(
    ast: &SourceFile,
    consts: &HashMap<String, Idx<Expr>>,
    m: &ModuleDecl,
) -> Option<Scan> {
    let clock = m
        .ports
        .iter()
        .find(|p| matches!(ast.types[p.ty].kind, TypeRefKind::Clock))?
        .name
        .text
        .clone();
    let mut w = Walker {
        ast,
        consts,
        clock,
        scan: Scan {
            regs: Vec::new(),
            writes: HashMap::new(),
            tainted: HashSet::new(),
            matches: Vec::new(),
            locals: m.ports.iter().map(|p| p.name.text.clone()).collect(),
        },
        conds: Vec::new(),
        arms: Vec::new(),
        in_for: false,
        on_clock: false,
    };
    for &si in &m.body {
        let stmt = &ast.stmts[si];
        match &stmt.kind {
            StmtKind::Reg(r) => {
                w.scan.locals.insert(r.name.text.clone());
                let info = r.ty.and_then(|t| {
                    uint_width(ast, consts, t)
                        .map(|w| (w, None))
                        .or_else(|| enum_width(ast, consts, t))
                });
                if let Some((width, enum_info)) = info {
                    w.scan.regs.push(RegInfo {
                        name: r.name.text.clone(),
                        width,
                        enum_info,
                        init: r.init,
                        opted_out: has_attr(&stmt.attrs, NO_AUTO_CONTRACTS),
                    });
                }
            }
            StmtKind::Let(l) => {
                w.scan.locals.insert(l.name.text.clone());
            }
            StmtKind::Wire(x) => {
                w.scan.locals.insert(x.name.text.clone());
            }
            StmtKind::Instance(x) => {
                w.scan.locals.insert(x.name.text.clone());
            }
            StmtKind::On(on) => {
                w.on_clock = match &on.trigger {
                    OnTrigger::Clock(n) | OnTrigger::Reset(n) => n.text == w.clock,
                    OnTrigger::Error => false,
                };
                w.block(on.body);
                w.on_clock = false;
            }
            StmtKind::Comb(b) => w.block(*b),
            StmtKind::Assign(a) => {
                w.scan.tainted.insert(a.lhs.base.text.clone());
            }
            _ => {}
        }
    }
    Some(w.scan)
}

/// `uN` / `uint<N>` genişliği; 1..=64 dışı None (u128 aşımı olmasın).
fn uint_width(
    ast: &SourceFile,
    consts: &HashMap<String, Idx<Expr>>,
    ty: Idx<volt_ast::TypeRef>,
) -> Option<u32> {
    let w = match &ast.types[ty].kind {
        TypeRefKind::UInt(n) => i128::from(*n),
        TypeRefKind::UIntN(e) => eval_const(ast, consts, *e, DEPTH0)?,
        _ => return None,
    };
    u32::try_from(w).ok().filter(|w| (1..=64).contains(w))
}

/// Geçerli kodlamalı, birim varyantlı enum tipi (ADR-0074): genişlik
/// ve varyant kodları.
fn enum_width(
    ast: &SourceFile,
    consts: &HashMap<String, Idx<Expr>>,
    ty: Idx<volt_ast::TypeRef>,
) -> Option<(u32, Option<EnumInfo>)> {
    let decl = volt_ast::enum_layout::enum_of_type(ast, ty)?;
    let layout = volt_ast::enum_layout::valid_layout(ast, decl, &mut |e| {
        eval_const(ast, consts, e, DEPTH0)
    })?;
    if !(1..=64).contains(&layout.width) {
        return None;
    }
    let variants = decl
        .variants
        .iter()
        .zip(&layout.values)
        .map(|(v, &c)| (v.name.text.clone(), c as i128))
        .collect();
    Some((
        layout.width,
        Some(EnumInfo {
            name: decl.name.text.clone(),
            variants,
            dense: layout.is_dense(),
        }),
    ))
}

struct Walker<'a> {
    ast: &'a SourceFile,
    consts: &'a HashMap<String, Idx<Expr>>,
    clock: String,
    scan: Scan,
    conds: Vec<Cond>,
    arms: Vec<ArmCtx>,
    in_for: bool,
    /// İlk saatin `on` bloğunda mıyız?
    on_clock: bool,
}

impl Walker<'_> {
    fn block(&mut self, b: Idx<Block>) {
        for s in &self.ast.blocks[b].stmts {
            match s {
                BlockStmt::NonBlockAssign { lhs, rhs, span } => {
                    let name = lhs.base.text.clone();
                    if !lhs.suffixes.is_empty() || self.in_for || !self.on_clock {
                        self.scan.tainted.insert(name);
                        continue;
                    }
                    self.scan.writes.entry(name).or_default().push(Write {
                        rhs: *rhs,
                        span: *span,
                        conds: self.conds.clone(),
                        arms: self.arms.clone(),
                    });
                }
                BlockStmt::BlockAssign { lhs, .. } => {
                    self.scan.tainted.insert(lhs.base.text.clone());
                }
                BlockStmt::If(ifs) => self.if_stmt(ifs),
                BlockStmt::Match(ms) => self.match_stmt(ms),
                BlockStmt::For(f) => {
                    let outer = std::mem::replace(&mut self.in_for, true);
                    self.block(f.body);
                    self.in_for = outer;
                }
                BlockStmt::Let(_) | BlockStmt::Error => {}
            }
        }
    }

    fn if_stmt(&mut self, ifs: &IfStmt) {
        self.conds.push(Cond::Then(ifs.cond));
        self.block(ifs.then_block);
        self.conds.pop();
        if let Some(els) = &ifs.else_branch {
            self.conds.push(Cond::Else(ifs.cond));
            match els {
                ElseBranch::Block(b) => self.block(*b),
                ElseBranch::If(inner) => self.if_stmt(inner),
            }
            self.conds.pop();
        }
    }

    fn match_stmt(&mut self, ms: &volt_ast::MatchStmt) {
        let scrutinee = match &self.ast.exprs[ms.scrutinee].kind {
            ExprKind::Path(p) if p.segments.len() == 1 => Some(p.segments[0].text.clone()),
            _ => None,
        };
        let match_id = self.scan.matches.len();
        if let Some(name) = &scrutinee {
            let arms = ms
                .arms
                .iter()
                .map(|a| {
                    if a.guard.is_some() {
                        return None;
                    }
                    arm_pattern(self.ast, self.consts, a.pattern)
                })
                .collect::<Option<Vec<_>>>();
            self.scan.matches.push(RegMatch {
                scrutinee: name.clone(),
                span: ms.span,
                arms,
            });
        } else {
            // Kimliği sabit tutmak için adsız kayıt (FSM sayılmaz).
            self.scan.matches.push(RegMatch {
                scrutinee: String::new(),
                span: ms.span,
                arms: None,
            });
        }
        for (i, arm) in ms.arms.iter().enumerate() {
            let MatchArmBody::Block(b) = arm.body else {
                continue;
            };
            self.arms.push(ArmCtx {
                scrutinee: scrutinee.clone(),
                match_id,
                arm: i,
            });
            self.block(b);
            self.arms.pop();
        }
    }
}

/// Literal / enum varyantı / alternatif / joker desen; başkası None.
fn arm_pattern(
    ast: &SourceFile,
    consts: &HashMap<String, Idx<Expr>>,
    p: Idx<volt_ast::Pattern>,
) -> Option<ArmPat> {
    match &ast.patterns[p].kind {
        PatternKind::Wildcard => Some(ArmPat::Wildcard),
        PatternKind::Literal(e) => Some(ArmPat::Values(vec![(int_lit(ast, *e)?, Lit::Expr(*e))])),
        PatternKind::Path { path, args: None } => {
            let (decl, idx) = super::enum_variant(ast, path)?;
            let layout = volt_ast::enum_layout::valid_layout(ast, decl, &mut |e| {
                eval_const(ast, consts, e, DEPTH0)
            })?;
            let code = i128::try_from(layout.values[idx]).ok()?;
            let segs = vec![decl.name.text.clone(), decl.variants[idx].name.text.clone()];
            Some(ArmPat::Values(vec![(code, Lit::Path(segs))]))
        }
        PatternKind::Or(alts) => {
            let mut vals = Vec::new();
            for &a in alts {
                match arm_pattern(ast, consts, a)? {
                    ArmPat::Wildcard => return Some(ArmPat::Wildcard),
                    ArmPat::Values(vs) => vals.extend(vs),
                }
            }
            Some(ArmPat::Values(vals))
        }
        _ => None,
    }
}

fn int_lit(ast: &SourceFile, e: Idx<Expr>) -> Option<i128> {
    match &ast.exprs[e].kind {
        ExprKind::IntLit { value, .. } => i128::try_from(*value).ok(),
        _ => None,
    }
}
