//! Modül özeti (ADR-0114): örnek sınırında kombinasyonel bağımlılık.
//!
//! Her modül ve extern için bir kez, çocuktan başlayarak: hangi çıkış
//! portu hangi giriş portuna aralarında register olmadan bağlıdır. Örnek
//! sitesi çocuğun içini görmeden bu özeti kullanır:
//!
//! * saat alanı (domain-inference.md K8b): saatsiz hedefin anotasyonsuz
//!   çıkışı, bağlı olduğu girişlere bağlanan değerlerin K5 join'ini taşır;
//! * el sıkışma (E4007, ADR-0050 §4): `valid`'den `ready`'ye yol örnek
//!   çıkışlarından özet üzerinden geçer.
//!
//! Tek graf kurucu ([`CombGraph`]) ikisinin de kaynağıdır. Kenarlar:
//! sürekli atama, `let`, `comb` bloğu ve modül düzeyi `for` gövdesi;
//! koşul, `match` sınananı, kol muhafızı ve hedefin indeks, aralık ve
//! part-select ifadeleri işlenendir (K5); fonksiyon çağrısının
//! argümanları işlenendir (ADR-0081). `on` bloğu, register ve
//! `sync()`/`sync3()` yolu keser. Örnek çıkışı okuması (`inst.port`),
//! çocuğun özetinde o çıkışın bağlı olduğu girişlere bağlanan ifadelerin
//! okumasıdır; özeti olmayan hedef opaktır.
//!
//! Saat portu çözümlenmiş türünden anlaşılır (`type Clk = clock` de saattir,
//! ADR-0070), alan çıkarımıyla aynı kural.
//!
//! Özet kaynakları:
//! * Volt modülü (saatli ya da saatsiz): gövdeden hesaplanır. Generic
//!   modülün her açılımı (ADR-0041) ayrı bir modüldür, ayrı özet alır.
//! * Saatsiz extern: içi görünmez; anotasyonsuz ya da anotasyonlu her
//!   çıkış her girişe bağlı sayılır (en kötü durum, K4).
//! * Saatli extern: özet boştur (K5): çıkışları saat haritasından
//!   alanını alır, E4007 onun içinden geçen yolu görmez.
//! * Yerleşik primitif: [`volt_ast::builtin::BuiltinPrim::comb_inputs`] tablosu (K7).

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use volt_ast::{
    ArrayLitKind, Block, BlockStmt, ElseBranch, Expr, ExprKind, Idx, IfStmt, ItemKind, LValue,
    LValueSuffix, MatchArmBody, ModuleDecl, Port, PortDir, SourceFile, StmtKind,
};
use volt_span::Span;

use crate::resolve::{BuiltinKind, DefId, DefKind, ResolveResult};
use crate::ty::Ty;
use crate::typeck::TypeckResult;

/// Bir modülün ya da extern'in özeti.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    /// Çıkış portu → kombinasyonel bağlı olduğu giriş portları.
    pub comb: BTreeMap<String, BTreeSet<String>>,
    /// `comb`'un yalnız bir saatsiz extern'in en kötü durum varsayımıyla
    /// (K4) kurulan çiftleri: çıkış → o girişler. Yalnız tanının notu
    /// için: varsayımla kurulan yol da E4007'dir, not bunu söyler.
    pub assumed: BTreeMap<String, BTreeSet<String>>,
    /// Hedef gövdesi görünmeyen saatsiz bir extern: `comb` en kötü durum
    /// varsayımıdır (K4). Volt modülünde her zaman `false` (saatsiz bir
    /// Volt modülünün özeti gövdesinden hesaplanır).
    pub worst_case: bool,
}

/// Birimin bütün modül ve extern özetleri; birim başına bir kez
/// hesaplanır ve alan çıkarımıyla E4007'ye aynı nesne verilir.
#[derive(Debug, Default)]
pub struct Summaries {
    by_def: HashMap<DefId, Summary>,
}

/// Örnek sitesinde bir çıkışın arkası: hedefin adı ve bağlı girişler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputDeps {
    /// Hedef modülün, extern'in ya da yerleşik primitifin adı.
    pub target: String,
    pub inputs: Vec<String>,
    /// `inputs`'tan yalnız varsayımla bağlı olanlar (K4).
    pub assumed: BTreeSet<String>,
    /// Hedef saatsiz bir extern: bağımlılık en kötü durum varsayımıdır.
    pub worst_case: bool,
}

impl Summaries {
    /// Birimdeki her modülün ve extern'in özeti, çocuktan başlayarak.
    pub fn compute(ast: &SourceFile, res: &ResolveResult, tyck: &TypeckResult) -> Self {
        let mut s = Summaries::default();
        let clk = Clocks { res, tyck };
        let mut busy = HashSet::new();
        for &item in &ast.items {
            let def = match &ast.items_arena[item].kind {
                ItemKind::Module(m) => res.decl_spans.get(&m.name.span),
                ItemKind::Extern(x) => res.decl_spans.get(&x.name.span),
                ItemKind::Domain(_)
                | ItemKind::Fn(_)
                | ItemKind::Struct(_)
                | ItemKind::Enum(_)
                | ItemKind::Const(_)
                | ItemKind::TypeAlias(_)
                | ItemKind::Test(_)
                | ItemKind::Error => None,
            };
            if let Some(&def) = def {
                s.summarize(ast, res, &clk, def, &mut busy);
            }
        }
        s
    }

    /// Hedef tanımının (modül ya da extern) özeti.
    pub fn get(&self, def: DefId) -> Option<&Summary> {
        self.by_def.get(&def)
    }

    /// `inst.port` okumasının bağlı olduğu hedef girişleri. Örnek
    /// çözülmemişse, hedefin özeti yoksa ya da port çıkış değilse `None`
    /// (opak: yol yok).
    pub fn output_deps(&self, res: &ResolveResult, inst: DefId, port: &str) -> Option<OutputDeps> {
        if let Some(prim) = res.instance_builtin.get(&inst) {
            prim.port(port).filter(|p| p.dir == PortDir::Out)?;
            return Some(OutputDeps {
                target: prim.name().to_string(),
                inputs: prim
                    .comb_inputs(port)
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                assumed: BTreeSet::new(),
                worst_case: false,
            });
        }
        let target = *res.instance_module.get(&inst)?;
        let sum = self.by_def.get(&target)?;
        let ins = sum.comb.get(port)?;
        Some(OutputDeps {
            target: res.defs[target.0 as usize].name.clone(),
            inputs: ins.iter().cloned().collect(),
            assumed: sum.assumed.get(port).cloned().unwrap_or_default(),
            worst_case: sum.worst_case,
        })
    }

    fn summarize(
        &mut self,
        ast: &SourceFile,
        res: &ResolveResult,
        clk: &Clocks<'_>,
        def: DefId,
        busy: &mut HashSet<DefId>,
    ) {
        if self.by_def.contains_key(&def) || busy.contains(&def) {
            return;
        }
        let Some(&item) = res.item_of_def.get(&def) else {
            return;
        };
        let sum = match &ast.items_arena[item].kind {
            ItemKind::Module(m) => {
                // Çocuklar önce; örnekleme döngüsü (E1006) `busy` ile
                // kesilir, döngüdeki hedef opak kalır.
                busy.insert(def);
                for &si in &m.body {
                    let StmtKind::Instance(inst) = &ast.stmts[si].kind else {
                        continue;
                    };
                    let child = res
                        .decl_spans
                        .get(&inst.name.span)
                        .and_then(|d| res.instance_module.get(d));
                    if let Some(&child) = child {
                        self.summarize(ast, res, clk, child, busy);
                    }
                }
                busy.remove(&def);
                self.summarize_module(ast, res, clk, m)
            }
            ItemKind::Extern(x) => extern_summary(clk, &x.ports),
            ItemKind::Domain(_)
            | ItemKind::Fn(_)
            | ItemKind::Struct(_)
            | ItemKind::Enum(_)
            | ItemKind::Const(_)
            | ItemKind::TypeAlias(_)
            | ItemKind::Test(_)
            | ItemKind::Error => return,
        };
        self.by_def.insert(def, sum);
    }

    fn summarize_module(
        &self,
        ast: &SourceFile,
        res: &ResolveResult,
        clk: &Clocks<'_>,
        m: &ModuleDecl,
    ) -> Summary {
        let inputs: HashMap<DefId, String> = m
            .ports
            .iter()
            .filter(|p| p.direction == PortDir::In && !clk.is_clock_port(p))
            .filter_map(|p| Some((*res.decl_spans.get(&p.name.span)?, p.name.text.clone())))
            .collect();
        let graph = CombGraph::build(ast, res, m, self);
        let mut comb = BTreeMap::new();
        let mut assumed = BTreeMap::new();
        let names = |set: HashSet<DefId>| -> BTreeSet<String> {
            set.into_iter()
                .filter_map(|d| inputs.get(&d).cloned())
                .collect()
        };
        for p in m.ports.iter().filter(|p| p.direction == PortDir::Out) {
            let Some(&out) = res.decl_spans.get(&p.name.span) else {
                continue;
            };
            let all = names(graph.reachable(out, |_| true));
            let certain = names(graph.reachable(out, |d| !d.is_assumed()));
            let only_assumed: BTreeSet<String> = all.difference(&certain).cloned().collect();
            if !only_assumed.is_empty() {
                assumed.insert(p.name.text.clone(), only_assumed);
            }
            comb.insert(p.name.text.clone(), all);
        }
        Summary {
            comb,
            assumed,
            worst_case: false,
        }
    }
}

/// Extern'in özeti: saatsizse en kötü durum (her çıkış her girişe bağlı,
/// K4), saatliyse boş (K5).
fn extern_summary(clk: &Clocks<'_>, ports: &[Port]) -> Summary {
    let clockless = !ports.iter().any(|p| clk.is_clock_port(p));
    let inputs: BTreeSet<String> = if clockless {
        ports
            .iter()
            .filter(|p| p.direction == PortDir::In)
            .map(|p| p.name.text.clone())
            .collect()
    } else {
        BTreeSet::new()
    };
    let comb: BTreeMap<String, BTreeSet<String>> = ports
        .iter()
        .filter(|p| p.direction == PortDir::Out)
        .map(|p| (p.name.text.clone(), inputs.clone()))
        .collect();
    // Saatsiz extern'in bütün bağımlılıkları varsayımdır.
    let assumed = if clockless {
        comb.clone()
    } else {
        BTreeMap::new()
    };
    Summary {
        comb,
        assumed,
        worst_case: clockless,
    }
}

/// Saat portu: çözümlenmiş türü `clock` olan port (takma ad dahil,
/// ADR-0070); alan çıkarımının `is_clock_port`'uyla aynı kural.
struct Clocks<'a> {
    res: &'a ResolveResult,
    tyck: &'a TypeckResult,
}

impl Clocks<'_> {
    fn is_clock_port(&self, p: &Port) -> bool {
        self.res
            .decl_spans
            .get(&p.name.span)
            .and_then(|d| self.tyck.def_types.get(d))
            .is_some_and(|&t| matches!(self.tyck.types.ty(t), Ty::Clock))
    }
}

/// Bir örnek çıkışının içinden geçen adım: E4007 yolunda
/// `g.y (AndGate: y <- b)` biçiminde yazılır.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hop {
    /// Örnek adı (`g`).
    pub inst: String,
    /// Okunan çıkış (`y`).
    pub output: String,
    /// Hedefin adı (`AndGate`).
    pub target: String,
    /// Çıkışın bağlı olduğu giriş (`b`).
    pub input: String,
    /// Hedef saatsiz bir extern (en kötü durum varsayımı, K4).
    pub worst_case: bool,
    /// Bu adım (ya da içindeki bir adım) yalnız saatsiz extern
    /// varsayımına dayanıyor.
    pub assumed: bool,
}

/// Kombinasyonel bağımlılık: hangi tanımdan, nerede okunarak, hangi
/// deyimle; yol bir örneğin içinden geçiyorsa o adım.
#[derive(Debug, Clone)]
pub struct Dep {
    pub def: DefId,
    /// Okuma konumu.
    pub site: Span,
    /// Sürücü deyimin konumu.
    pub stmt: Span,
    pub hop: Option<Hop>,
}

impl Dep {
    /// Kenar yalnız saatsiz extern varsayımına dayanıyor (K4).
    pub fn is_assumed(&self) -> bool {
        self.hop.as_ref().is_some_and(|h| h.assumed)
    }
}

/// Bir okuma: tanım, konum ve varsa örnek adımı.
type Read = (DefId, Span, Option<Hop>);

/// Modül gövdesinin kombinasyonel bağımlılık grafiği: sürülen tanım →
/// bağımlılıkları.
#[derive(Debug, Default)]
pub struct CombGraph {
    pub edges: HashMap<DefId, Vec<Dep>>,
    /// Kurulumda açılan (örnek, çıkış) sayısı: örnek zincirinde açılımın
    /// aşama sayısıyla doğrusal kaldığını sınamak için.
    pub expansions: usize,
}

/// Örnek sitesinde bağlanan değer.
#[derive(Clone, Copy)]
enum Bound {
    Expr(Idx<Expr>),
    /// `clk` kısayolu: yerel ad port adıyla aynı.
    Def(DefId, Span),
}

impl CombGraph {
    pub fn build(ast: &SourceFile, res: &ResolveResult, m: &ModuleDecl, sums: &Summaries) -> Self {
        let mut g = CombGraph::default();
        let mut bindings: HashMap<DefId, HashMap<String, Bound>> = HashMap::new();
        for &si in &m.body {
            let StmtKind::Instance(inst) = &ast.stmts[si].kind else {
                continue;
            };
            let Some(&idef) = res.decl_spans.get(&inst.name.span) else {
                continue;
            };
            let mut map = HashMap::new();
            for b in &inst.bindings {
                let v = match b.value {
                    Some(e) => Bound::Expr(e),
                    None => match res.use_spans.get(&b.port_name.span) {
                        Some(&d) => Bound::Def(d, b.port_name.span),
                        None => continue,
                    },
                };
                map.insert(b.port_name.text.clone(), v);
            }
            bindings.insert(idef, map);
        }
        let mut w = Walker {
            ast,
            res,
            sums,
            bindings: &bindings,
            graph: &mut g,
            expanding: RefCell::new(HashSet::new()),
            expanded: RefCell::new(HashMap::new()),
            cuts: Cell::new(0),
            expansions: Cell::new(0),
        };
        for &si in &m.body {
            let stmt = &ast.stmts[si];
            match &stmt.kind {
                StmtKind::Assign(a) => {
                    if let Some(&target) = res.use_spans.get(&a.lhs.base.span) {
                        let mut ops = Vec::new();
                        w.target_reads(&a.lhs, &mut ops);
                        w.add(target, a.rhs, &ops, stmt.span);
                    }
                }
                StmtKind::Let(l) => {
                    if let Some(&target) = res.decl_spans.get(&l.name.span) {
                        w.add(target, l.value, &[], stmt.span);
                    }
                }
                // Modül düzeyi `for` parser'da açılır (ADR-0056); kalan bir
                // gövde `comb` gibi kombinasyoneldir.
                StmtKind::Comb(b) => w.block(*b, &mut Vec::new()),
                StmtKind::For(f) => w.block(f.body, &mut Vec::new()),
                // Register sınırı: `on` blokları yolu keser. Örneğin
                // çıkışları okundukları yerde özetle açılır.
                StmtKind::On(_)
                | StmtKind::Instance(_)
                | StmtKind::Reg(_)
                | StmtKind::Wire(_)
                | StmtKind::Expr(_)
                | StmtKind::Error => {}
            }
        }
        let expansions = w.expansions.get();
        g.expansions = expansions;
        g
    }

    /// `from`dan, `keep`in kabul ettiği kenarlardan erişilebilen bütün
    /// tanımlar (`from` hariç).
    pub fn reachable(&self, from: DefId, keep: impl Fn(&Dep) -> bool) -> HashSet<DefId> {
        let mut seen = HashSet::new();
        let mut stack = vec![from];
        while let Some(d) = stack.pop() {
            if let Some(deps) = self.edges.get(&d) {
                for dep in deps.iter().filter(|d| keep(d)) {
                    if seen.insert(dep.def) {
                        stack.push(dep.def);
                    }
                }
            }
        }
        seen
    }

    /// `from`dan `to`ya, `keep`in kabul ettiği kenarlardan kombinasyonel
    /// yol (DFS); bulunursa kenar zinciri.
    pub fn find_path(
        &self,
        from: DefId,
        to: DefId,
        keep: &dyn Fn(&Dep) -> bool,
    ) -> Option<Vec<Dep>> {
        let mut visited: HashSet<DefId> = HashSet::new();
        let mut chain: Vec<Dep> = Vec::new();
        if self.dfs(from, to, keep, &mut visited, &mut chain) {
            Some(chain)
        } else {
            None
        }
    }

    fn dfs(
        &self,
        cur: DefId,
        to: DefId,
        keep: &dyn Fn(&Dep) -> bool,
        visited: &mut HashSet<DefId>,
        chain: &mut Vec<Dep>,
    ) -> bool {
        if !visited.insert(cur) {
            return false;
        }
        let Some(deps) = self.edges.get(&cur) else {
            return false;
        };
        for dep in deps.iter().filter(|d| keep(d)) {
            chain.push(dep.clone());
            if dep.def == to || self.dfs(dep.def, to, keep, visited, chain) {
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
    sums: &'a Summaries,
    bindings: &'a HashMap<DefId, HashMap<String, Bound>>,
    graph: &'a mut CombGraph,
    /// Açılmakta olan (örnek, çıkış) çiftleri: örnekler üzerinden kurulan
    /// kombinasyonel döngü açılımı sonsuz kılmaz.
    expanding: RefCell<HashSet<(DefId, String)>>,
    /// Açılmış (örnek, çıkış) çiftlerinin okumaları: her çift bir kez
    /// açılır. Aksi hâlde her aşaması öncekinin iki çıkışını okuyan bir
    /// zincir 2^N açılım yapar.
    expanded: RefCell<HashMap<(DefId, String), Vec<Read>>>,
    /// Döngü kesimi sayacı: açılımı sırasında bir döngü kesilen çift
    /// eksik okumayla saklanmaz (başka bir girişten okununca tam açılır).
    cuts: Cell<usize>,
    /// Açılan (örnek, çıkış) sayısı ([`CombGraph::expansions`]).
    expansions: Cell<usize>,
}

impl Walker<'_> {
    /// Hedefin indeks, aralık ve part-select ifadelerinin okumaları: `sel`
    /// değişince `y[sel] = d`'de `d`'yi taşıyan bit değişir, `y` `sel`'e de
    /// kombinasyonel bağlıdır (K5).
    fn target_reads(&self, lhs: &LValue, out: &mut Vec<Read>) {
        for suffix in &lhs.suffixes {
            match suffix {
                LValueSuffix::Index(e) => self.reads(*e, out),
                LValueSuffix::Range { hi, lo } => {
                    self.reads(*hi, out);
                    self.reads(*lo, out);
                }
                LValueSuffix::PartSelect { start, width, .. } => {
                    self.reads(*start, out);
                    self.reads(*width, out);
                }
                LValueSuffix::Field(_) => {}
            }
        }
    }

    /// `target`'ın sürücüsü `rhs` (+ çevreleyen koşullar ve hedefin
    /// indeksleri).
    fn add(&mut self, target: DefId, rhs: Idx<Expr>, conds: &[Read], stmt: Span) {
        let mut reads = Vec::new();
        self.reads(rhs, &mut reads);
        let deps = self.graph.edges.entry(target).or_default();
        for (def, site, hop) in reads.into_iter().chain(conds.iter().cloned()) {
            deps.push(Dep {
                def,
                site,
                stmt,
                hop,
            });
        }
    }

    /// `comb` bloğu: atamalar koşullara da bağımlıdır.
    fn block(&mut self, b: Idx<Block>, conds: &mut Vec<Read>) {
        let block = &self.ast.blocks[b];
        for bs in &block.stmts {
            match bs {
                BlockStmt::BlockAssign { lhs, rhs, span }
                | BlockStmt::NonBlockAssign { lhs, rhs, span } => {
                    if let Some(&target) = self.res.use_spans.get(&lhs.base.span) {
                        let mut ops = conds.clone();
                        self.target_reads(lhs, &mut ops);
                        self.add(target, *rhs, &ops, *span);
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

    fn if_stmt(&mut self, ifs: &IfStmt, conds: &mut Vec<Read>) {
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

    /// `inst.port` okuması: çocuğun özetinde `port`un bağlı olduğu
    /// girişlere bağlanan değerlerin okumaları, örnek adımıyla. Örnek ya
    /// da özet yoksa `false` (çağıran tabanı okur).
    fn instance_output(&self, inst: DefId, port: &str, out: &mut Vec<Read>) -> bool {
        let Some(map) = self.bindings.get(&inst) else {
            return false;
        };
        let Some(deps) = self.sums.output_deps(self.res, inst, port) else {
            return false;
        };
        let key = (inst, port.to_string());
        if let Some(reads) = self.expanded.borrow().get(&key) {
            out.extend(reads.iter().cloned());
            return true;
        }
        if !self.expanding.borrow_mut().insert(key.clone()) {
            // Örnekler üzerinden kombinasyonel döngü: bu adım zaten açık.
            self.cuts.set(self.cuts.get() + 1);
            return true;
        }
        self.expansions.set(self.expansions.get() + 1);
        let cuts = self.cuts.get();
        let mut mine: Vec<Read> = Vec::new();
        for input in &deps.inputs {
            let hop = Hop {
                inst: self.res.defs[inst.0 as usize].name.clone(),
                output: port.to_string(),
                target: deps.target.clone(),
                input: input.clone(),
                worst_case: deps.worst_case,
                assumed: deps.assumed.contains(input),
            };
            let mut inner = Vec::new();
            match map.get(input) {
                Some(Bound::Expr(e)) => self.reads(*e, &mut inner),
                Some(Bound::Def(d, s)) => inner.push((*d, *s, None)),
                None => {}
            }
            // Dıştaki adım okumanın deyimine en yakın olandır; içteki bir
            // adımın varsayımı dıştakine geçer. İç adım düştüğünden aynı
            // (tanım, konum, varsayım) bu giriş için bir kez yeter.
            let mut seen = HashSet::new();
            for (d, s, h) in inner {
                let assumed = hop.assumed || h.is_some_and(|h| h.assumed);
                if seen.insert((d, s, assumed)) {
                    let mut hop = hop.clone();
                    hop.assumed = assumed;
                    mine.push((d, s, Some(hop)));
                }
            }
        }
        self.expanding.borrow_mut().remove(&key);
        if self.cuts.get() == cuts {
            self.expanded.borrow_mut().insert(key, mine.clone());
        }
        out.extend(mine);
        true
    }

    fn is_sync_call(&self, callee: Idx<Expr>) -> bool {
        self.res.resolutions.get(&callee).is_some_and(|&d| {
            matches!(
                self.res.def_kind(d),
                DefKind::Builtin(BuiltinKind::Sync | BuiltinKind::Sync3)
            )
        })
    }

    /// İfadedeki tüm çözümlenmiş isim okumaları (konumlarıyla).
    fn reads(&self, e: Idx<Expr>, out: &mut Vec<Read>) {
        let expr = &self.ast.exprs[e];
        match &expr.kind {
            ExprKind::Path(_) => {
                if let Some(&def) = self.res.resolutions.get(&e) {
                    out.push((def, expr.span, None));
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
            ExprKind::Field { base, field } => {
                if let Some(&inst) = self.res.resolutions.get(base) {
                    if self.instance_output(inst, &field.text, out) {
                        return;
                    }
                }
                self.reads(*base, out)
            }
            // `sync()`/`sync3()` iki flop'tur: yolu keser (K9).
            ExprKind::Call { callee, .. } if self.is_sync_call(*callee) => {}
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
            | ExprKind::Concat(_)
            | ExprKind::Error => {}
        }
    }

    /// İfade konumundaki blok (match kolu gövdesi): deyim sağ tarafları
    /// ve kuyruk ifadesi.
    fn block_reads(&self, b: Idx<Block>, out: &mut Vec<Read>) {
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

#[cfg(test)]
mod tests {
    use volt_syntax::{parse, FileId};

    use super::Summaries;
    use crate::consteval::ConstEvaluator;
    use crate::resolve::{resolve_file, ResolveResult};
    use crate::typeck::typecheck;

    /// Birimin özetleri, tip denetiminden sonra (saat portu türünden).
    fn summaries(ast: &volt_ast::SourceFile, res: &ResolveResult) -> Summaries {
        let mut ev = ConstEvaluator::new(ast, res);
        ev.eval_all_consts();
        let tyck = typecheck(ast, res, &mut ev);
        Summaries::compute(ast, res, &tyck)
    }

    /// `module`'ün özeti: çıkış → girişler, okunur biçimde.
    fn summary_of(src: &str, module: &str) -> Vec<(String, Vec<String>)> {
        let parsed = parse(FileId(0), src);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.error_codes());
        let res = resolve_file(&parsed.ast);
        let sums = summaries(&parsed.ast, &res);
        let (def, _) = res.def_by_name(module).expect("modül");
        sums.get(def)
            .expect("özet")
            .comb
            .iter()
            .map(|(o, i)| (o.clone(), i.iter().cloned().collect()))
            .collect()
    }

    fn deps(pairs: &[(&str, &[&str])]) -> Vec<(String, Vec<String>)> {
        pairs
            .iter()
            .map(|(o, i)| (o.to_string(), i.iter().map(|s| s.to_string()).collect()))
            .collect()
    }

    #[test]
    fn continuous_assignment_and_let_are_edges() {
        let src = "module M {\n    in a : bool\n    in b : bool\n    out y : bool\n    out z : bool\n    let t = a && b\n    y = t\n    z = true\n}\n";
        assert_eq!(
            summary_of(src, "M"),
            deps(&[("y", &["a", "b"]), ("z", &[])])
        );
    }

    #[test]
    fn independent_paths_stay_apart() {
        let src = "module M {\n    in a1 : bool\n    in a2 : bool\n    out y1 : bool\n    out y2 : bool\n    y1 = a1\n    y2 = a2\n}\n";
        assert_eq!(
            summary_of(src, "M"),
            deps(&[("y1", &["a1"]), ("y2", &["a2"])])
        );
    }

    #[test]
    fn comb_condition_match_subject_and_guard_are_operands() {
        let src = "module M {\n    in c : bool\n    in s : bool\n    in g : bool\n    in d : bool\n    out y : bool\n    out z : bool\n    comb {\n        y = false\n        if c { y = d }\n    }\n    comb {\n        match s {\n            true if g => { z = true }\n            _ => { z = false }\n        }\n    }\n}\n";
        assert_eq!(
            summary_of(src, "M"),
            deps(&[("y", &["c", "d"]), ("z", &["g", "s"])])
        );
    }

    #[test]
    fn function_arguments_are_operands() {
        let src = "fn both(x: bool, y: bool) -> bool {\n    x && y\n}\nmodule M {\n    in a : bool\n    in b : bool\n    out y : bool\n    y = both(a, b)\n}\n";
        assert_eq!(summary_of(src, "M"), deps(&[("y", &["a", "b"])]));
    }

    #[test]
    fn on_block_and_sync_cut_the_path() {
        let src = "module M {\n    in clk : clock\n    in a : bool\n    in b : bool\n    out q : bool\n    out s : bool\n    reg r : bool = false\n    on clk {\n        r <= a\n    }\n    q = r\n    s = sync(b, clk)\n}\n";
        assert_eq!(summary_of(src, "M"), deps(&[("q", &[]), ("s", &[])]));
    }

    #[test]
    fn nested_instances_compose_from_the_child_up() {
        let src = "module Pass {\n    in a : bool\n    out y : bool\n    y = a\n}\nmodule Outer {\n    in u : bool\n    in v : bool\n    out w : bool\n    let p = Pass { a: v }\n    w = !p.y\n}\n";
        assert_eq!(summary_of(src, "Outer"), deps(&[("w", &["v"])]));
    }

    #[test]
    fn registered_child_output_cuts_the_path() {
        let src = "module R {\n    in clk : clock\n    in d : bool\n    out q : bool\n    reg r : bool = false\n    on clk {\n        r <= d\n    }\n    q = r\n}\nmodule M {\n    in clk : clock\n    in a : bool\n    out y : bool\n    let r = R { clk: clk, d: a }\n    y = r.q\n}\n";
        assert_eq!(summary_of(src, "M"), deps(&[("y", &[])]));
    }

    #[test]
    fn builtins_follow_the_table() {
        let src = "module M {\n    in clk : clock\n    in a : bool\n    in b : bool\n    out e : bool\n    out f : bool\n    let ed = EdgeDetect { clk: clk, signal: a }\n    let sr = ShiftRegister<bool, 2> { clk: clk, data_in: b, shift_en: true }\n    e = ed.rising\n    f = sr.data_out\n}\n";
        assert_eq!(summary_of(src, "M"), deps(&[("e", &["a"]), ("f", &[])]));
    }

    #[test]
    fn externs_are_worst_case_without_a_clock_and_empty_with_one() {
        let src = "extern module C {\n    in a : bool\n    in b : bool\n    out y : bool\n    out z : bool\n}\nextern module K {\n    in clk : clock\n    in a : bool\n    out y : bool\n}\n";
        assert_eq!(
            summary_of(src, "C"),
            deps(&[("y", &["a", "b"]), ("z", &["a", "b"])])
        );
        assert_eq!(summary_of(src, "K"), deps(&[("y", &[])]));
    }

    #[test]
    fn each_generic_expansion_is_its_own_module() {
        let src = "module Pick<const N: u32> {\n    in a : bool\n    in b : bool\n    out y : bool\n    y = if N == 0 { a } else { b }\n}\nmodule M {\n    in a : bool\n    in b : bool\n    out y0 : bool\n    out y1 : bool\n    let p0 = Pick<0> { a: a, b: b }\n    let p1 = Pick<1> { a: a, b: b }\n    y0 = p0.y\n    y1 = p1.y\n}\n";
        // Koşul sabittir ve her iki kol okunur: port düzeyinde iki açılım
        // da a ve b'ye bağlıdır; önemli olan her açılımın bir özeti olması.
        assert_eq!(summary_of(src, "Pick_0"), deps(&[("y", &["a", "b"])]));
        assert_eq!(summary_of(src, "Pick_1"), deps(&[("y", &["a", "b"])]));
        assert_eq!(
            summary_of(src, "M"),
            deps(&[("y0", &["a", "b"]), ("y1", &["a", "b"])])
        );
    }

    #[test]
    fn a_combinational_loop_through_instances_terminates() {
        let src = "module Pass {\n    in a : bool\n    out y : bool\n    y = a\n}\nmodule M {\n    in h : bool\n    out o : bool\n    wire w : bool\n    let p = Pass { a: w }\n    let q = Pass { a: p.y && h }\n    w = q.y\n    o = w\n}\n";
        assert_eq!(summary_of(src, "M"), deps(&[("o", &["h"])]));
    }

    /// `instance_output`'un kesimi (`expanding`, `cuts`) geçerli bir
    /// programda tetiklenmez: bir örneğin bağlaması ancak kendisinden önce
    /// bildirilmiş örneklerin çıkışlarını okuyabilir, sonraki örneği ya da
    /// kendi çıkışını okumak çözümlemede E1002'dir ve özet yalnız temiz
    /// çözümlemeden sonra hesaplanır (`pipeline.rs`). Örnekler üzerinden
    /// döngü bir `wire`'dan geçer; tel bir `Path` okumasıdır, açılım
    /// özyinelemesine girmez (`a_combinational_loop_through_instances_terminates`).
    /// `.cargo/mutants.toml`'daki A2 kaydının dayanağı.
    #[test]
    fn an_instance_binding_cannot_read_a_later_or_its_own_instance() {
        let pass = "module Pass {\n    in a : bool\n    in b : bool\n    out y : bool\n    out z : bool\n    \
                    y = a\n    z = b\n}\n";
        for (body, n) in [
            (
                "let p = Pass { a: q.y, b: h }\n    let q = Pass { a: p.y, b: h }",
                1,
            ),
            ("let p = Pass { a: p.z, b: p.y }", 2),
        ] {
            let src = format!(
                "{pass}module M {{\n    in h : bool\n    out o : bool\n    {body}\n    o = p.y\n}}\n"
            );
            let parsed = parse(FileId(0), &src);
            assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.error_codes());
            let res = resolve_file(&parsed.ast);
            let codes: Vec<&str> = res
                .diagnostics
                .iter()
                .filter(|d| d.severity == volt_diagnostics::Severity::Error)
                .map(|d| d.code.as_str())
                .collect();
            assert_eq!(codes, vec!["E1002"; n], "{body}");
        }
    }

    /// K4: saatsiz extern'in bağımlılıkları varsayımdır; bir Volt
    /// modülünün içinden geçince de varsayım olarak kalır, gerçek bir kapı
    /// aynı çifti kurarsa artık varsayım değildir.
    #[test]
    fn assumptions_of_a_clockless_extern_survive_nesting() {
        let src = "extern module X {\n    in a : bool\n    in b : bool\n    out y : bool\n}\n\
                   module W {\n    in a : bool\n    in b : bool\n    out y : bool\n    out z : bool\n    \
                   let x = X { a: a, b: b }\n    y = x.y\n    z = x.y || a\n}\n";
        let parsed = parse(FileId(0), src);
        let res = resolve_file(&parsed.ast);
        let sums = summaries(&parsed.ast, &res);
        let (w, _) = res.def_by_name("W").expect("W");
        let s = sums.get(w).expect("özet");
        let set = |v: &[&str]| {
            v.iter()
                .map(|s| s.to_string())
                .collect::<std::collections::BTreeSet<_>>()
        };
        assert_eq!(s.comb["y"], set(&["a", "b"]));
        assert_eq!(s.assumed["y"], set(&["a", "b"]));
        // z, a'yı görünür bir kapıdan da okuyor: yalnız b varsayım.
        assert_eq!(s.comb["z"], set(&["a", "b"]));
        assert_eq!(s.assumed["z"], set(&["b"]));
        let (x, _) = res.def_by_name("X").expect("X");
        assert_eq!(sums.get(x).expect("özet").assumed["y"], set(&["a", "b"]));
    }

    /// Özetin girişleri yalnız saat dışı giriş portlarıdır: bir çıkışı
    /// okuyan çıkış o çıkışa değil, onun girişlerine bağlıdır (mutasyon
    /// koşusu 42670004438, `summarize_module`'deki giriş filtresi).
    #[test]
    fn only_non_clock_inputs_are_summary_inputs() {
        let src = "module M {\n    in clk : clock\n    in a : bool\n    out y : bool\n    out z : bool\n    y = a\n    z = !y\n}\n";
        assert_eq!(summary_of(src, "M"), deps(&[("y", &["a"]), ("z", &["a"])]));
    }

    /// Erişilemezlik (ADR-0109 A2, `.cargo/mutants.toml`): `match`
    /// ifadesinin kolu blok olamaz — `parse_match_expr` kolları blok
    /// bağlamı olmadan ayrıştırır, blok kolu E0001'dir ve AST'de
    /// `ExprKind::Match` blok kolu taşımaz. `Walker::block_reads` bu yüzden
    /// bugün çağrılmaz; kod ileride blok kolu açılırsa diye durur.
    #[test]
    fn a_match_expression_never_has_a_block_arm() {
        let src = "module M {\n    in s : bool\n    in a : bool\n    in b : bool\n    out y : bool\n    y = match s {\n        true => { a }\n        _ => { b }\n    }\n}\n";
        let parsed = parse(FileId(0), src);
        assert!(
            parsed.error_codes().contains(&"E0001"),
            "{:?}",
            parsed.error_codes()
        );
        for e in parsed.ast.exprs.iter() {
            if let volt_ast::ExprKind::Match { arms, .. } = &e.kind {
                assert!(arms
                    .iter()
                    .all(|a| matches!(a.body, volt_ast::MatchArmBody::Expr(_))));
            }
        }
    }

    /// Yalnız saatsiz extern'in özeti en kötü durum olarak işaretlenir;
    /// saatsiz bir Volt modülününki gövdesinden gelir.
    #[test]
    fn only_a_clockless_extern_is_worst_case() {
        let src = "extern module X {\n    in a : bool\n    out y : bool\n}\nextern module K {\n    in clk : clock\n    in a : bool\n    out y : bool\n}\nmodule P {\n    in a : bool\n    out y : bool\n    y = a\n}\n";
        let parsed = parse(FileId(0), src);
        let res = resolve_file(&parsed.ast);
        let sums = summaries(&parsed.ast, &res);
        let worst = |n: &str| {
            sums.get(res.def_by_name(n).expect("tanım").0)
                .expect("özet")
                .worst_case
        };
        assert!(worst("X"));
        assert!(!worst("K"));
        assert!(!worst("P"));
    }

    #[test]
    fn summary_is_per_port_not_per_bit() {
        // Port düzeyi (ADR-0114 sınırı): y[0] = a, y[1] = b → y ikisine de bağlı.
        let src = "module M {\n    in a : bool\n    in b : bool\n    out y : bits<2>\n    comb {\n        y[0] = a\n        y[1] = b\n    }\n}\n";
        assert_eq!(summary_of(src, "M"), deps(&[("y", &["a", "b"])]));
    }

    /// Hedef indeksi işlenendir (ADR-0114 §1): `y[sel] = d`'de `sel`
    /// değişince `d`'yi taşıyan bit değişir; part-select başlangıcı da aynı.
    #[test]
    fn a_target_index_is_an_operand() {
        let src = "module M {
    in sel : u3
    in d : bool
    in d2 : bits<2>
    out y : u8
    out z : u8
    comb {
        y = 0
        y[sel] = d
        z = 0
        z[sel +: 2] = d2
    }
}
";
        assert_eq!(
            summary_of(src, "M"),
            deps(&[("y", &["d", "sel"]), ("z", &["d2", "sel"])])
        );
    }

    /// Örnek zincirinde her (örnek, çıkış) bir kez açılır: `N` aşamalı,
    /// her çıkışı iki girişe bağlı zincirde açılım `2N - 1`'dir (aşama
    /// başına iki çıkış, son aşamada yalnız okunan), aşama sayısıyla
    /// doğrusal; sonuncunun okuması her giriş için iki yaprak taşır.
    /// Zamana değil sayıya bakar.
    #[test]
    fn an_instance_chain_expands_each_output_once() {
        for n in [4usize, 8, 16] {
            let mut src = String::from(
                "module St {
    in a : bool
    in b : bool
    out x : bool
    out y : bool
    x = a ^ b
    y = a && b
}
module Top {
    in a0 : bool
    in b0 : bool
    out o : bool
    let s1 = St { a: a0, b: b0 }
",
            );
            for i in 2..=n {
                src.push_str(&format!(
                    "    let s{i} = St {{ a: s{p}.x, b: s{p}.y }}
",
                    p = i - 1
                ));
            }
            src.push_str(&format!(
                "    o = s{n}.x
}}
"
            ));
            let parsed = parse(FileId(0), &src);
            assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.error_codes());
            let res = resolve_file(&parsed.ast);
            let sums = summaries(&parsed.ast, &res);
            let (top_def, _) = res.def_by_name("Top").expect("Top");
            let volt_ast::ItemKind::Module(top) =
                &parsed.ast.items_arena[res.item_of_def[&top_def]].kind
            else {
                panic!("Top bir modül olmalı");
            };
            let g = super::CombGraph::build(&parsed.ast, &res, top, &sums);
            assert_eq!(g.expansions, 2 * n - 1, "N = {n}: açılım sayısı");
            let (o, _) = res.def_by_name("o").expect("o");
            assert_eq!(g.edges[&o].len(), 4, "N = {n}: o'nun kenarları");
        }
    }

    /// Saat portu türünden anlaşılır: takma adlı saat (ADR-0070) de
    /// saattir; extern saatlidir (özet boş, K5), en kötü durum değildir.
    #[test]
    fn a_clock_through_a_type_alias_is_a_clock() {
        let src = "type Clk = clock
extern module K {
    in clk : Clk
    in a : bool
    out y : bool
}
";
        let parsed = parse(FileId(0), src);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.error_codes());
        let res = resolve_file(&parsed.ast);
        let sums = summaries(&parsed.ast, &res);
        let k = sums.get(res.def_by_name("K").expect("K").0).expect("özet");
        assert!(!k.worst_case);
        assert!(k.comb["y"].is_empty(), "{:?}", k.comb);
    }
}
