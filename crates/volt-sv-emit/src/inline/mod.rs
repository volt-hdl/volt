//! Fonksiyon çağrılarının SV'ye indirgenmesi (ADR-0081 Karar 12 — A:
//! çağrı yerinde açılım).
//!
//! fn SV'de görünmez: her çağrı, çağıran modülde kendi kombinasyonel
//! devresine açılır. Geçit emit'ten ÖNCE, `structs::lower`'dan da önce,
//! AST'nin bir kopyası üzerinde koşar: açılımın ürettiği struct tipli
//! teller ve enum varyant kullanımları mevcut geçitlerden geçer.
//! Birimde çağrılan fn yoksa `None` döner ve emitter özgün AST'yi
//! kullanır — fn kullanmayan tasarımın çıktısı byte-aynı kalır.
//!
//! | Çağrı yeri                                   | Kip    |
//! |----------------------------------------------|--------|
//! | modül `let` / atama / örnek bağlantısı       | tel    |
//! | `on` bloğu (argümanlar modül düzeyi adlar)   | tel    |
//! | `on` bloğu (argüman blok yereline başvurur)  | ikame  |
//! | `comb` bloğu, blok içi `for`, kontrat, `reg` başlangıcı | ikame |
//!
//! Karar 12.4: fn gövdesi çağrılmasa da bir kez doğrulanır —
//! [`validation_unit`] her fn için gövdesini ikame kipinde süren bir
//! sentetik modül kurar; emitter onu çıktıya yazmadan emit eder ve
//! tanılar fn tanımında, çağrı sayısından bağımsız bir kez çıkar.

mod expand;
mod scope;
mod types;

use std::collections::{HashMap, HashSet};

use volt_ast::{
    Block, BlockStmt, ElseBranch, Expr, ExprKind, Idx, IfStmt, Item, ItemKind, LValue,
    LValueSuffix, ModuleDecl, Name, Path, Pattern, PatternArgs, PatternKind, Port, PortDir,
    SourceFile, Stmt, StmtKind, Visibility,
};
use volt_diagnostics::Diagnostic;
use volt_span::Span;

use expand::{Ctx, Expander, Mode};
use scope::FnInfo;
use types::{canon, DeclTypes};

/// Açılmış birim + emitter notları.
pub(crate) struct Lowered {
    pub ast: SourceFile,
    pub notes: InlineNotes,
}

/// Emitter'ın açılımdan okuduğu yan bilgiler.
#[derive(Debug, Default)]
pub(crate) struct InlineNotes {
    /// (modül, çağrının ilk teli) → başlık yorumu
    /// `// <fn>(<argümanlar>) — <dosya>:<satır>`.
    pub headers: HashMap<(String, String), CallHeader>,
    /// SV boyut dönüşümü `W'(e)` olarak yazılacak `as` düğümleri (ikame
    /// kipinde genişlik korunumu).
    pub sized_casts: HashSet<Idx<Expr>>,
    /// Gövdedeki const başvuruları: çağıranın aynı adlı sinyaline değil
    /// const'a bağlanır (hijyen).
    pub global_paths: HashSet<Idx<Expr>>,
    /// İkame kipinde tipsiz `let` değeri: emitter onu tel kipindeki telin
    /// çıkarılacak genişliğinde `W'(e)` olarak yazar.
    pub self_sized: HashSet<Idx<Expr>>,
}

impl InlineNotes {
    /// `name` teli (ya da struct indirgemesinin ondan türettiği ilk
    /// yaprak `name_<alan>`) bir çağrı başlığı taşıyor mu.
    pub(crate) fn header_key(&self, module: &str, name: &str) -> Option<(String, String)> {
        // Önce tam ad, sonra alt çizgi sınırlarında en uzun ön ek
        // (`br_taken_0_o_a` → `br_taken_0_o`): O(ad uzunluğu) arama.
        let mut prefix = name;
        loop {
            let key = (module.to_string(), prefix.to_string());
            if self.headers.contains_key(&key) {
                return Some(key);
            }
            prefix = &prefix[..prefix.rfind('_')?];
        }
    }
}

/// Bir çağrının başlık yorumu için kaynak konumları.
#[derive(Debug, Clone)]
pub(crate) struct CallHeader {
    pub fn_name: String,
    pub call: Span,
    pub args: Vec<Span>,
}

/// Sentetik doğrulama modüllerinin ad öneki (kullanıcı adı olamaz: `__`).
pub(crate) const VALIDATION_PREFIX: &str = "__volt_fn_";

/// Birimi açar. `skip`: tanımda doğrulanamamış fn'ler (çağrıları açılmaz,
/// tanı zaten var).
pub(crate) fn lower(
    ast: &SourceFile,
    skip: &HashSet<String>,
) -> (Option<Lowered>, Vec<Diagnostic>) {
    let fns = collect_fns(ast);
    if fns.is_empty() || !has_user_call(ast, &fns) {
        return (None, Vec::new());
    }
    let consts = const_names(ast);
    let mut ex = Expander::new(ast, &fns, skip, consts);
    for &item in &ast.items {
        if matches!(ast.items_arena[item].kind, ItemKind::Module(_)) {
            lower_module(&mut ex, item);
        }
    }
    let Expander {
        out, notes, diags, ..
    } = ex;
    (Some(Lowered { ast: out, notes }), diags)
}

fn collect_fns(ast: &SourceFile) -> HashMap<String, FnInfo<'_>> {
    let mut fns = HashMap::new();
    for &item in &ast.items {
        let it = &ast.items_arena[item];
        if let ItemKind::Fn(f) = &it.kind {
            // Generic fn bu turda E0003 (HIR); açılmaz.
            if f.generics.is_empty() && !fns.contains_key(&f.name.text) {
                fns.insert(f.name.text.clone(), FnInfo::new(ast, f, it.span));
            }
        }
    }
    fns
}

fn const_names(ast: &SourceFile) -> HashSet<String> {
    ast.items
        .iter()
        .filter_map(|&i| match &ast.items_arena[i].kind {
            ItemKind::Const(c) => Some(c.name.text.clone()),
            _ => None,
        })
        .collect()
}

/// Modüllerde (gövde ya da kontrat) bir fn adına yapılan çağrı var mı.
fn has_user_call(ast: &SourceFile, fns: &HashMap<String, FnInfo<'_>>) -> bool {
    let fn_exprs: HashSet<Idx<Expr>> = fns
        .values()
        .flat_map(|f| f.bindings.keys().copied())
        .collect();
    ast.exprs.iter_idx().any(|(e, expr)| {
        !fn_exprs.contains(&e)
            && matches!(&expr.kind, ExprKind::Call { callee, .. }
                if matches!(&ast.exprs[*callee].kind,
                    ExprKind::Path(p) if p.segments.len() == 1 && fns.contains_key(&p.segments[0].text)))
    })
}

/// Modülde kullanıcının bildirdiği adlar (E1003 ve gölgeleme) + const'lar.
fn module_names(ast: &SourceFile, m: &ModuleDecl) -> HashSet<String> {
    let mut names = const_names(ast);
    for p in &m.ports {
        names.insert(p.name.text.clone());
    }
    for &s in &m.body {
        let name = match &ast.stmts[s].kind {
            StmtKind::Reg(r) => &r.name,
            StmtKind::Let(l) => &l.name,
            StmtKind::Wire(w) => &w.name,
            StmtKind::Instance(i) => &i.name,
            _ => continue,
        };
        names.insert(name.text.clone());
    }
    names
}

fn lower_module(ex: &mut Expander<'_>, item: Idx<Item>) {
    let ItemKind::Module(m) = &ex.out.items_arena[item].kind else {
        return;
    };
    let m = m.clone();
    let names = module_names(&ex.out, &m);
    let decls = DeclTypes::of_module(&ex.out, &m);
    ex.enter_module(&m.name.text, names, decls);
    let mut body = Vec::with_capacity(m.body.len());
    for &s in &m.body {
        lower_stmt(ex, s);
        body.append(&mut ex.pending);
        body.push(s);
    }
    for c in &m.contracts {
        ex.rewrite(c.expr, &Ctx::subst());
    }
    if let ItemKind::Module(out_m) = &mut ex.out.items_arena[item].kind {
        out_m.body = body;
    }
}

fn lower_stmt(ex: &mut Expander<'_>, s: Idx<Stmt>) {
    match ex.out.stmts[s].kind.clone() {
        StmtKind::Let(l) => {
            if let Some((name, args)) = whole_call(ex, l.value) {
                let ret = ex.fns[&name].decl.return_ty;
                let whole = match (l.ty, ret) {
                    // Tipsiz `let`in tipi dönüş tipidir (tip denetimiyle aynı).
                    (None, Some(r)) => {
                        if let StmtKind::Let(out_l) = &mut ex.out.stmts[s].kind {
                            out_l.ty = Some(r);
                        }
                        true
                    }
                    (Some(t), Some(r)) => same_type(ex, t, r),
                    _ => false,
                };
                let repl = ex.expand_call(
                    l.value,
                    &name,
                    &args,
                    Mode::Wire,
                    &Ctx::module(),
                    Some(whole),
                    None,
                );
                ex.replace(l.value, repl);
            } else {
                ex.rewrite(l.value, &Ctx::module());
            }
        }
        StmtKind::Assign(a) => {
            rewrite_lvalue(ex, &a.lhs, &Ctx::module());
            let target = a
                .lhs
                .suffixes
                .is_empty()
                .then(|| ex.decls.types.get(&a.lhs.base.text).copied())
                .flatten();
            match (whole_call(ex, a.rhs), target) {
                (Some((name, args)), Some(t)) => {
                    let whole = ex.fns[&name]
                        .decl
                        .return_ty
                        .is_some_and(|r| same_type(ex, t, r));
                    let repl = ex.expand_call(
                        a.rhs,
                        &name,
                        &args,
                        Mode::Wire,
                        &Ctx::module(),
                        Some(whole),
                        None,
                    );
                    ex.replace(a.rhs, repl);
                }
                _ => ex.rewrite(a.rhs, &Ctx::module()),
            }
        }
        StmtKind::Reg(r) => ex.rewrite(r.init, &Ctx::subst()),
        StmtKind::Instance(inst) => {
            for b in &inst.bindings {
                if let Some(v) = b.value {
                    ex.rewrite(v, &Ctx::module());
                }
            }
        }
        StmtKind::On(on) => rewrite_block(ex, on.body, Ctx::module()),
        StmtKind::Comb(b) => rewrite_block(ex, b, Ctx::subst()),
        StmtKind::Expr(e) => ex.rewrite(e, &Ctx::module()),
        StmtKind::Wire(_) | StmtKind::For(_) | StmtKind::Error => {}
    }
}

/// İfadenin kökü bir kullanıcı fn çağrısı mı (tüm sağ taraf).
fn whole_call(ex: &Expander<'_>, e: Idx<Expr>) -> Option<(String, Vec<Idx<Expr>>)> {
    let ExprKind::Call { callee, args } = &ex.out.exprs[e].kind else {
        return None;
    };
    let name = ex.user_fn(*callee, &Ctx::module())?;
    Some((name, args.clone()))
}

fn same_type(ex: &Expander<'_>, a: Idx<volt_ast::TypeRef>, b: Idx<volt_ast::TypeRef>) -> bool {
    matches!((canon(&ex.out, a), canon(&ex.out, b)), (Some(x), Some(y)) if x == y)
}

fn rewrite_lvalue(ex: &mut Expander<'_>, lv: &LValue, ctx: &Ctx) {
    for s in &lv.suffixes {
        match s {
            LValueSuffix::Index(e) => ex.rewrite(*e, ctx),
            LValueSuffix::Range { hi, lo } => {
                ex.rewrite(*hi, ctx);
                ex.rewrite(*lo, ctx);
            }
            LValueSuffix::PartSelect { start, width, .. } => {
                ex.rewrite(*start, ctx);
                ex.rewrite(*width, ctx);
            }
            LValueSuffix::Field(_) => {}
        }
    }
}

fn rewrite_block(ex: &mut Expander<'_>, block: Idx<Block>, mut ctx: Ctx) {
    let stmts = ex.out.blocks[block].stmts.clone();
    for st in &stmts {
        match st {
            BlockStmt::NonBlockAssign { lhs, rhs, .. }
            | BlockStmt::BlockAssign { lhs, rhs, .. } => {
                rewrite_lvalue(ex, lhs, &ctx);
                ex.rewrite(*rhs, &ctx);
            }
            BlockStmt::If(i) => rewrite_if(ex, i, &ctx),
            BlockStmt::Match(m) => {
                ex.rewrite(m.scrutinee, &ctx);
                for arm in &m.arms {
                    let mut arm_ctx = ctx.clone();
                    pattern_names(&ex.out, arm.pattern, &mut arm_ctx.locals);
                    if let Some(g) = arm.guard {
                        ex.rewrite(g, &arm_ctx);
                    }
                    match arm.body {
                        volt_ast::MatchArmBody::Expr(e) => ex.rewrite(e, &arm_ctx),
                        volt_ast::MatchArmBody::Block(b) => rewrite_block(ex, b, arm_ctx),
                    }
                }
            }
            BlockStmt::Let(l) => {
                ex.rewrite(l.value, &ctx);
                ctx.locals.push(l.name.text.clone());
            }
            BlockStmt::For(f) => {
                ex.rewrite(f.start, &ctx);
                ex.rewrite(f.end, &ctx);
                let mut inner = ctx.clone();
                inner.subst_only = true;
                inner.locals.push(f.var.text.clone());
                rewrite_block(ex, f.body, inner);
            }
            BlockStmt::Error => {}
        }
    }
    if let Some(t) = ex.out.blocks[block].tail {
        ex.rewrite(t, &ctx);
    }
}

fn rewrite_if(ex: &mut Expander<'_>, i: &IfStmt, ctx: &Ctx) {
    ex.rewrite(i.cond, ctx);
    rewrite_block(ex, i.then_block, ctx.clone());
    match &i.else_branch {
        Some(ElseBranch::Block(b)) => rewrite_block(ex, *b, ctx.clone()),
        Some(ElseBranch::If(inner)) => rewrite_if(ex, inner, ctx),
        None => {}
    }
}

/// Desenin bağladığı adlar (match kolu yerelleri).
fn pattern_names(ast: &SourceFile, p: Idx<Pattern>, out: &mut Vec<String>) {
    match &ast.patterns[p].kind {
        PatternKind::Binding(n) => out.push(n.text.clone()),
        PatternKind::Tuple(ps) | PatternKind::Or(ps) => {
            for &q in ps {
                pattern_names(ast, q, out);
            }
        }
        PatternKind::Path { args, .. } => match args {
            Some(PatternArgs::Tuple(ps)) => {
                for &q in ps {
                    pattern_names(ast, q, out);
                }
            }
            Some(PatternArgs::Struct(fields)) => {
                for f in fields {
                    match f.pattern {
                        Some(q) => pattern_names(ast, q, out),
                        None => out.push(f.name.text.clone()),
                    }
                }
            }
            None => {}
        },
        PatternKind::Wildcard | PatternKind::Literal(_) | PatternKind::Error => {}
    }
}

/// Karar 12.4 — doğrulama birimi: birimin modülleri çıkarılır; her
/// (generic olmayan, sonuçlu) fn için gövdesini fn kapsamında (portlar =
/// parametreler, `let`'ler = modül `let`'leri) süren bir sentetik modül
/// eklenir. Doğrulama fn'in KENDİ gövdesidir: gövdedeki her iç çağrı
/// çağrılanın dönüş tipinde bir giriş portuyla (`__c<j>`) temsil edilir,
/// argümanları parametre tipinde tellerle (`__a<j>_<m>`) ayrıca emit
/// edilir — iç içe açılım olmadığı için doğrulama gövde boyutunda
/// doğrusaldır (üstel çağrı ağacı çağrılanın kendi doğrulamasında kalır).
///
/// ```text
/// module __volt_fn_<fn> {
///     in <param> : T …   in __c0 : R(g) …
///     let <let> = …      let __a0_0 : P(g, 0) = <arg> …
///     out __r : R        __r = <son ifade>
/// }
/// ```
///
/// Dönüş: (birim, fn adı → tanım span'i).
pub(crate) fn validation_unit(ast: &SourceFile) -> Option<(SourceFile, Vec<(String, Span)>)> {
    let fns = collect_fns(ast);
    if fns.is_empty() {
        return None;
    }
    let mut out = ast.clone();
    out.items
        .retain(|&i| !matches!(out.items_arena[i].kind, ItemKind::Module(_)));
    let mut names: Vec<&String> = fns.keys().collect();
    names.sort_by_key(|n| fns[*n].item_span.start);
    let mut spans = Vec::new();
    for name in names {
        let info = &fns[name];
        let f = info.decl;
        let (Some(ret), Some(tail)) = (f.return_ty, info.tail(ast)) else {
            continue;
        };
        // İç çağrılar (kaynak sırasıyla); sonucu olmayan çağrılan (E2015)
        // temsil edilemez — bu fn doğrulanmaz, tanı çağrılanda.
        let calls = nested_calls(ast, info, &fns);
        if calls
            .iter()
            .any(|(_, g, _)| fns[g].decl.return_ty.is_none())
        {
            continue;
        }
        let span = f.name.span;
        let ident = |text: String| Name { text, span };
        let mut ports: Vec<Port> = f
            .params
            .iter()
            .map(|p| port(PortDir::In, p.name.clone(), p.ty, p.span))
            .collect();
        let mut body = Vec::new();
        let mut arg_lets = Vec::new();
        for (j, (call, g, args)) in calls.iter().enumerate() {
            let callee = fns[g].decl;
            let c = format!("__c{j}");
            if let Some(r) = callee.return_ty {
                ports.push(port(PortDir::In, ident(c.clone()), r, span));
            }
            for (m, (&a, p)) in args.iter().zip(&callee.params).enumerate() {
                arg_lets.push(let_stmt(
                    &mut out,
                    ident(format!("__a{j}_{m}")),
                    Some(p.ty),
                    a,
                ));
            }
            let call_span = out.exprs[*call].span;
            out.exprs[*call] = Expr {
                span: call_span,
                kind: ExprKind::Path(Path {
                    span: call_span,
                    segments: vec![Name {
                        text: c,
                        span: call_span,
                    }],
                }),
            };
        }
        for l in &info.lets {
            body.push(let_stmt(&mut out, l.name.clone(), l.ty, l.value));
        }
        body.extend(arg_lets);
        ports.push(port(PortDir::Out, ident("__r".to_string()), ret, span));
        let assign = out.stmts.alloc(Stmt {
            span,
            attrs: Vec::new(),
            kind: StmtKind::Assign(volt_ast::AssignStmt {
                lhs: LValue {
                    span,
                    base: ident("__r".to_string()),
                    suffixes: Vec::new(),
                },
                rhs: tail,
            }),
        });
        body.push(assign);
        let module = ModuleDecl {
            name: ident(format!("{VALIDATION_PREFIX}{name}")),
            generics: Vec::new(),
            ports,
            contracts: Vec::new(),
            body,
            closing_name: None,
            mmio_regs: Vec::new(),
        };
        let item = out.items_arena.alloc(Item {
            span,
            attrs: Vec::new(),
            doc: None,
            visibility: Visibility::Private,
            kind: ItemKind::Module(module),
        });
        out.items.push(item);
        spans.push((name.clone(), info.item_span));
    }
    Some((out, spans))
}

/// Gövdedeki kullanıcı fn çağrıları: (çağrı, çağrılan, argümanlar).
fn nested_calls(
    ast: &SourceFile,
    info: &FnInfo<'_>,
    fns: &HashMap<String, FnInfo<'_>>,
) -> Vec<(Idx<Expr>, String, Vec<Idx<Expr>>)> {
    let mut roots: Vec<Idx<Expr>> = info.lets.iter().map(|l| l.value).collect();
    roots.extend(info.tail(ast));
    let mut out = Vec::new();
    for root in roots {
        volt_ast::visit::walk_expr(ast, root, |e| {
            let ExprKind::Call { callee, args } = &ast.exprs[e].kind else {
                return;
            };
            let ExprKind::Path(p) = &ast.exprs[*callee].kind else {
                return;
            };
            let global = matches!(
                info.bindings.get(callee),
                None | Some(scope::Binding::Global)
            );
            if let [seg] = p.segments.as_slice() {
                if global && fns.contains_key(&seg.text) {
                    out.push((e, seg.text.clone(), args.clone()));
                }
            }
        });
    }
    out
}

fn let_stmt(
    out: &mut SourceFile,
    name: Name,
    ty: Option<Idx<volt_ast::TypeRef>>,
    value: Idx<Expr>,
) -> Idx<Stmt> {
    let span = name.span;
    out.stmts.alloc(Stmt {
        span,
        attrs: Vec::new(),
        kind: StmtKind::Let(volt_ast::LetDecl { name, ty, value }),
    })
}

fn port(direction: PortDir, name: Name, ty: Idx<volt_ast::TypeRef>, span: Span) -> Port {
    Port {
        span,
        attrs: Vec::new(),
        doc: None,
        direction,
        name,
        ty,
        domain: None,
        bundle: None,
    }
}
