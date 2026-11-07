//! Modül taraması (ADR-0066): register'lar, her register'a yapılan
//! sıralı yazmalar (yol koşullarıyla) ve register üzerindeki `match`
//! deyimleri. Tanıyıcılar (fsm, counter) yalnız bu özeti okur.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use volt_ast::{
    Block, BlockStmt, ElseBranch, Expr, ExprKind, Idx, IfStmt, Item, MatchArmBody, ModuleDecl,
    OnTrigger, PatternKind, SourceFile, StmtKind, TypeRefKind,
};
use volt_span::Span;

use super::super::handshake::has_attr;
use super::super::mono::scope::Scopes;
use super::super::mono::unroll::{decl_name, eval_const};
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
pub(super) enum Branch {
    Then(Idx<Expr>),
    Else(Idx<Expr>),
}

/// Koşul ve `if` deyiminin bulunduğu yerin kapsam zinciri: koşuldaki ad
/// orada çözülür (#138).
#[derive(Clone)]
pub(super) struct Cond {
    pub branch: Branch,
    pub scope: Rc<Scopes>,
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
    /// Yazmanın bulunduğu yerin kapsam zinciri: sağ taraftaki ad orada
    /// çözülür (#138; blok `let`'i kök const'u gölgeler).
    pub scope: Rc<Scopes>,
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
}

/// Modülü tarar; saat portu yoksa None (kontratlar ilk saatte örneklenir).
pub(super) fn scan_module(
    ast: &SourceFile,
    consts: &HashMap<String, Idx<Expr>>,
    item: Idx<Item>,
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
        },
        scopes: contract_chain(ast, item, m),
        snapshot: None,
        conds: Vec::new(),
        arms: Vec::new(),
        in_for: false,
        on_clock: false,
    };
    for &si in &m.body {
        let stmt = &ast.stmts[si];
        match &stmt.kind {
            StmtKind::Reg(r) => {
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

/// Taramanın modül kapsamı: kök, portlar ve modül gövdesinin BÜTÜN
/// bildirimleri. Üretilen kontrat modül kapsamında yaşar ve oradaki her
/// bildirimi görür; sınırdaki ya da sabit yazmadaki ad hem yazmanın
/// bulunduğu yerde hem kontratta kök const'a bağlı olmalıdır. Bloklar bu
/// zincirin üstüne kendi yerellerini ekler (#138).
fn contract_chain(ast: &SourceFile, item: Idx<Item>, m: &ModuleDecl) -> Scopes {
    let mut chain = Scopes::module(ast, item);
    for &si in &m.body {
        if let Some(name) = decl_name(&ast.stmts[si].kind) {
            chain.declare_outer(name);
        }
    }
    chain
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
    /// Gezilen yerin kapsam zinciri (#138): kök, modül kapsamı, modül
    /// gövdesinin bildirimleri sırayla, blokların yerelleri.
    scopes: Scopes,
    /// Zincirin son anlık görüntüsü; zincir değişince düşer.
    snapshot: Option<Rc<Scopes>>,
    conds: Vec<Cond>,
    arms: Vec<ArmCtx>,
    in_for: bool,
    /// İlk saatin `on` bloğunda mıyız?
    on_clock: bool,
}

impl Walker<'_> {
    /// Zinciri değiştirir; anlık görüntü geçersizleşir.
    fn declare(&mut self, f: impl FnOnce(&mut Scopes)) {
        f(&mut self.scopes);
        self.snapshot = None;
    }

    /// Bulunulan yerin kapsam zinciri (paylaşılan anlık görüntü).
    fn scope(&mut self) -> Rc<Scopes> {
        self.snapshot
            .get_or_insert_with(|| Rc::new(self.scopes.clone()))
            .clone()
    }

    fn block(&mut self, b: Idx<Block>) {
        self.declare(Scopes::push);
        for s in &self.ast.blocks[b].stmts {
            match s {
                BlockStmt::NonBlockAssign { lhs, rhs, span } => {
                    let name = lhs.base.text.clone();
                    if !lhs.suffixes.is_empty() || self.in_for || !self.on_clock {
                        self.scan.tainted.insert(name);
                        continue;
                    }
                    let scope = self.scope();
                    self.scan.writes.entry(name).or_default().push(Write {
                        rhs: *rhs,
                        scope,
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
                    self.declare(|s| {
                        s.push();
                        s.declare_local(&f.var, true);
                    });
                    self.block(f.body);
                    self.declare(Scopes::pop);
                    self.in_for = outer;
                }
                BlockStmt::Let(l) => self.declare(|s| {
                    s.declare_local(&l.name, false);
                }),
                BlockStmt::Error => {}
            }
        }
        self.declare(Scopes::pop);
    }

    fn if_stmt(&mut self, ifs: &IfStmt) {
        let scope = self.scope();
        self.conds.push(Cond {
            branch: Branch::Then(ifs.cond),
            scope: scope.clone(),
        });
        self.block(ifs.then_block);
        self.conds.pop();
        if let Some(els) = &ifs.else_branch {
            self.conds.push(Cond {
                branch: Branch::Else(ifs.cond),
                scope,
            });
            match els {
                ElseBranch::Block(b) => self.block(*b),
                ElseBranch::If(inner) => self.if_stmt(inner),
            }
            self.conds.pop();
        }
    }

    fn match_stmt(&mut self, ms: &volt_ast::MatchStmt) {
        // Yerel (blok `let`'i, döngü değişkeni) register'ı gölgeler: o
        // `match` register üzerinde değildir (#138).
        let scrutinee = match &self.ast.exprs[ms.scrutinee].kind {
            ExprKind::Path(p)
                if p.segments.len() == 1 && !self.scopes.is_local(&p.segments[0].text) =>
            {
                Some(p.segments[0].text.clone())
            }
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

/// Literal / sabit adı / enum varyantı / alternatif / joker desen; başkası
/// None.
fn arm_pattern(
    ast: &SourceFile,
    consts: &HashMap<String, Idx<Expr>>,
    p: Idx<volt_ast::Pattern>,
) -> Option<ArmPat> {
    match &ast.patterns[p].kind {
        PatternKind::Wildcard => Some(ArmPat::Wildcard),
        // Sabit adı deseni (`LIMIT`, ADR-0085) değeriyle; kontrata ad kopyalanır.
        PatternKind::Literal(e) => {
            let value = match &ast.exprs[*e].kind {
                ExprKind::Path(_) => eval_const(ast, consts, *e, DEPTH0)?,
                _ => int_lit(ast, *e)?,
            };
            Some(ArmPat::Values(vec![(value, Lit::Expr(*e))]))
        }
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
