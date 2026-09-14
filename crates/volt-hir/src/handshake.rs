//! Handshake<T> protokol denetimi (ADR-0050): E4007.
//!
//! Üretici tarafta (`out tx : Handshake<T>`) `tx.valid`, `tx.ready`'ye
//! KOMBİNASYONEL bağımlı olamaz. AXI'nin (A3.3.1) ve Volt Handshake
//! kontratının temel kuralı: üretici valid'i yükseltmek için ready'yi
//! beklemez — iki taraf da karşısını beklerse el sıkışma hiç
//! tamamlanmaz (kilitlenme). Tüketicinin ready'si valid'e bağlı
//! olabilir; bu yüzden yalnız üretici portları denetlenir.
//!
//! Denetim modül gövdesindeki sürekli atamaları (`x = e`), `let`
//! bağlarını ve `comb` bloklarını (koşullar dâhil) bir bağımlılık
//! grafiğine çevirir; `on` blokları register olduğundan yolu keser.
//! Örnek çıkışları (`inst.port`) opaktır: örneğin içinden geçen yol
//! görülmez (bkz. ADR-0050 sınırlar).

use std::collections::{HashMap, HashSet};

use volt_ast::{
    ArrayLitKind, Block, BlockStmt, ElseBranch, Expr, ExprKind, Idx, IfStmt, ItemKind,
    MatchArmBody, ModuleDecl, PortDir, SourceFile, StmtKind,
};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use crate::resolve::{DefId, ResolveResult};

const HANDSHAKE: &str = "Handshake";
const FIELD_VALID: &str = "valid";
const FIELD_READY: &str = "ready";

/// Bir sinyalin kombinasyonel bağımlılığı: hangi tanımdan, nerede
/// okunarak, hangi deyimle.
#[derive(Debug, Clone, Copy)]
struct Dep {
    def: DefId,
    /// Okuma konumu (`ready` burada okunuyor).
    site: Span,
    /// Sürücü deyimin konumu (`valid` burada sürülüyor).
    stmt: Span,
}

#[derive(Default)]
struct CombGraph {
    edges: HashMap<DefId, Vec<Dep>>,
}

/// Her üretici Handshake portu için E4007 denetimi.
pub fn check_handshakes(ast: &SourceFile, res: &ResolveResult) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for &item in &ast.items {
        let ItemKind::Module(m) = &ast.items_arena[item].kind else {
            continue;
        };
        let mut valids: Vec<(DefId, String, Span)> = Vec::new();
        let mut readys: HashMap<String, DefId> = HashMap::new();
        for p in &m.ports {
            let Some(origin) = &p.bundle else { continue };
            if origin.bundle != HANDSHAKE {
                continue;
            }
            let Some(&def) = res.decl_spans.get(&p.name.span) else {
                continue;
            };
            match (origin.path.as_str(), p.direction) {
                (FIELD_VALID, PortDir::Out) => {
                    valids.push((def, origin.port.text.clone(), origin.port.span));
                }
                (FIELD_READY, PortDir::In) => {
                    readys.insert(origin.port.text.clone(), def);
                }
                _ => {}
            }
        }
        if valids.is_empty() {
            continue;
        }
        let graph = CombGraph::build(ast, res, m);
        for (valid, port, port_span) in valids {
            let Some(&ready) = readys.get(&port) else {
                continue;
            };
            if let Some(chain) = graph.find_path(valid, ready) {
                diags.push(e4007(res, &port, port_span, &chain));
            }
        }
    }
    diags
}

/// E4007 — beş parça: kod, konum (valid'in sürücüsü), açıklama, öneri,
/// gerekçe notu (ADR-0050) + ikincil etiketler (ready okuması, port).
fn e4007(res: &ResolveResult, port: &str, port_span: Span, chain: &[Dep]) -> Diagnostic {
    let first = chain[0];
    let last = chain[chain.len() - 1];
    let names: Vec<String> = std::iter::once(format!("{port}.valid"))
        .chain(
            chain
                .iter()
                .map(|d| res.defs[d.def.0 as usize].name.clone()),
        )
        .collect();
    let path = names.join(" <- ");
    Diagnostic::error(
        ErrorCode::E4007,
        lstr!(en: "handshake '{port}': valid depends combinationally on ready";
              tr: "'{port}' el sıkışması: valid, ready'ye kombinasyonel bağımlı"),
        LabeledSpan::primary(
            first.stmt,
            lstr!(en: "valid is driven here"; tr: "valid burada sürülüyor"),
        ),
        lstr!(en: "decide valid from registered state (reg valid_r : bool = false; on clk {{ ... }}; {port}.valid = valid_r); the consumer may derive ready from valid, the producer must not derive valid from ready";
              tr: "valid'i register'lanmış durumdan üretin (reg valid_r : bool = false; on clk {{ ... }}; {port}.valid = valid_r); tüketici ready'yi valid'den türetebilir, üretici valid'i ready'den türetemez"),
    )
    .with_secondary(
        last.site,
        lstr!(en: "ready is read here"; tr: "ready burada okunuyor"),
    )
    .with_secondary(
        port_span,
        lstr!(en: "handshake port declared here"; tr: "el sıkışma portu burada bildirildi"),
    )
    .with_note(
        NoteKind::Reason,
        lstr!(en: "combinational path: {path}. A producer that waits for ready before raising valid deadlocks against a consumer that waits for valid (ADR-0050, AXI A3.3.1)";
              tr: "kombinasyonel yol: {path}. valid'i yükseltmek için ready'yi bekleyen üretici, valid'i bekleyen tüketiciyle kilitlenir (ADR-0050, AXI A3.3.1)"),
    )
}

impl CombGraph {
    fn build(ast: &SourceFile, res: &ResolveResult, m: &ModuleDecl) -> Self {
        let mut g = CombGraph::default();
        let mut w = Walker {
            ast,
            res,
            graph: &mut g,
        };
        for &si in &m.body {
            let stmt = &ast.stmts[si];
            match &stmt.kind {
                StmtKind::Assign(a) => {
                    if let Some(&target) = res.use_spans.get(&a.lhs.base.span) {
                        w.add(target, a.rhs, &[], stmt.span);
                    }
                }
                StmtKind::Let(l) => {
                    if let Some(&target) = res.decl_spans.get(&l.name.span) {
                        w.add(target, l.value, &[], stmt.span);
                    }
                }
                StmtKind::Comb(b) => w.block(*b, &mut Vec::new()),
                // Register sınırı: `on` blokları ve örnekler yolu keser.
                StmtKind::On(_)
                | StmtKind::Instance(_)
                | StmtKind::Reg(_)
                | StmtKind::Wire(_)
                | StmtKind::For(_)
                | StmtKind::Expr(_)
                | StmtKind::Error => {}
            }
        }
        g
    }

    /// `from`dan `to`ya kombinasyonel yol (DFS); bulunursa kenar zinciri.
    fn find_path(&self, from: DefId, to: DefId) -> Option<Vec<Dep>> {
        let mut visited: HashSet<DefId> = HashSet::new();
        let mut chain: Vec<Dep> = Vec::new();
        if self.dfs(from, to, &mut visited, &mut chain) {
            Some(chain)
        } else {
            None
        }
    }

    fn dfs(
        &self,
        cur: DefId,
        to: DefId,
        visited: &mut HashSet<DefId>,
        chain: &mut Vec<Dep>,
    ) -> bool {
        if !visited.insert(cur) {
            return false;
        }
        let Some(deps) = self.edges.get(&cur) else {
            return false;
        };
        for dep in deps {
            chain.push(*dep);
            if dep.def == to || self.dfs(dep.def, to, visited, chain) {
                return true;
            }
            chain.pop();
        }
        false
    }
}

/// Gövde gezici: ifadelerdeki okumaları toplar, koşul bağlamını taşır.
struct Walker<'a> {
    ast: &'a SourceFile,
    res: &'a ResolveResult,
    graph: &'a mut CombGraph,
}

impl Walker<'_> {
    /// `target`'ın sürücüsü `rhs` (+ çevreleyen koşullar).
    fn add(&mut self, target: DefId, rhs: Idx<Expr>, conds: &[(DefId, Span)], stmt: Span) {
        let mut reads = Vec::new();
        self.reads(rhs, &mut reads);
        let deps = self.graph.edges.entry(target).or_default();
        for (def, site) in reads.into_iter().chain(conds.iter().copied()) {
            deps.push(Dep { def, site, stmt });
        }
    }

    /// `comb` bloğu: atamalar koşullara da bağımlıdır.
    fn block(&mut self, b: Idx<Block>, conds: &mut Vec<(DefId, Span)>) {
        let block = &self.ast.blocks[b];
        for bs in &block.stmts {
            match bs {
                BlockStmt::BlockAssign { lhs, rhs, span }
                | BlockStmt::NonBlockAssign { lhs, rhs, span } => {
                    if let Some(&target) = self.res.use_spans.get(&lhs.base.span) {
                        self.add(target, *rhs, conds, *span);
                    }
                }
                BlockStmt::Let(l) => {
                    if let Some(&target) = self.res.decl_spans.get(&l.name.span) {
                        self.add(target, l.value, conds, self.ast.exprs[l.value].span);
                    }
                }
                BlockStmt::If(ifs) => self.if_stmt(ifs, conds),
                BlockStmt::Match(m) => {
                    let mark = conds.len();
                    self.reads(m.scrutinee, conds);
                    for arm in &m.arms {
                        let inner = conds.len();
                        if let Some(g) = arm.guard {
                            self.reads(g, conds);
                        }
                        match arm.body {
                            MatchArmBody::Block(ab) => self.block(ab, conds),
                            MatchArmBody::Expr(_) => {}
                        }
                        conds.truncate(inner);
                    }
                    conds.truncate(mark);
                }
                BlockStmt::For(f) => self.block(f.body, conds),
                BlockStmt::Error => {}
            }
        }
    }

    fn if_stmt(&mut self, ifs: &IfStmt, conds: &mut Vec<(DefId, Span)>) {
        let mark = conds.len();
        self.reads(ifs.cond, conds);
        self.block(ifs.then_block, conds);
        match &ifs.else_branch {
            Some(ElseBranch::Block(b)) => self.block(*b, conds),
            Some(ElseBranch::If(inner)) => self.if_stmt(inner, conds),
            None => {}
        }
        conds.truncate(mark);
    }

    /// İfadedeki tüm çözümlenmiş isim okumaları (konumlarıyla).
    fn reads(&self, e: Idx<Expr>, out: &mut Vec<(DefId, Span)>) {
        let expr = &self.ast.exprs[e];
        match &expr.kind {
            ExprKind::Path(_) => {
                if let Some(&def) = self.res.resolutions.get(&e) {
                    out.push((def, expr.span));
                }
            }
            ExprKind::Binary { lhs, rhs, .. } => {
                self.reads(*lhs, out);
                self.reads(*rhs, out);
            }
            ExprKind::Unary { operand, .. } => self.reads(*operand, out),
            ExprKind::Index { base, index } => {
                self.reads(*base, out);
                self.reads(*index, out);
            }
            ExprKind::Range { base, hi, lo } => {
                self.reads(*base, out);
                self.reads(*hi, out);
                self.reads(*lo, out);
            }
            ExprKind::PartSelect {
                base, start, width, ..
            } => {
                self.reads(*base, out);
                self.reads(*start, out);
                self.reads(*width, out);
            }
            // Örnek çıkışı (`inst.port`) opaktır: taban örnek tanımıdır,
            // ready'ye yol vermez.
            ExprKind::Field { base, .. } => self.reads(*base, out),
            ExprKind::Call { callee, args } => {
                self.reads(*callee, out);
                for a in args {
                    self.reads(*a, out);
                }
            }
            ExprKind::Cast { expr, .. } => self.reads(*expr, out),
            ExprKind::If {
                cond,
                then_expr,
                else_expr,
            } => {
                self.reads(*cond, out);
                self.reads(*then_expr, out);
                self.reads(*else_expr, out);
            }
            ExprKind::Match { scrutinee, arms } => {
                self.reads(*scrutinee, out);
                for arm in arms {
                    if let Some(g) = arm.guard {
                        self.reads(g, out);
                    }
                    match arm.body {
                        MatchArmBody::Expr(ae) => self.reads(ae, out),
                        MatchArmBody::Block(ab) => self.block_reads(ab, out),
                    }
                }
            }
            ExprKind::StructLit { fields, .. } => {
                for f in fields {
                    if let Some(v) = f.value {
                        self.reads(v, out);
                    }
                }
            }
            ExprKind::ArrayLit(ArrayLitKind::List(items)) | ExprKind::TupleLit(items) => {
                for i in items {
                    self.reads(*i, out);
                }
            }
            ExprKind::ArrayLit(ArrayLitKind::Repeat { value, count }) => {
                self.reads(*value, out);
                self.reads(*count, out);
            }
            ExprKind::IntLit { .. }
            | ExprKind::BoolLit(_)
            | ExprKind::StringLit(_)
            | ExprKind::Todo { .. }
            | ExprKind::Error => {}
        }
    }

    /// İfade konumundaki blok (match kolu gövdesi): deyim sağ tarafları
    /// ve kuyruk ifadesi.
    fn block_reads(&self, b: Idx<Block>, out: &mut Vec<(DefId, Span)>) {
        let block = &self.ast.blocks[b];
        for bs in &block.stmts {
            match bs {
                BlockStmt::BlockAssign { rhs, .. } | BlockStmt::NonBlockAssign { rhs, .. } => {
                    self.reads(*rhs, out)
                }
                BlockStmt::Let(l) => self.reads(l.value, out),
                BlockStmt::If(ifs) => {
                    self.reads(ifs.cond, out);
                    self.block_reads(ifs.then_block, out);
                    let mut eb = ifs.else_branch.as_ref();
                    while let Some(branch) = eb {
                        match branch {
                            ElseBranch::Block(b) => {
                                self.block_reads(*b, out);
                                eb = None;
                            }
                            ElseBranch::If(inner) => {
                                self.reads(inner.cond, out);
                                self.block_reads(inner.then_block, out);
                                eb = inner.else_branch.as_ref();
                            }
                        }
                    }
                }
                BlockStmt::Match(m) => {
                    self.reads(m.scrutinee, out);
                    for arm in &m.arms {
                        if let Some(g) = arm.guard {
                            self.reads(g, out);
                        }
                        match arm.body {
                            MatchArmBody::Expr(ae) => self.reads(ae, out),
                            MatchArmBody::Block(ab) => self.block_reads(ab, out),
                        }
                    }
                }
                BlockStmt::For(f) => self.block_reads(f.body, out),
                BlockStmt::Error => {}
            }
        }
        if let Some(t) = block.tail {
            self.reads(t, out);
        }
    }
}
