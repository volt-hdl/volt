//! Struct değerlerinin SV'ye indirgenmesi (ADR-0077 Karar 5 — B eşlemesi).
//!
//! Struct tipli her sinyal (port, `reg`, `wire`, `let`) yaprak başına bir
//! SV sinyaline açılır: `p : P` → `p_a`, `p_s`, `p_i_x` … (ADR-0039 §14
//! bundle kuralı). Geçiş emit'ten ÖNCE, AST'nin bir kopyası üzerinde
//! koşar ve SV eşlemesi olmayan bütün-struct işlemlerini yaprak
//! işlemlerine çevirir; emitter yalnız düz sinyaller görür:
//!
//! | Kaynak                        | İndirgenmiş                              |
//! |-------------------------------|------------------------------------------|
//! | `p.i.x`                       | `p_i_x`                                  |
//! | `p <= e` / `p = e`            | yaprak başına atama (bildirim sırası)    |
//! | `P { a: x, b: y }`            | yaprak başına değer                      |
//! | `p == q` / `!=`               | `{p_a, …} == {q_a, …}` (Karar 3 düzeni)  |
//! | `p as uN`                     | `N'({p_a, …})`                           |
//! | `raw as P`                    | yaprak başına dilim `raw[11:8]` …        |
//! | `if c { p } else { q }`       | yaprak başına üçlü                       |
//! | `Sub { d: e }`                | `.d_a(…), .d_s(…)` …                     |
//! | `s0.q.a`                      | `s0.q_a`                                 |
//! | `prev(p)`, `sync(p, clk)`     | yaprak başına                            |
//!
//! Struct kullanmayan birimde geçiş `None` döner ve emitter özgün AST'yi
//! kullanır — çıktı byte-aynı kalır (Karar 5 kural 15). Yan ürünler
//! (`StructNotes`): grup başındaki düzen yorumu ve modülün hiç okumadığı
//! yaprakların lint susturması (kural 2, 13).

mod value;

/// Genişlik sabiti (takma ad ve E0003 metni için).
pub(crate) fn const_int(ast: &SourceFile, e: Idx<Expr>) -> Option<i128> {
    value::const_int(ast, e, 0)
}

use std::collections::{HashMap, HashSet};

use volt_ast::struct_layout::{self, Leaf, StructLayout};
use volt_ast::{
    Block, BlockStmt, ElseBranch, Expr, ExprKind, IfStmt, ItemKind, LValue, LValueSuffix, LetDecl,
    MatchArmBody, ModuleDecl, Name, Port, PortBinding, PortDir, RegDecl, SourceFile, Stmt,
    StmtKind, TypeRef, WireDecl,
};
use volt_ast::{Idx, TypeRefKind};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan};
use volt_span::Span;

/// İndirgenmiş birim + emitter notları.
pub(crate) struct Lowered {
    pub ast: SourceFile,
    pub notes: StructNotes,
}

/// Emitter'ın bildirim satırlarına eklediği struct bilgileri; anahtar
/// (modül, yaprak sinyali).
#[derive(Debug, Default)]
pub(crate) struct StructNotes {
    /// Grubun ilk yaprağının üstüne yazılan düzen yorumu (kural 2).
    pub layout: HashMap<(String, String), String>,
    /// Modülün hiç okumadığı giriş/iç yapraklar (kural 13).
    pub unused: HashSet<(String, String)>,
    /// Dizi tipli register yaprağı: paketlenmiş vektör (ADR-0056 düzeni).
    pub packed: HashSet<(String, String)>,
    /// Struct register yaprağı → aynı register'ın bütün yaprakları: bir
    /// `on` bloğu bir yaprağı yazıyorsa hepsi o bloğun reset'ine girer —
    /// yazılmayan yaprak reset değerini korur (Karar 4).
    pub siblings: HashMap<(String, String), Vec<String>>,
}

impl StructNotes {
    pub(crate) fn layout_of(&self, module: &str, name: &str) -> Option<&str> {
        self.layout
            .get(&(module.to_string(), name.to_string()))
            .map(String::as_str)
    }

    pub(crate) fn is_unused(&self, module: &str, name: &str) -> bool {
        self.unused
            .contains(&(module.to_string(), name.to_string()))
    }

    pub(crate) fn is_packed(&self, module: &str, name: &str) -> bool {
        self.packed
            .contains(&(module.to_string(), name.to_string()))
    }

    pub(crate) fn siblings_of(&self, module: &str, name: &str) -> Option<&[String]> {
        self.siblings
            .get(&(module.to_string(), name.to_string()))
            .map(Vec::as_slice)
    }
}

/// Birimi indirger. Struct tipli sinyal yoksa `None` (özgün AST kullanılır).
pub(crate) fn lower(ast: &SourceFile) -> (Option<Lowered>, Vec<Diagnostic>) {
    let has_plain_struct = ast
        .items
        .iter()
        .any(|&i| matches!(&ast.items_arena[i].kind, ItemKind::Struct(s) if !s.is_port));
    if !has_plain_struct {
        return (None, Vec::new());
    }
    let mut lw = Lowerer::new(ast.clone());
    let modules: Vec<Idx<volt_ast::Item>> = lw
        .ast
        .items
        .iter()
        .copied()
        .filter(|&i| matches!(lw.ast.items_arena[i].kind, ItemKind::Module(_)))
        .collect();
    for item in modules {
        lw.lower_module(item);
    }
    let diags = std::mem::take(&mut lw.diags);
    if !lw.changed {
        return (None, diags);
    }
    (
        Some(Lowered {
            ast: lw.ast,
            notes: lw.notes,
        }),
        diags,
    )
}

/// Modül içi sembol bilgisi.
#[derive(Default)]
struct ModEnv {
    /// Struct tipli sinyal → struct adı (geçerli düzenli).
    sigs: HashMap<String, String>,
    /// Kullanıcı modülü örneği → hedef modül adı.
    insts: HashMap<String, String>,
}

pub(super) struct Lowerer {
    ast: SourceFile,
    /// Düz struct adı → düzen (geçersizse `None`).
    layouts: HashMap<String, Option<StructLayout>>,
    /// Modül adı → (port adı → struct adı) — örnek bağlantısı ve
    /// `s0.q.a` erişimi için (özgün tipler).
    module_ports: HashMap<String, HashMap<String, String>>,
    /// `const K : P = …` → (struct adı, değer).
    consts: HashMap<String, (String, Idx<Expr>)>,
    env: ModEnv,
    module: String,
    diags: Vec<Diagnostic>,
    notes: StructNotes,
    changed: bool,
    rewritten: HashSet<Idx<Expr>>,
}

impl Lowerer {
    fn new(ast: SourceFile) -> Self {
        let mut lw = Lowerer {
            ast,
            layouts: HashMap::new(),
            module_ports: HashMap::new(),
            consts: HashMap::new(),
            env: ModEnv::default(),
            module: String::new(),
            diags: Vec::new(),
            notes: StructNotes::default(),
            changed: false,
            rewritten: HashSet::new(),
        };
        lw.collect_items();
        lw
    }

    /// Düzenler, modül port tipleri ve struct sabitleri.
    fn collect_items(&mut self) {
        let ast = &self.ast;
        let mut layouts = HashMap::new();
        for &i in &ast.items {
            if let ItemKind::Struct(s) = &ast.items_arena[i].kind {
                if !s.is_port {
                    let l =
                        struct_layout::layout(ast, s, &mut |e| value::const_int(ast, e, 0)).ok();
                    layouts.insert(s.name.text.clone(), l);
                }
            }
        }
        self.layouts = layouts;
        let mut module_ports = HashMap::new();
        let mut consts = HashMap::new();
        for &i in &self.ast.items {
            match &self.ast.items_arena[i].kind {
                ItemKind::Module(m) => {
                    let ports: HashMap<String, String> = m
                        .ports
                        .iter()
                        .filter_map(|p| Some((p.name.text.clone(), self.struct_of_type(p.ty)?)))
                        .collect();
                    module_ports.insert(m.name.text.clone(), ports);
                }
                ItemKind::Const(c) => {
                    if let Some(s) = self.struct_of_type(c.ty) {
                        consts.insert(c.name.text.clone(), (s, c.value));
                    }
                }
                _ => {}
            }
        }
        self.module_ports = module_ports;
        self.consts = consts;
    }

    /// Tip, düzeni geçerli bir düz struct'ı adlandırıyorsa adı. Generic,
    /// struct dizisi ya da düzeni kurulamayan struct indirgenmez —
    /// emitter'ın tip eşlemesi onlara E0003 verir.
    fn struct_of_type(&self, ty: Idx<TypeRef>) -> Option<String> {
        let decl = struct_layout::struct_of_type(&self.ast, ty)?;
        let target = crate::alias::resolve(&self.ast, ty);
        if let TypeRefKind::Path { args, .. } = &self.ast.types[target].kind {
            if !args.is_empty() {
                return None;
            }
        }
        let name = decl.name.text.clone();
        self.layouts.get(&name)?.as_ref().map(|_| name)
    }

    fn layout(&self, name: &str) -> Option<&StructLayout> {
        self.layouts.get(name)?.as_ref()
    }

    /// Bir yaprağın SV sinyal adı: `taban_alan_altalan`.
    fn leaf_name(base: &str, path: &[String]) -> String {
        format!("{base}_{}", path.join("_"))
    }

    // ═══ Modül ════════════════════════════════════════════════════

    fn lower_module(&mut self, item: Idx<volt_ast::Item>) {
        let ItemKind::Module(m) = &self.ast.items_arena[item].kind else {
            return;
        };
        let module = m.clone();
        self.module = module.name.text.clone();
        self.env = ModEnv::default();
        self.build_env(&module);
        let uses_structs = !self.env.sigs.is_empty()
            || self.touches_struct_ports(&module)
            || self.mentions_struct_values(&module);
        if !uses_structs {
            return;
        }
        self.changed = true;
        self.check_collisions(&module);

        let mut leaves_declared: Vec<(String, Vec<String>)> = Vec::new();
        let ports = self.lower_ports(&module, &mut leaves_declared);
        let mut body = Vec::with_capacity(module.body.len());
        for &stmt in &module.body {
            body.extend(self.lower_stmt(stmt, &mut leaves_declared));
        }
        let contracts = module.contracts.clone();
        for c in &contracts {
            self.rewrite_expr(c.expr);
        }
        self.safety_net(&body, &contracts);

        if let ItemKind::Module(m) = &mut self.ast.items_arena[item].kind {
            m.ports = ports;
            m.body = body;
        }
        self.note_unused(item, &leaves_declared);
    }

    /// Struct tipli sinyaller ve örnekler.
    fn build_env(&mut self, m: &ModuleDecl) {
        for p in &m.ports {
            if let Some(s) = self.struct_of_type(p.ty) {
                self.env.sigs.insert(p.name.text.clone(), s);
            }
        }
        // Tipsiz `let`/`reg` başka struct değerinden çıkarılır; iki tur
        // sıra bağımlılığını çözer.
        for _ in 0..2 {
            for &stmt in &m.body {
                let entry = match &self.ast.stmts[stmt].kind {
                    StmtKind::Reg(r) => Some((
                        r.name.text.clone(),
                        r.ty.and_then(|t| self.struct_of_type(t)).or_else(|| {
                            r.ty.is_none()
                                .then(|| self.struct_of_expr(r.init))
                                .flatten()
                        }),
                    )),
                    StmtKind::Wire(w) => Some((w.name.text.clone(), self.struct_of_type(w.ty))),
                    StmtKind::Let(l) => Some((l.name.text.clone(), self.let_struct(l))),
                    StmtKind::Instance(inst) => {
                        if let [seg] = inst.module_path.segments.as_slice() {
                            self.env
                                .insts
                                .insert(inst.name.text.clone(), seg.text.clone());
                        }
                        None
                    }
                    StmtKind::On(on) => {
                        self.scan_block_lets(on.body);
                        None
                    }
                    StmtKind::Comb(b) => {
                        self.scan_block_lets(*b);
                        None
                    }
                    _ => None,
                };
                if let Some((name, Some(s))) = entry {
                    self.env.sigs.insert(name, s);
                }
            }
        }
    }

    fn let_struct(&self, l: &LetDecl) -> Option<String> {
        match l.ty {
            Some(t) => self.struct_of_type(t),
            None => self.struct_of_expr(l.value),
        }
    }

    /// Blok içi struct `let`'leri (ad modül çapında tekil sayılır).
    fn scan_block_lets(&mut self, block: Idx<Block>) {
        let stmts = self.ast.blocks[block].stmts.clone();
        for s in &stmts {
            match s {
                BlockStmt::Let(l) => {
                    if let Some(st) = self.let_struct(l) {
                        self.env.sigs.insert(l.name.text.clone(), st);
                    }
                }
                BlockStmt::If(i) => self.scan_if_lets(i),
                BlockStmt::Match(m) => {
                    for arm in &m.arms {
                        if let MatchArmBody::Block(b) = arm.body {
                            self.scan_block_lets(b);
                        }
                    }
                }
                BlockStmt::For(f) => self.scan_block_lets(f.body),
                _ => {}
            }
        }
    }

    fn scan_if_lets(&mut self, i: &IfStmt) {
        self.scan_block_lets(i.then_block);
        match &i.else_branch {
            Some(ElseBranch::Block(b)) => self.scan_block_lets(*b),
            Some(ElseBranch::If(n)) => self.scan_if_lets(n),
            None => {}
        }
    }

    /// Modül, struct portlu bir modülü örnekliyor mu?
    fn touches_struct_ports(&self, m: &ModuleDecl) -> bool {
        self.env.insts.values().any(|t| {
            self.module_ports
                .get(t)
                .is_some_and(|ports| !ports.is_empty())
        }) || m.ports.iter().any(|p| self.struct_of_type(p.ty).is_some())
    }

    /// Modül struct sabiti ya da `raw as P` gibi struct değerli bir ifade
    /// kullanıyor mu (struct sinyali olmadan)?
    fn mentions_struct_values(&self, m: &ModuleDecl) -> bool {
        let mut found = false;
        let mut visit = |ast: &SourceFile, e: Idx<Expr>| match &ast.exprs[e].kind {
            ExprKind::Path(p) if p.segments.len() == 1 => {
                found |= self.consts.contains_key(&p.segments[0].text);
            }
            ExprKind::Cast { ty, .. } => found |= self.struct_of_type(*ty).is_some(),
            ExprKind::StructLit { path, .. } => {
                found |= path
                    .segments
                    .last()
                    .is_some_and(|s| self.layout(&s.text).is_some());
            }
            _ => {}
        };
        value::walk_module_exprs(&self.ast, m, &mut visit);
        found
    }

    // ═══ Portlar ══════════════════════════════════════════════════

    fn lower_ports(
        &mut self,
        m: &ModuleDecl,
        declared: &mut Vec<(String, Vec<String>)>,
    ) -> Vec<Port> {
        let mut out = Vec::with_capacity(m.ports.len());
        for p in &m.ports {
            let Some(s) = self.struct_of_type(p.ty) else {
                out.push(p.clone());
                continue;
            };
            let leaves = self
                .layout(&s)
                .map(|l| l.leaves.clone())
                .unwrap_or_default();
            self.note_layout(&p.name.text, &s, &leaves);
            let mut names = Vec::new();
            for leaf in &leaves {
                let name = Self::leaf_name(&p.name.text, &leaf.path);
                names.push(name.clone());
                out.push(Port {
                    span: p.span,
                    attrs: p.attrs.clone(),
                    doc: None,
                    direction: p.direction,
                    name: Name {
                        text: name,
                        span: p.name.span,
                    },
                    ty: leaf.ty,
                    domain: p.domain.clone(),
                    bundle: p.bundle.clone(),
                });
            }
            // Çıkış yaprakları okunmasa da susturulmaz (UNUSEDSIGNAL almaz).
            if p.direction != PortDir::Out {
                declared.push((p.name.text.clone(), names));
            }
        }
        out
    }

    /// Grubun ilk yaprağına düzen yorumu: `// struct P d : a[11:8] …`.
    fn note_layout(&mut self, base: &str, s: &str, leaves: &[Leaf]) {
        let Some(first) = leaves.first() else {
            return;
        };
        let describe = self
            .layout(s)
            .map(StructLayout::describe)
            .unwrap_or_default();
        self.notes.layout.insert(
            (self.module.clone(), Self::leaf_name(base, &first.path)),
            format!("// struct {s} {base} : {describe}"),
        );
        for leaf in leaves {
            if matches!(
                self.ast.types[crate::alias::resolve(&self.ast, leaf.ty)].kind,
                TypeRefKind::Array { .. }
            ) {
                self.notes
                    .packed
                    .insert((self.module.clone(), Self::leaf_name(base, &leaf.path)));
            }
        }
    }

    // ═══ Deyimler ═════════════════════════════════════════════════

    fn lower_stmt(
        &mut self,
        stmt: Idx<Stmt>,
        declared: &mut Vec<(String, Vec<String>)>,
    ) -> Vec<Idx<Stmt>> {
        let st = self.ast.stmts[stmt].clone();
        match &st.kind {
            StmtKind::Reg(r) if self.env.sigs.contains_key(&r.name.text) => {
                let s = self.env.sigs[&r.name.text].clone();
                let out = self.expand_decl(
                    &st,
                    &r.name,
                    &s,
                    declared,
                    |name, leaf, init| {
                        StmtKind::Reg(RegDecl {
                            name,
                            domain: r.domain.clone(),
                            ty: Some(leaf.ty),
                            init: init.expect("reg başlangıç değeri"),
                        })
                    },
                    Some(r.init),
                );
                if let Some((_, names)) = declared.last() {
                    for n in names {
                        self.notes
                            .siblings
                            .insert((self.module.clone(), n.clone()), names.clone());
                    }
                }
                out
            }
            StmtKind::Wire(w) if self.env.sigs.contains_key(&w.name.text) => {
                let s = self.env.sigs[&w.name.text].clone();
                self.expand_decl(
                    &st,
                    &w.name,
                    &s,
                    declared,
                    |name, leaf, _| StmtKind::Wire(WireDecl { name, ty: leaf.ty }),
                    None,
                )
            }
            StmtKind::Let(l) if self.env.sigs.contains_key(&l.name.text) => {
                let s = self.env.sigs[&l.name.text].clone();
                self.expand_decl(
                    &st,
                    &l.name,
                    &s,
                    declared,
                    |name, leaf, value| {
                        StmtKind::Let(LetDecl {
                            name,
                            ty: Some(leaf.ty),
                            value: value.expect("let değeri"),
                        })
                    },
                    Some(l.value),
                )
            }
            StmtKind::Reg(r) => {
                self.rewrite_expr(r.init);
                vec![stmt]
            }
            StmtKind::Let(l) => {
                self.rewrite_expr(l.value);
                vec![stmt]
            }
            StmtKind::Assign(a) => {
                let Some(parts) = self.expand_assign(&a.lhs, a.rhs) else {
                    self.rewrite_lvalue(&a.lhs);
                    self.rewrite_expr(a.rhs);
                    return vec![stmt];
                };
                parts
                    .into_iter()
                    .map(|(lhs, rhs)| {
                        self.rewrite_expr(rhs);
                        self.rewrite_lvalue(&lhs);
                        self.ast.stmts.alloc(Stmt {
                            span: st.span,
                            attrs: st.attrs.clone(),
                            kind: StmtKind::Assign(volt_ast::AssignStmt { lhs, rhs }),
                        })
                    })
                    .collect()
            }
            StmtKind::Instance(inst) => {
                let bindings = self.lower_bindings(inst);
                if let StmtKind::Instance(i) = &mut self.ast.stmts[stmt].kind {
                    i.bindings = bindings;
                }
                vec![stmt]
            }
            StmtKind::On(on) => {
                self.lower_block(on.body);
                vec![stmt]
            }
            StmtKind::Comb(b) => {
                self.lower_block(*b);
                vec![stmt]
            }
            StmtKind::Expr(e) => {
                self.rewrite_expr(*e);
                vec![stmt]
            }
            StmtKind::Wire(_) | StmtKind::For(_) | StmtKind::Error => vec![stmt],
        }
    }

    /// Struct bildirimi → yaprak bildirimleri; `value` yaprağa izdüşürülür.
    fn expand_decl(
        &mut self,
        st: &Stmt,
        name: &Name,
        s: &str,
        declared: &mut Vec<(String, Vec<String>)>,
        make: impl Fn(Name, &Leaf, Option<Idx<Expr>>) -> StmtKind,
        value: Option<Idx<Expr>>,
    ) -> Vec<Idx<Stmt>> {
        let leaves = self.layout(s).map(|l| l.leaves.clone()).unwrap_or_default();
        self.note_layout(&name.text, s, &leaves);
        let mut out = Vec::new();
        let mut names = Vec::new();
        for leaf in &leaves {
            let projected = match value {
                Some(v) => match self.project(v, &leaf.path) {
                    Some(p) => {
                        let p = self.pack_array(p, leaf.ty);
                        self.rewrite_expr(p);
                        Some(p)
                    }
                    None => {
                        self.unsupported_value(v, s);
                        return Vec::new();
                    }
                },
                None => None,
            };
            let leaf_name = Self::leaf_name(&name.text, &leaf.path);
            names.push(leaf_name.clone());
            let kind = make(
                Name {
                    text: leaf_name,
                    span: name.span,
                },
                leaf,
                projected,
            );
            out.push(self.ast.stmts.alloc(Stmt {
                span: st.span,
                attrs: st.attrs.clone(),
                kind,
            }));
        }
        declared.push((name.text.clone(), names));
        out
    }

    /// Struct hedefli atama → (hedef, değer) çiftleri; hedef struct değilse
    /// `None`.
    fn expand_assign(&mut self, lhs: &LValue, rhs: Idx<Expr>) -> Option<Vec<(LValue, Idx<Expr>)>> {
        let s = self.env.sigs.get(&lhs.base.text)?.clone();
        let layout = self.layout(&s)?.clone();
        let path: Vec<String> = lhs
            .suffixes
            .iter()
            .map_while(|x| match x {
                LValueSuffix::Field(f) => Some(f.text.clone()),
                _ => None,
            })
            .collect();
        let rest: Vec<LValueSuffix> = lhs.suffixes[path.len()..].to_vec();
        if layout.leaves.iter().any(|l| l.path == path) {
            return Some(vec![(
                LValue {
                    span: lhs.span,
                    base: Name {
                        text: Self::leaf_name(&lhs.base.text, &path),
                        span: lhs.base.span,
                    },
                    suffixes: rest,
                },
                rhs,
            )]);
        }
        let mut out = Vec::new();
        for leaf in layout.leaves_under(&path) {
            let rel = &leaf.path[path.len()..];
            let Some(v) = self.project(rhs, rel) else {
                self.unsupported_value(rhs, &s);
                return Some(Vec::new());
            };
            let v = self.pack_array(v, leaf.ty);
            out.push((
                LValue {
                    span: lhs.span,
                    base: Name {
                        text: Self::leaf_name(&lhs.base.text, &leaf.path),
                        span: lhs.base.span,
                    },
                    suffixes: Vec::new(),
                },
                v,
            ));
        }
        Some(out)
    }

    /// Hedefin sonek ifadeleri (indeks, aralık) yeniden yazılır.
    fn rewrite_lvalue(&mut self, lv: &LValue) {
        for s in &lv.suffixes {
            match s {
                LValueSuffix::Index(e) => self.rewrite_expr(*e),
                LValueSuffix::Range { hi, lo } => {
                    self.rewrite_expr(*hi);
                    self.rewrite_expr(*lo);
                }
                LValueSuffix::PartSelect { start, width, .. } => {
                    self.rewrite_expr(*start);
                    self.rewrite_expr(*width);
                }
                LValueSuffix::Field(_) => {}
            }
        }
    }

    /// Struct portlu modülün örneği: bağlantı yaprak başına (kural 11).
    fn lower_bindings(&mut self, inst: &volt_ast::InstanceDecl) -> Vec<PortBinding> {
        let target = inst.module_path.segments.last().map(|s| s.text.clone());
        let ports = target
            .and_then(|t| self.module_ports.get(&t).cloned())
            .unwrap_or_default();
        let mut out = Vec::with_capacity(inst.bindings.len());
        for b in &inst.bindings {
            let Some(s) = ports.get(&b.port_name.text) else {
                if let Some(v) = b.value {
                    self.rewrite_expr(v);
                }
                out.push(b.clone());
                continue;
            };
            let value = match b.value {
                Some(v) => v,
                None => self.ast.exprs.alloc(Expr {
                    span: b.port_name.span,
                    kind: ExprKind::Path(volt_ast::Path {
                        span: b.port_name.span,
                        segments: vec![b.port_name.clone()],
                    }),
                }),
            };
            let leaves = self.layout(s).map(|l| l.leaves.clone()).unwrap_or_default();
            for leaf in &leaves {
                let Some(v) = self.project(value, &leaf.path) else {
                    self.unsupported_value(value, s);
                    break;
                };
                let v = self.pack_array(v, leaf.ty);
                self.rewrite_expr(v);
                out.push(PortBinding {
                    span: b.span,
                    port_name: Name {
                        text: Self::leaf_name(&b.port_name.text, &leaf.path),
                        span: b.port_name.span,
                    },
                    value: Some(v),
                });
            }
        }
        out
    }

    // ═══ Bloklar ══════════════════════════════════════════════════

    fn lower_block(&mut self, block: Idx<Block>) {
        let stmts = self.ast.blocks[block].stmts.clone();
        let mut out = Vec::with_capacity(stmts.len());
        for s in stmts {
            self.lower_block_stmt(s, &mut out);
        }
        self.ast.blocks[block].stmts = out;
        if let Some(t) = self.ast.blocks[block].tail {
            self.rewrite_expr(t);
        }
    }

    fn lower_block_stmt(&mut self, s: BlockStmt, out: &mut Vec<BlockStmt>) {
        match s {
            BlockStmt::NonBlockAssign { lhs, rhs, span } => match self.expand_assign(&lhs, rhs) {
                Some(parts) => {
                    for (lhs, rhs) in parts {
                        self.rewrite_expr(rhs);
                        self.rewrite_lvalue(&lhs);
                        out.push(BlockStmt::NonBlockAssign { lhs, rhs, span });
                    }
                }
                None => {
                    self.rewrite_expr(rhs);
                    self.rewrite_lvalue(&lhs);
                    out.push(BlockStmt::NonBlockAssign { lhs, rhs, span });
                }
            },
            BlockStmt::BlockAssign { lhs, rhs, span } => match self.expand_assign(&lhs, rhs) {
                Some(parts) => {
                    for (lhs, rhs) in parts {
                        self.rewrite_expr(rhs);
                        self.rewrite_lvalue(&lhs);
                        out.push(BlockStmt::BlockAssign { lhs, rhs, span });
                    }
                }
                None => {
                    self.rewrite_expr(rhs);
                    self.rewrite_lvalue(&lhs);
                    out.push(BlockStmt::BlockAssign { lhs, rhs, span });
                }
            },
            BlockStmt::If(i) => {
                self.lower_if(&i);
                out.push(BlockStmt::If(i));
            }
            BlockStmt::Match(m) => {
                self.check_match_scrutinee(m.scrutinee);
                self.rewrite_expr(m.scrutinee);
                for arm in &m.arms {
                    if let Some(g) = arm.guard {
                        self.rewrite_expr(g);
                    }
                    match arm.body {
                        MatchArmBody::Block(b) => self.lower_block(b),
                        MatchArmBody::Expr(e) => self.rewrite_expr(e),
                    }
                }
                out.push(BlockStmt::Match(m));
            }
            BlockStmt::Let(l) => match self.env.sigs.get(&l.name.text).cloned() {
                Some(st) => {
                    let leaves = self
                        .layout(&st)
                        .map(|x| x.leaves.clone())
                        .unwrap_or_default();
                    for leaf in &leaves {
                        let Some(v) = self.project(l.value, &leaf.path) else {
                            self.unsupported_value(l.value, &st);
                            return;
                        };
                        let v = self.pack_array(v, leaf.ty);
                        self.rewrite_expr(v);
                        out.push(BlockStmt::Let(LetDecl {
                            name: Name {
                                text: Self::leaf_name(&l.name.text, &leaf.path),
                                span: l.name.span,
                            },
                            ty: Some(leaf.ty),
                            value: v,
                        }));
                    }
                }
                None => {
                    self.rewrite_expr(l.value);
                    out.push(BlockStmt::Let(l));
                }
            },
            BlockStmt::For(f) => {
                self.rewrite_expr(f.start);
                self.rewrite_expr(f.end);
                self.lower_block(f.body);
                out.push(BlockStmt::For(f));
            }
            BlockStmt::Error => out.push(BlockStmt::Error),
        }
    }

    fn lower_if(&mut self, i: &IfStmt) {
        self.rewrite_expr(i.cond);
        self.lower_block(i.then_block);
        match &i.else_branch {
            Some(ElseBranch::Block(b)) => self.lower_block(*b),
            Some(ElseBranch::If(n)) => self.lower_if(n),
            None => {}
        }
    }

    /// `match p { … }` — struct deseni bu turda yok (Karar 2).
    fn check_match_scrutinee(&mut self, e: Idx<Expr>) {
        if let Some(s) = self.struct_of_expr(e) {
            let span = self.ast.exprs[e].span;
            self.future(
                span,
                &lstr!(en: "'match' on a whole struct '{s}' value (match a field instead: match p.s)";
                       tr: "bütün bir '{s}' struct değeri üzerinde 'match' (bunun yerine bir alanı eşleyin: match p.s)"),
            );
        }
    }

    // ═══ Tanılar ══════════════════════════════════════════════════

    /// E0003 — `Emitter::future` ile aynı biçim (ADR-0070).
    fn future(&mut self, span: Span, what: &str) {
        let diag = Diagnostic::error(
            ErrorCode::E0003,
            lstr!(en: "not supported yet: {what}"; tr: "henüz desteklenmiyor: {what}"),
            LabeledSpan::primary(span, ""),
            lstr!(
                en: "this is valid Volt but has no SystemVerilog mapping yet; express it with supported constructs (see volt explain E0003)";
                tr: "bu geçerli Volt ama henüz SystemVerilog eşlemesi yok; desteklenen yapılarla yazın (bkz. volt explain E0003)"
            ),
        );
        if !self.diags.contains(&diag) {
            self.diags.push(diag);
        }
    }

    /// İzdüşürülemeyen struct değeri için E0003 (neyin desteklenmediği).
    fn unsupported_value(&mut self, e: Idx<Expr>, s: &str) {
        let span = self.ast.exprs[e].span;
        let what = match &self.ast.exprs[e].kind {
            ExprKind::Match { .. } => {
                lstr!(en: "'match' expressions of struct type '{s}'"; tr: "'{s}' struct tipli 'match' ifadeleri")
            }
            ExprKind::Call { .. } => {
                lstr!(en: "function calls that return struct '{s}'"; tr: "'{s}' struct'ı döndüren fonksiyon çağrıları")
            }
            ExprKind::Index { .. } => lstr!(en: "arrays of structs"; tr: "struct dizileri"),
            ExprKind::Cast { .. } => {
                lstr!(en: "a cast to struct '{s}' from a computed value (bind it to a let first: let raw : uN = ...; raw as {s})";
                      tr: "hesaplanan bir değerden '{s}' struct'ına dönüşüm (önce bir let'e bağlayın: let raw : uN = ...; raw as {s})")
            }
            _ => {
                lstr!(en: "this struct '{s}' value expression"; tr: "bu '{s}' struct değer ifadesi")
            }
        };
        self.future(span, &what);
    }

    /// E1003 — yaprak adı modüldeki başka bir adla çakışıyor (kural 1).
    fn check_collisions(&mut self, m: &ModuleDecl) {
        let mut names: HashSet<String> = m.ports.iter().map(|p| p.name.text.clone()).collect();
        for &s in &m.body {
            match &self.ast.stmts[s].kind {
                StmtKind::Reg(r) => names.insert(r.name.text.clone()),
                StmtKind::Wire(w) => names.insert(w.name.text.clone()),
                StmtKind::Let(l) => names.insert(l.name.text.clone()),
                StmtKind::Instance(i) => names.insert(i.name.text.clone()),
                _ => false,
            };
        }
        let mut sigs: Vec<(String, String, Span)> = Vec::new();
        for p in &m.ports {
            if let Some(s) = self.env.sigs.get(&p.name.text) {
                sigs.push((p.name.text.clone(), s.clone(), p.name.span));
            }
        }
        for &st in &m.body {
            let name = match &self.ast.stmts[st].kind {
                StmtKind::Reg(r) => Some(&r.name),
                StmtKind::Wire(w) => Some(&w.name),
                StmtKind::Let(l) => Some(&l.name),
                _ => None,
            };
            if let Some(n) = name {
                if let Some(s) = self.env.sigs.get(&n.text) {
                    sigs.push((n.text.clone(), s.clone(), n.span));
                }
            }
        }
        let mut seen: HashSet<String> = HashSet::new();
        for (base, s, span) in sigs {
            let leaves = self
                .layout(&s)
                .map(|l| l.leaves.clone())
                .unwrap_or_default();
            for leaf in leaves {
                let leaf_name = Self::leaf_name(&base, &leaf.path);
                if names.contains(&leaf_name) || !seen.insert(leaf_name.clone()) {
                    let field = leaf.dotted();
                    self.diags.push(Diagnostic::error(
                        ErrorCode::E1003,
                        lstr!(en: "'{leaf_name}' is already defined in this scope"; tr: "'{leaf_name}' bu kapsamda zaten tanımlı"),
                        LabeledSpan::primary(
                            span,
                            lstr!(en: "field '{field}' of '{base}' becomes the SystemVerilog signal '{leaf_name}'";
                                  tr: "'{base}' sinyalinin '{field}' alanı SystemVerilog'da '{leaf_name}' sinyali olur"),
                        ),
                        lstr!(en: "rename the signal or the field: struct fields become SV signals named <signal>_<field> (ADR-0077)";
                              tr: "sinyali ya da alanı yeniden adlandırın: struct alanları <sinyal>_<alan> adlı SV sinyallerine iner (ADR-0077)"),
                    ));
                }
            }
        }
    }

    /// İndirgenmemiş struct adı kalırsa (desteklenmeyen konum) E0003 —
    /// emitter tanımsız bir sinyal adı yazmasın.
    fn safety_net(&mut self, body: &[Idx<Stmt>], contracts: &[volt_ast::Contract]) {
        let sigs = self.env.sigs.clone();
        let mut hits: Vec<(Span, String)> = Vec::new();
        let mut visit = |ast: &SourceFile, e: Idx<Expr>| {
            if let ExprKind::Path(p) = &ast.exprs[e].kind {
                if let [seg] = p.segments.as_slice() {
                    if let Some(s) = sigs.get(&seg.text) {
                        hits.push((ast.exprs[e].span, s.clone()));
                    }
                }
            }
        };
        for &s in body {
            value::walk_stmt_exprs(&self.ast, s, &mut visit);
        }
        for c in contracts {
            value::walk_expr(&self.ast, c.expr, &mut visit);
        }
        for (span, s) in hits {
            self.future(
                span,
                &lstr!(en: "a whole struct '{s}' value in this position (use a field, ==, 'as uN' or an assignment)";
                       tr: "bu konumda bütün bir '{s}' struct değeri (bir alan, ==, 'as uN' ya da atama kullanın)"),
            );
        }
    }

    /// Kural 13: modülün hiç okumadığı giriş/iç yapraklar.
    fn note_unused(&mut self, item: Idx<volt_ast::Item>, declared: &[(String, Vec<String>)]) {
        let ItemKind::Module(m) = &self.ast.items_arena[item].kind else {
            return;
        };
        let mut reads: HashSet<String> = HashSet::new();
        let mut visit = |ast: &SourceFile, e: Idx<Expr>| {
            if let ExprKind::Path(p) = &ast.exprs[e].kind {
                if let [seg] = p.segments.as_slice() {
                    reads.insert(seg.text.clone());
                }
            }
        };
        for &s in &m.body {
            value::walk_stmt_exprs(&self.ast, s, &mut visit);
        }
        for (_, leaves) in declared {
            for leaf in leaves {
                if !reads.contains(leaf) {
                    self.notes
                        .unused
                        .insert((self.module.clone(), leaf.clone()));
                }
            }
        }
    }
}
