//! İsim çözümleme (docs/spec/name-resolution.md).
//!
//! İki geçiş: öğe seviyesinde ileri referans serbest (Geçiş 1),
//! modül/blok gövdeleri sıralı — bildirim kullanımdan önce (Geçiş 2,
//! ihlalde E1002). Prelude yerleşikleri §7'de; Trit prelude'de DEĞİL
//! (opt-in import, UX Anayasası).

use std::collections::{HashMap, HashSet};

use volt_ast::{
    ArrayLitKind, Block, BlockStmt, ElseBranch, Expr, ExprKind, GenericArg, GenericParamKind, Idx,
    Item, ItemKind, LValue, LValueSuffix, MatchArm, MatchArmBody, ModuleDecl, Name, OnTrigger,
    Path, Pattern, PatternArgs, PatternKind, PortDir, SourceFile, Stmt, StmtKind, TypeRef,
    TypeRefKind, UseTree, Visibility,
};
use volt_diagnostics::{Applicability, Diagnostic, ErrorCode, LabeledSpan, NoteKind, Suggestion};
use volt_span::{FileId, Span};

// ═══ Kimlikler ════════════════════════════════════════════════════

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct DefId(pub u32);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DefKind {
    // ── Öğe seviyesi ──
    Module,
    Domain,
    Function,
    Struct,
    Enum,
    EnumVariant {
        parent: DefId,
    },
    Const,
    TypeAlias,
    ExternModule,

    // ── Modül içi ──
    Port {
        dir: PortDir,
    },
    Register,
    Wire,
    Instance,

    // ── Yerel ──
    LocalBinding,
    LoopVar,
    PatternBinding,
    GenericParam,

    // ── Yerleşik ──
    Builtin(BuiltinKind),

    /// `use` ile getirilen dış isim — F1b tek dosya derlediği için
    /// gövdesi çözülmez; kullanımlar hatasız kabul edilir (W1005 izler).
    Import,

    /// Hata kurtarma — her kullanımla uyumlu.
    Error,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BuiltinKind {
    Sync,
    Sync3,
    Zext,
    Sext,
    Trunc,
    Concat,
    Replicate,
    PopCount,
    Clog2,
}

#[derive(Debug)]
pub struct DefData {
    pub kind: DefKind,
    pub name: String,
    /// Bildirim konumu — hata mesajlarında "burada tanımlı".
    pub span: Span,
    pub scope: ScopeId,
    pub is_public: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ScopeId(pub u32);

#[derive(Debug)]
pub struct Scope {
    pub kind: ScopeKind,
    pub parent: Option<ScopeId>,
    pub bindings: HashMap<String, DefId>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScopeKind {
    Root,
    Prelude,
    Module(DefId),
    Function(DefId),
    Block,
    Loop,
    MatchArm,
}

// ═══ Sonuç ════════════════════════════════════════════════════════

#[derive(Debug, Default)]
pub struct ResolveResult {
    pub defs: Vec<DefData>,
    pub diagnostics: Vec<Diagnostic>,
    /// Path ifadesi → çözülen tanım (const eval bu haritayı kullanır).
    pub resolutions: HashMap<Idx<Expr>, DefId>,
    /// Const tanımı → başlangıç ifadesi.
    pub const_inits: HashMap<DefId, Idx<Expr>>,
    /// Enum varyantı → (sıra, açık discriminant ifadesi).
    pub variant_info: HashMap<DefId, (usize, Option<Idx<Expr>>)>,
    /// Bildirim isminin span'ı → tanım (tip denetçisi bildirimleri bulur).
    pub decl_spans: HashMap<Span, DefId>,
    /// Kullanım isminin span'ı → tanım (lvalue tabanları için).
    pub use_spans: HashMap<Span, DefId>,
    /// Tip konumundaki Path → tanım (struct/enum/alias tipleri).
    pub type_resolutions: HashMap<Idx<TypeRef>, DefId>,
    /// Okunan tanımlar (W4001/W4002 sürücü analizi için).
    pub reads: HashSet<DefId>,
    /// Instance tanımı → hedef modül tanımı.
    pub instance_module: HashMap<DefId, DefId>,
    /// Öğe tanımı → AST öğesi (port/alan tip araması için).
    pub item_of_def: HashMap<DefId, Idx<Item>>,
}

impl ResolveResult {
    pub fn error_codes(&self) -> Vec<&'static str> {
        self.diagnostics.iter().map(|d| d.code.as_str()).collect()
    }

    pub fn def_kind(&self, def: DefId) -> DefKind {
        self.defs[def.0 as usize].kind
    }

    /// Test yardımcısı: isme göre ilk tanım.
    pub fn def_by_name(&self, name: &str) -> Option<(DefId, &DefData)> {
        self.defs
            .iter()
            .enumerate()
            .find(|(_, d)| d.name == name)
            .map(|(i, d)| (DefId(i as u32), d))
    }
}

/// Bir kaynak dosyanın tüm isimlerini çözer.
pub fn resolve_file(ast: &SourceFile) -> ResolveResult {
    let mut r = Resolver::new(ast);
    r.run();
    r.finish()
}

// ═══ Çözücü ═══════════════════════════════════════════════════════

struct Resolver<'a> {
    ast: &'a SourceFile,
    defs: Vec<DefData>,
    scopes: Vec<Scope>,
    diagnostics: Vec<Diagnostic>,
    resolutions: HashMap<Idx<Expr>, DefId>,
    const_inits: HashMap<DefId, Idx<Expr>>,
    variant_info: HashMap<DefId, (usize, Option<Idx<Expr>>)>,
    decl_spans: HashMap<Span, DefId>,
    use_spans: HashMap<Span, DefId>,
    type_resolutions: HashMap<Idx<TypeRef>, DefId>,

    prelude: ScopeId,
    root: ScopeId,
    error_def: DefId,

    /// Okuma kullanımları (W1001/W1004/W1005/W3004 için).
    reads: HashSet<DefId>,
    /// Yazma kullanımları (W1004 yardım metni için).
    writes: HashSet<DefId>,
    /// "İleride bildirilecek" yığını — E1002 tespiti.
    pending: Vec<HashMap<String, Span>>,
    /// Öğe tanımı → AST öğesi (port/alan araması için).
    item_of_def: HashMap<DefId, Idx<Item>>,
    /// Enum tanımı → varyant isim/def listesi.
    enum_variants: HashMap<DefId, Vec<(String, DefId)>>,
    /// Instance tanımı → hedef modül tanımı.
    instance_module: HashMap<DefId, DefId>,
    /// Modül örnekleme kenarları (E1006 döngü tespiti).
    instance_edges: Vec<(DefId, DefId)>,
}

impl<'a> Resolver<'a> {
    fn new(ast: &'a SourceFile) -> Self {
        let mut r = Resolver {
            ast,
            defs: Vec::new(),
            scopes: Vec::new(),
            diagnostics: Vec::new(),
            resolutions: HashMap::new(),
            const_inits: HashMap::new(),
            variant_info: HashMap::new(),
            decl_spans: HashMap::new(),
            use_spans: HashMap::new(),
            type_resolutions: HashMap::new(),
            prelude: ScopeId(0),
            root: ScopeId(0),
            error_def: DefId(0),
            reads: HashSet::new(),
            writes: HashSet::new(),
            pending: Vec::new(),
            item_of_def: HashMap::new(),
            enum_variants: HashMap::new(),
            instance_module: HashMap::new(),
            instance_edges: Vec::new(),
        };
        r.prelude = r.new_scope(ScopeKind::Prelude, None);
        r.root = r.new_scope(ScopeKind::Root, Some(r.prelude));
        // Hata kurtarma tanımı — çözülemeyen her isim buna bağlanır.
        r.error_def = r.add_def(DefKind::Error, "<hata>", synthetic_span(), r.prelude, true);
        r.init_prelude();
        r
    }

    fn run(&mut self) {
        self.collect_imports();
        // ── Geçiş 1: öğe toplama (ileri referans serbest) ──
        for &item in &self.ast.items {
            self.collect_item(item);
        }
        // ── Geçiş 2: gövde çözümleme ──
        for &item in &self.ast.items {
            self.resolve_item_body(item);
        }
        self.check_instance_cycles();
        self.report_unused();
    }

    fn finish(self) -> ResolveResult {
        ResolveResult {
            defs: self.defs,
            diagnostics: self.diagnostics,
            resolutions: self.resolutions,
            const_inits: self.const_inits,
            variant_info: self.variant_info,
            decl_spans: self.decl_spans,
            use_spans: self.use_spans,
            type_resolutions: self.type_resolutions,
            reads: self.reads,
            instance_module: self.instance_module,
            item_of_def: self.item_of_def,
        }
    }

    // ═══ Altyapı ══════════════════════════════════════════════════

    fn new_scope(&mut self, kind: ScopeKind, parent: Option<ScopeId>) -> ScopeId {
        let id = ScopeId(self.scopes.len() as u32);
        self.scopes.push(Scope {
            kind,
            parent,
            bindings: HashMap::new(),
        });
        id
    }

    fn add_def(
        &mut self,
        kind: DefKind,
        name: &str,
        span: Span,
        scope: ScopeId,
        is_public: bool,
    ) -> DefId {
        let id = DefId(self.defs.len() as u32);
        self.defs.push(DefData {
            kind,
            name: name.to_string(),
            span,
            scope,
            is_public,
        });
        id
    }

    fn declare(&mut self, name: &Name, kind: DefKind, scope: ScopeId, is_public: bool) -> DefId {
        let def = self.add_def(kind, &name.text, name.span, scope, is_public);
        self.scopes[scope.0 as usize]
            .bindings
            .insert(name.text.clone(), def);
        self.decl_spans.insert(name.span, def);
        def
    }

    /// Aynı kapsamda çift tanım E1003; dış kapsam gölgelemesi W1002/W1003.
    fn declare_checked(
        &mut self,
        name: &Name,
        kind: DefKind,
        scope: ScopeId,
        is_public: bool,
    ) -> DefId {
        if let Some(&prev) = self.scopes[scope.0 as usize].bindings.get(&name.text) {
            let prev_span = self.defs[prev.0 as usize].span;
            self.diagnostics.push(
                Diagnostic::error(
                    ErrorCode::E1003,
                    format!("'{}' bu kapsamda zaten tanımlı", name.text),
                    LabeledSpan::primary(name.span, "ikinci tanım burada"),
                    "farklı bir isim kullanın — donanımda iki sinyal aynı adı taşıyamaz",
                )
                .with_secondary(prev_span, "önceki tanım burada"),
            );
            return self.error_def;
        }

        if let Some(outer) = self.lookup_in_parents(&name.text, scope) {
            let outer_data = &self.defs[outer.0 as usize];
            if let DefKind::Builtin(_) = outer_data.kind {
                self.diagnostics.push(Diagnostic::warning(
                    ErrorCode::W1003,
                    format!("yerleşik '{}' gölgeleniyor", name.text),
                    LabeledSpan::primary(name.span, "bu tanım yerleşiği gizler"),
                    "farklı bir isim seçin — yerleşik fonksiyon bu kapsamda erişilmez olur",
                ));
            } else if !matches!(outer_data.kind, DefKind::Error | DefKind::Import) {
                let outer_span = outer_data.span;
                self.diagnostics.push(
                    Diagnostic::warning(
                        ErrorCode::W1002,
                        format!("'{}' dış kapsamdaki tanımı gölgeliyor", name.text),
                        LabeledSpan::primary(name.span, "iç tanım burada"),
                        "karışıklığı önlemek için farklı bir isim kullanın",
                    )
                    .with_secondary(outer_span, "gölgelenen tanım burada"),
                );
            }
        }

        self.declare(name, kind, scope, is_public)
    }

    fn lookup_in_parents(&self, name: &str, scope: ScopeId) -> Option<DefId> {
        let mut current = self.scopes[scope.0 as usize].parent;
        while let Some(s) = current {
            if let Some(&def) = self.scopes[s.0 as usize].bindings.get(name) {
                return Some(def);
            }
            current = self.scopes[s.0 as usize].parent;
        }
        None
    }

    fn init_prelude(&mut self) {
        // name-resolution.md §7 — Trit bilinçli olarak YOK (opt-in import).
        const BUILTINS: &[(&str, BuiltinKind)] = &[
            ("sync", BuiltinKind::Sync),
            ("sync3", BuiltinKind::Sync3),
            ("zext", BuiltinKind::Zext),
            ("sext", BuiltinKind::Sext),
            ("trunc", BuiltinKind::Trunc),
            ("concat", BuiltinKind::Concat),
            ("replicate", BuiltinKind::Replicate),
            ("popcount", BuiltinKind::PopCount),
            ("clog2", BuiltinKind::Clog2),
        ];
        for &(name, kind) in BUILTINS {
            let def = self.add_def(
                DefKind::Builtin(kind),
                name,
                synthetic_span(),
                self.prelude,
                true,
            );
            self.scopes[self.prelude.0 as usize]
                .bindings
                .insert(name.to_string(), def);
        }
    }

    // ═══ Import'lar ═══════════════════════════════════════════════

    fn collect_imports(&mut self) {
        let mut names: Vec<Name> = Vec::new();
        for use_decl in &self.ast.uses {
            match &use_decl.tree {
                Some(UseTree::Alias(alias)) => names.push(alias.clone()),
                Some(UseTree::List(paths)) => {
                    for p in paths {
                        if let Some(last) = p.segments.last() {
                            names.push(last.clone());
                        }
                    }
                }
                // Glob: hangi isimlerin geldiği bilinemez — F1b'de izlenmez.
                Some(UseTree::Glob) => {}
                None => {
                    if let Some(last) = use_decl.path.segments.last() {
                        names.push(last.clone());
                    }
                }
            }
        }
        for name in names {
            if let Some(&prev) = self.scopes[self.root.0 as usize].bindings.get(&name.text) {
                let prev_span = self.defs[prev.0 as usize].span;
                self.diagnostics.push(
                    Diagnostic::error(
                        ErrorCode::E1010,
                        format!("belirsiz import: '{}' iki kez getiriliyor", name.text),
                        LabeledSpan::primary(name.span, "ikinci import burada"),
                        "birine 'as' ile takma ad verin: use yol::öğe as YeniAd",
                    )
                    .with_secondary(prev_span, "ilk import burada"),
                );
                continue;
            }
            self.declare(&name, DefKind::Import, self.root, false);
        }
    }

    // ═══ Geçiş 1: öğe toplama ═════════════════════════════════════

    fn collect_item(&mut self, item_idx: Idx<Item>) {
        let item = &self.ast.items_arena[item_idx];
        let is_public = item.visibility == Visibility::Public;
        let def = match &item.kind {
            ItemKind::Module(m) => {
                Some(self.declare_checked(&m.name.clone(), DefKind::Module, self.root, is_public))
            }
            ItemKind::Domain(d) => {
                Some(self.declare_checked(&d.name.clone(), DefKind::Domain, self.root, is_public))
            }
            ItemKind::Fn(f) => {
                Some(self.declare_checked(&f.name.clone(), DefKind::Function, self.root, is_public))
            }
            ItemKind::Struct(s) => {
                Some(self.declare_checked(&s.name.clone(), DefKind::Struct, self.root, is_public))
            }
            ItemKind::Enum(e) => {
                let name = e.name.clone();
                let enum_def = self.declare_checked(&name, DefKind::Enum, self.root, is_public);
                let mut variants = Vec::new();
                for (i, v) in e.variants.iter().enumerate() {
                    let v_def = self.add_def(
                        DefKind::EnumVariant { parent: enum_def },
                        &v.name.text,
                        v.name.span,
                        self.root,
                        is_public,
                    );
                    self.variant_info.insert(v_def, (i, v.discriminant));
                    variants.push((v.name.text.clone(), v_def));
                }
                self.enum_variants.insert(enum_def, variants);
                Some(enum_def)
            }
            ItemKind::Const(c) => {
                let def =
                    self.declare_checked(&c.name.clone(), DefKind::Const, self.root, is_public);
                self.const_inits.insert(def, c.value);
                Some(def)
            }
            ItemKind::TypeAlias(t) => Some(self.declare_checked(
                &t.name.clone(),
                DefKind::TypeAlias,
                self.root,
                is_public,
            )),
            ItemKind::Extern(x) => Some(self.declare_checked(
                &x.name.clone(),
                DefKind::ExternModule,
                self.root,
                is_public,
            )),
            ItemKind::Error => None,
        };
        if let Some(def) = def {
            self.item_of_def.insert(def, item_idx);
        }
    }

    // ═══ Geçiş 2: gövde çözümleme ═════════════════════════════════

    fn resolve_item_body(&mut self, item_idx: Idx<Item>) {
        let item = &self.ast.items_arena[item_idx];
        match &item.kind {
            ItemKind::Module(m) => self.resolve_module_body(m),
            ItemKind::Domain(d) => {
                for field in &d.fields {
                    if let volt_ast::DomainValue::Literal(expr) = &field.value {
                        self.resolve_expr(*expr, self.root);
                    }
                }
            }
            ItemKind::Fn(f) => {
                let fn_def = self.lookup_item_def(&f.name.text);
                let scope = self.new_scope(ScopeKind::Function(fn_def), Some(self.root));
                for g in &f.generics {
                    self.declare_generic(g, scope);
                }
                for p in &f.params {
                    self.declare_checked(&p.name.clone(), DefKind::LocalBinding, scope, false);
                    self.resolve_type(p.ty, scope);
                }
                if let Some(ret) = f.return_ty {
                    self.resolve_type(ret, scope);
                }
                for c in &f.contracts {
                    self.resolve_expr(c.expr, scope);
                }
                self.resolve_block(f.body, scope);
            }
            ItemKind::Struct(s) => {
                let scope = self.new_scope(ScopeKind::Block, Some(self.root));
                for g in &s.generics {
                    self.declare_generic(g, scope);
                }
                for field in &s.fields {
                    self.resolve_type(field.ty, scope);
                }
            }
            ItemKind::Enum(e) => {
                if let Some(repr) = e.repr {
                    self.resolve_type(repr, self.root);
                }
                for v in &e.variants {
                    if let Some(disc) = v.discriminant {
                        self.resolve_expr(disc, self.root);
                    }
                }
            }
            ItemKind::Const(c) => {
                self.resolve_type(c.ty, self.root);
                self.resolve_expr(c.value, self.root);
            }
            ItemKind::TypeAlias(t) => {
                let scope = self.new_scope(ScopeKind::Block, Some(self.root));
                for g in &t.generics {
                    self.declare_generic(g, scope);
                }
                self.resolve_type(t.target, scope);
            }
            ItemKind::Extern(x) => {
                let scope = self.new_scope(ScopeKind::Block, Some(self.root));
                for g in &x.generics {
                    self.declare_generic(g, scope);
                }
                for p in &x.ports {
                    self.resolve_type(p.ty, scope);
                }
            }
            ItemKind::Error => {}
        }
    }

    fn lookup_item_def(&self, name: &str) -> DefId {
        self.scopes[self.root.0 as usize]
            .bindings
            .get(name)
            .copied()
            .unwrap_or(self.error_def)
    }

    fn declare_generic(&mut self, g: &volt_ast::GenericParam, scope: ScopeId) {
        match &g.kind {
            GenericParamKind::Type { name, .. } => {
                self.declare_checked(&name.clone(), DefKind::GenericParam, scope, false);
            }
            GenericParamKind::Const { name, ty } => {
                let (name, ty) = (name.clone(), *ty);
                self.declare_checked(&name, DefKind::GenericParam, scope, false);
                self.resolve_type(ty, scope);
            }
        }
    }

    fn resolve_module_body(&mut self, m: &ModuleDecl) {
        let module_def = self.lookup_item_def(&m.name.text);
        let scope = self.new_scope(ScopeKind::Module(module_def), Some(self.root));

        // 1. Generic parametreler (en önce).
        for g in &m.generics {
            self.declare_generic(g, scope);
        }

        // 2. Portlar — birbirini görebilir, sıra anlamsal bilgi taşımaz.
        for p in &m.ports {
            self.declare_checked(
                &p.name.clone(),
                DefKind::Port { dir: p.direction },
                scope,
                false,
            );
        }
        for p in &m.ports {
            self.resolve_type(p.ty, scope);
            if let Some(domain) = &p.domain {
                self.resolve_domain_ref(&domain.clone(), scope);
            }
        }

        // 3. Kontratlar.
        for c in &m.contracts {
            self.resolve_expr(c.expr, scope);
        }

        // 4. Gövde — SIRALI; ileride bildirilecekler E1002 için izlenir.
        let mut later: HashMap<String, Span> = HashMap::new();
        for &stmt in &m.body {
            if let Some(name) = declared_name(&self.ast.stmts[stmt].kind) {
                later.entry(name.text.clone()).or_insert(name.span);
            }
        }
        self.pending.push(later);
        for &stmt in &m.body {
            self.resolve_stmt(stmt, scope, module_def);
        }
        self.pending.pop();
    }

    fn resolve_domain_ref(&mut self, name: &Name, scope: ScopeId) {
        let def = self.resolve_simple(name, scope, true);
        let kind = self.defs[def.0 as usize].kind;
        if !matches!(
            kind,
            DefKind::Domain | DefKind::Error | DefKind::Import | DefKind::Port { .. }
        ) {
            self.diagnostics.push(Diagnostic::error(
                ErrorCode::E3002,
                format!("'{}' bir saat alanı değil", name.text),
                LabeledSpan::primary(name.span, "domain bekleniyor"),
                "domain Ad { clock = posedge ... } ile tanımlanmış bir isim kullanın",
            ));
        }
    }

    // ═══ Deyimler ═════════════════════════════════════════════════

    fn resolve_stmt(&mut self, stmt_idx: Idx<Stmt>, scope: ScopeId, module_def: DefId) {
        let stmt = &self.ast.stmts[stmt_idx];
        match &stmt.kind {
            StmtKind::Reg(r) => {
                if let Some(ty) = r.ty {
                    self.resolve_type(ty, scope);
                }
                if let Some(domain) = &r.domain {
                    self.resolve_domain_ref(&domain.clone(), scope);
                }
                self.resolve_expr(r.init, scope);
                self.mark_declared(&r.name.text);
                self.declare_checked(&r.name.clone(), DefKind::Register, scope, false);
            }
            StmtKind::Let(l) => {
                if let Some(ty) = l.ty {
                    self.resolve_type(ty, scope);
                }
                self.resolve_expr(l.value, scope);
                self.mark_declared(&l.name.text);
                self.declare_checked(&l.name.clone(), DefKind::LocalBinding, scope, false);
            }
            StmtKind::Wire(w) => {
                self.resolve_type(w.ty, scope);
                self.mark_declared(&w.name.text);
                self.declare_checked(&w.name.clone(), DefKind::Wire, scope, false);
            }
            StmtKind::Instance(inst) => {
                let target = self.resolve_instance_target(&inst.module_path.clone(), scope);
                for arg in &inst.generic_args {
                    match arg {
                        GenericArg::Type(ty) => self.resolve_type(*ty, scope),
                        GenericArg::Const(e) => self.resolve_expr(*e, scope),
                    }
                }
                for b in &inst.bindings {
                    if let Some(target) = target {
                        self.check_port_exists(target, &b.port_name.clone());
                    }
                    match b.value {
                        Some(e) => self.resolve_expr(e, scope),
                        // `clk:` kısayolu — yerel isim port adıyla aynı.
                        None => {
                            let _ = self.resolve_simple(&b.port_name.clone(), scope, true);
                        }
                    }
                }
                self.mark_declared(&inst.name.text);
                let inst_def =
                    self.declare_checked(&inst.name.clone(), DefKind::Instance, scope, false);
                if let Some(target) = target {
                    self.instance_module.insert(inst_def, target);
                    self.instance_edges.push((module_def, target));
                }
            }
            StmtKind::On(on) => {
                match &on.trigger {
                    OnTrigger::Clock(name) | OnTrigger::Reset(name) => {
                        let _ = self.resolve_simple(&name.clone(), scope, true);
                    }
                    OnTrigger::Error => {}
                }
                self.resolve_block(on.body, scope);
            }
            StmtKind::Comb(block) => self.resolve_block(*block, scope),
            StmtKind::Assign(a) => {
                self.resolve_lvalue(&a.lhs, scope);
                self.resolve_expr(a.rhs, scope);
            }
            StmtKind::For(f) => self.resolve_for(f, scope),
            StmtKind::Expr(e) => self.resolve_expr(*e, scope),
            StmtKind::Error => {}
        }
    }

    fn resolve_instance_target(&mut self, path: &Path, scope: ScopeId) -> Option<DefId> {
        let first = path.segments.first()?.clone();
        let def = self.resolve_simple(&first, scope, true);
        // soc::uart::Uart gibi çok segmentli yollar F2 (paketler) işi.
        let kind = self.defs[def.0 as usize].kind;
        match kind {
            DefKind::Module | DefKind::ExternModule => Some(def),
            DefKind::Error | DefKind::Import => None,
            DefKind::Struct => None, // struct literal — tip kontrolü işi
            _ => {
                self.diagnostics.push(Diagnostic::error(
                    ErrorCode::E1001,
                    format!("'{}' bir modül değil", first.text),
                    LabeledSpan::primary(first.span, "modül bekleniyor"),
                    "örneklenecek isim bir module ya da extern module olmalı",
                ));
                None
            }
        }
    }

    fn check_port_exists(&mut self, module_def: DefId, port_name: &Name) {
        let Some(&item_idx) = self.item_of_def.get(&module_def) else {
            return;
        };
        let ports = match &self.ast.items_arena[item_idx].kind {
            ItemKind::Module(m) => &m.ports,
            ItemKind::Extern(x) => &x.ports,
            _ => return,
        };
        if !ports.iter().any(|p| p.name.text == port_name.text) {
            let module_name = self.defs[module_def.0 as usize].name.clone();
            let candidates: Vec<String> = ports.iter().map(|p| p.name.text.clone()).collect();
            let suggestion = closest_match(&port_name.text, &candidates);
            let mut diag = Diagnostic::error(
                ErrorCode::E1009,
                format!("'{}' modülünde '{}' portu yok", module_name, port_name.text),
                LabeledSpan::primary(port_name.span, "bilinmeyen port"),
                match &suggestion {
                    Some(s) => format!("'{s}' mi demek istediniz?"),
                    None => format!("mevcut portlar: {}", candidates.join(", ")),
                },
            );
            if let Some(s) = suggestion {
                diag = diag.with_suggestion(Suggestion {
                    span: port_name.span,
                    replacement: s,
                    applicability: Applicability::MaybeIncorrect,
                });
            }
            self.diagnostics.push(diag);
        }
    }

    fn resolve_for(&mut self, f: &volt_ast::ForStmt, scope: ScopeId) {
        self.resolve_expr(f.start, scope);
        self.resolve_expr(f.end, scope);
        let loop_scope = self.new_scope(ScopeKind::Loop, Some(scope));
        self.declare_checked(&f.var.clone(), DefKind::LoopVar, loop_scope, false);
        self.resolve_block(f.body, loop_scope);
    }

    fn resolve_lvalue(&mut self, lv: &LValue, scope: ScopeId) {
        let def = self.resolve_simple(&lv.base.clone(), scope, false);
        let mut current = Some(def);
        for suffix in &lv.suffixes {
            match suffix {
                LValueSuffix::Index(e) => {
                    self.resolve_expr(*e, scope);
                    current = None;
                }
                LValueSuffix::Range { hi, lo } => {
                    self.resolve_expr(*hi, scope);
                    self.resolve_expr(*lo, scope);
                    current = None;
                }
                LValueSuffix::Field(field) => {
                    // uart.busy hedefi: instance portu doğrulanabilir.
                    if let Some(base) = current {
                        if let Some(&target) = self.instance_module.get(&base) {
                            self.check_port_exists(target, &field.clone());
                        }
                    }
                    current = None;
                }
            }
        }
    }

    // ═══ Bloklar ══════════════════════════════════════════════════

    fn resolve_block(&mut self, block_idx: Idx<Block>, parent: ScopeId) {
        let block = &self.ast.blocks[block_idx];
        let scope = self.new_scope(ScopeKind::Block, Some(parent));

        let mut later: HashMap<String, Span> = HashMap::new();
        for stmt in &block.stmts {
            if let BlockStmt::Let(l) = stmt {
                later.entry(l.name.text.clone()).or_insert(l.name.span);
            }
        }
        self.pending.push(later);
        for stmt in &block.stmts {
            self.resolve_block_stmt(stmt, scope);
        }
        if let Some(tail) = block.tail {
            self.resolve_expr(tail, scope);
        }
        self.pending.pop();
    }

    fn resolve_block_stmt(&mut self, stmt: &BlockStmt, scope: ScopeId) {
        match stmt {
            BlockStmt::NonBlockAssign { lhs, rhs, .. }
            | BlockStmt::BlockAssign { lhs, rhs, .. } => {
                self.resolve_lvalue(lhs, scope);
                self.resolve_expr(*rhs, scope);
            }
            BlockStmt::If(if_stmt) => self.resolve_if(if_stmt, scope),
            BlockStmt::Match(m) => {
                self.resolve_expr(m.scrutinee, scope);
                for arm in &m.arms {
                    self.resolve_arm(arm, scope);
                }
            }
            BlockStmt::Let(l) => {
                if let Some(ty) = l.ty {
                    self.resolve_type(ty, scope);
                }
                self.resolve_expr(l.value, scope);
                self.mark_declared(&l.name.text);
                self.declare_checked(&l.name.clone(), DefKind::LocalBinding, scope, false);
            }
            BlockStmt::For(f) => self.resolve_for(f, scope),
            BlockStmt::Error => {}
        }
    }

    fn resolve_if(&mut self, if_stmt: &volt_ast::IfStmt, scope: ScopeId) {
        self.resolve_expr(if_stmt.cond, scope);
        self.resolve_block(if_stmt.then_block, scope);
        match &if_stmt.else_branch {
            Some(ElseBranch::Block(b)) => self.resolve_block(*b, scope),
            Some(ElseBranch::If(nested)) => self.resolve_if(nested, scope),
            None => {}
        }
    }

    fn resolve_arm(&mut self, arm: &MatchArm, scope: ScopeId) {
        let arm_scope = self.new_scope(ScopeKind::MatchArm, Some(scope));
        self.resolve_pattern(arm.pattern, arm_scope);
        if let Some(guard) = arm.guard {
            self.resolve_expr(guard, arm_scope);
        }
        match &arm.body {
            MatchArmBody::Block(b) => self.resolve_block(*b, arm_scope),
            MatchArmBody::Expr(e) => self.resolve_expr(*e, arm_scope),
        }
    }

    fn resolve_pattern(&mut self, pat_idx: Idx<Pattern>, scope: ScopeId) {
        let pat = &self.ast.patterns[pat_idx];
        match &pat.kind {
            PatternKind::Wildcard | PatternKind::Error => {}
            PatternKind::Literal(e) => self.resolve_expr(*e, scope),
            PatternKind::Binding(name) => {
                self.declare_checked(&name.clone(), DefKind::PatternBinding, scope, false);
            }
            PatternKind::Path { path, args } => {
                self.resolve_path(&path.clone(), scope);
                match args {
                    Some(PatternArgs::Tuple(pats)) => {
                        for &p in pats {
                            self.resolve_pattern(p, scope);
                        }
                    }
                    Some(PatternArgs::Struct(fields)) => {
                        for f in fields {
                            match f.pattern {
                                Some(p) => self.resolve_pattern(p, scope),
                                // `Foo { x }` kısayolu x'i bağlar.
                                None => {
                                    self.declare_checked(
                                        &f.name.clone(),
                                        DefKind::PatternBinding,
                                        scope,
                                        false,
                                    );
                                }
                            }
                        }
                    }
                    None => {}
                }
            }
            PatternKind::Tuple(pats) | PatternKind::Or(pats) => {
                for &p in pats.clone().iter() {
                    self.resolve_pattern(p, scope);
                }
            }
        }
    }

    // ═══ Tipler ═══════════════════════════════════════════════════

    fn resolve_type(&mut self, ty_idx: Idx<TypeRef>, scope: ScopeId) {
        let ty = &self.ast.types[ty_idx];
        match &ty.kind {
            TypeRefKind::Bits(e) => self.resolve_expr(*e, scope),
            TypeRefKind::Array { elem, len } => {
                self.resolve_type(*elem, scope);
                self.resolve_expr(*len, scope);
            }
            TypeRefKind::Tuple(items) => {
                for &t in items.clone().iter() {
                    self.resolve_type(t, scope);
                }
            }
            TypeRefKind::Path { path, args } => {
                // u65..uN / i65..iN genişletilmiş tipleri parser Path olarak
                // taşır (widened types) — bunlar isim değil, yerleşik ailedir.
                if path.segments.len() == 1 && is_widened_int_type(&path.segments[0].text) {
                    return;
                }
                let def = self.resolve_path(&path.clone(), scope);
                self.type_resolutions.insert(ty_idx, def);
                for arg in args {
                    match arg {
                        GenericArg::Type(t) => self.resolve_type(*t, scope),
                        GenericArg::Const(e) => self.resolve_expr(*e, scope),
                    }
                }
            }
            // Yerleşik tipler (bool, clock, uN, bits, Trit, reset) isim değildir.
            _ => {}
        }
    }

    // ═══ İfadeler ═════════════════════════════════════════════════

    fn resolve_expr(&mut self, expr_idx: Idx<Expr>, scope: ScopeId) {
        let expr = &self.ast.exprs[expr_idx];
        match &expr.kind {
            ExprKind::Path(path) => {
                let def = self.resolve_path(&path.clone(), scope);
                self.resolutions.insert(expr_idx, def);
            }
            ExprKind::Binary { lhs, rhs, .. } => {
                self.resolve_expr(*lhs, scope);
                self.resolve_expr(*rhs, scope);
            }
            ExprKind::Unary { operand, .. } => self.resolve_expr(*operand, scope),
            ExprKind::Index { base, index } => {
                self.resolve_expr(*base, scope);
                self.resolve_expr(*index, scope);
            }
            ExprKind::Range { base, hi, lo } => {
                self.resolve_expr(*base, scope);
                self.resolve_expr(*hi, scope);
                self.resolve_expr(*lo, scope);
            }
            ExprKind::Field { base, field } => {
                let (base, field) = (*base, field.clone());
                self.resolve_expr(base, scope);
                // uart.busy: taban bir instance'a çözülüyorsa portu doğrula.
                if let Some(&base_def) = self.resolutions.get(&base) {
                    if let Some(&target) = self.instance_module.get(&base_def) {
                        self.check_port_exists(target, &field);
                    }
                }
            }
            ExprKind::Call { callee, args } => {
                self.resolve_expr(*callee, scope);
                for &a in args.clone().iter() {
                    self.resolve_expr(a, scope);
                }
            }
            ExprKind::Cast { expr: inner, ty } => {
                self.resolve_expr(*inner, scope);
                self.resolve_type(*ty, scope);
            }
            ExprKind::If {
                cond,
                then_expr,
                else_expr,
            } => {
                self.resolve_expr(*cond, scope);
                self.resolve_expr(*then_expr, scope);
                self.resolve_expr(*else_expr, scope);
            }
            ExprKind::Match { scrutinee, arms } => {
                self.resolve_expr(*scrutinee, scope);
                for arm in arms {
                    self.resolve_arm(arm, scope);
                }
            }
            ExprKind::StructLit { path, fields } => {
                let def = self.resolve_path(&path.clone(), scope);
                self.resolutions.insert(expr_idx, def);
                self.check_struct_fields(def, fields);
                for f in fields.iter() {
                    match f.value {
                        Some(e) => self.resolve_expr(e, scope),
                        None => {
                            let _ = self.resolve_simple(&f.name.clone(), scope, true);
                        }
                    }
                }
            }
            ExprKind::ArrayLit(ArrayLitKind::List(items)) => {
                for &i in items.clone().iter() {
                    self.resolve_expr(i, scope);
                }
            }
            ExprKind::ArrayLit(ArrayLitKind::Repeat { value, count }) => {
                self.resolve_expr(*value, scope);
                self.resolve_expr(*count, scope);
            }
            ExprKind::TupleLit(items) => {
                for &i in items.clone().iter() {
                    self.resolve_expr(i, scope);
                }
            }
            ExprKind::IntLit { .. }
            | ExprKind::BoolLit(_)
            | ExprKind::StringLit(_)
            | ExprKind::Todo { .. }
            | ExprKind::Error => {}
        }
    }

    fn check_struct_fields(&mut self, def: DefId, fields: &[volt_ast::FieldInit]) {
        if self.defs[def.0 as usize].kind != DefKind::Struct {
            return;
        }
        let Some(&item_idx) = self.item_of_def.get(&def) else {
            return;
        };
        let ItemKind::Struct(s) = &self.ast.items_arena[item_idx].kind else {
            return;
        };
        let known: Vec<String> = s.fields.iter().map(|f| f.name.text.clone()).collect();
        let struct_name = s.name.text.clone();
        let mut diags = Vec::new();
        for f in fields {
            if !known.contains(&f.name.text) {
                diags.push(Diagnostic::error(
                    ErrorCode::E1008,
                    format!("'{}' yapısında '{}' alanı yok", struct_name, f.name.text),
                    LabeledSpan::primary(f.name.span, "bilinmeyen alan"),
                    match closest_match(&f.name.text, &known) {
                        Some(s) => format!("'{s}' mi demek istediniz?"),
                        None => format!("mevcut alanlar: {}", known.join(", ")),
                    },
                ));
            }
        }
        self.diagnostics.extend(diags);
    }

    // ═══ Yol çözümleme ════════════════════════════════════════════

    fn resolve_path(&mut self, path: &Path, scope: ScopeId) -> DefId {
        let Some(first) = path.segments.first() else {
            return self.error_def;
        };
        if path.segments.len() == 1 {
            return self.resolve_simple(&first.clone(), scope, true);
        }

        // Çok segment: State::Idle vb.
        let mut current = self.resolve_simple(&first.clone(), scope, true);
        for seg in &path.segments[1..] {
            let kind = self.defs[current.0 as usize].kind;
            current = match kind {
                DefKind::Enum => self.lookup_variant(current, seg),
                DefKind::Error | DefKind::Import => return self.error_def,
                _ => {
                    let name = self.defs[current.0 as usize].name.clone();
                    self.diagnostics.push(Diagnostic::error(
                        ErrorCode::E1005,
                        format!("'{name}' bir ad alanı değil"),
                        LabeledSpan::primary(path.span, "'::' burada kullanılamaz"),
                        "'::' yalnız enum ve paket yollarında geçerlidir; \
                         sinyal erişimi için '.' kullanın",
                    ));
                    return self.error_def;
                }
            };
        }
        current
    }

    fn lookup_variant(&mut self, enum_def: DefId, seg: &Name) -> DefId {
        let variants = self
            .enum_variants
            .get(&enum_def)
            .cloned()
            .unwrap_or_default();
        if let Some((_, def)) = variants.iter().find(|(n, _)| *n == seg.text) {
            self.reads.insert(*def);
            return *def;
        }
        let enum_name = self.defs[enum_def.0 as usize].name.clone();
        let names: Vec<String> = variants.iter().map(|(n, _)| n.clone()).collect();
        self.diagnostics.push(Diagnostic::error(
            ErrorCode::E1007,
            format!("'{}' enum'ında '{}' varyantı yok", enum_name, seg.text),
            LabeledSpan::primary(seg.span, "bilinmeyen varyant"),
            match closest_match(&seg.text, &names) {
                Some(s) => format!("'{s}' mi demek istediniz?"),
                None => format!("mevcut varyantlar: {}", names.join(", ")),
            },
        ));
        self.error_def
    }

    fn resolve_simple(&mut self, name: &Name, scope: ScopeId, is_read: bool) -> DefId {
        let mut current = Some(scope);
        while let Some(s) = current {
            if let Some(&def) = self.scopes[s.0 as usize].bindings.get(&name.text) {
                if is_read {
                    self.reads.insert(def);
                } else {
                    self.writes.insert(def);
                }
                self.use_spans.insert(name.span, def);
                return def;
            }
            current = self.scopes[s.0 as usize].parent;
        }
        self.error_unresolved(name, scope);
        self.error_def
    }

    /// E1002 (ileride bildirilmiş) veya E1001 (hiç yok) üretir.
    fn error_unresolved(&mut self, name: &Name, scope: ScopeId) {
        for later in self.pending.iter().rev() {
            if let Some(&decl_span) = later.get(&name.text) {
                self.diagnostics.push(
                    Diagnostic::error(
                        ErrorCode::E1002,
                        format!("'{}' bu noktada henüz tanımlı değil", name.text),
                        LabeledSpan::primary(name.span, "burada kullanılıyor"),
                        format!("'{}' bildirimini bu kullanımdan yukarı taşıyın", name.text),
                    )
                    .with_secondary(decl_span, "ama burada tanımlanıyor")
                    .with_note(
                        NoteKind::Reason,
                        "modül içi bildirimler kullanımdan önce gelmeli",
                    ),
                );
                return;
            }
        }

        let candidates = self.visible_names(scope);
        let suggestion = closest_match(&name.text, &candidates);
        let mut diag = Diagnostic::error(
            ErrorCode::E1001,
            format!("tanımsız isim: '{}'", name.text),
            LabeledSpan::primary(name.span, "bu isim çözülemedi"),
            match &suggestion {
                Some(s) => format!("'{s}' mi demek istediniz?"),
                None => "bu isim hiçbir kapsamda tanımlı değil".to_string(),
            },
        );
        if let Some(s) = suggestion {
            diag = diag.with_suggestion(Suggestion {
                span: name.span,
                replacement: s,
                applicability: Applicability::MaybeIncorrect,
            });
        }
        self.diagnostics.push(diag);
    }

    fn visible_names(&self, scope: ScopeId) -> Vec<String> {
        let mut names = Vec::new();
        let mut current = Some(scope);
        while let Some(s) = current {
            names.extend(self.scopes[s.0 as usize].bindings.keys().cloned());
            current = self.scopes[s.0 as usize].parent;
        }
        names
    }

    /// İsim bildirildi — E1002 bekleyenler listesinden düş.
    fn mark_declared(&mut self, name: &str) {
        if let Some(top) = self.pending.last_mut() {
            top.remove(name);
        }
    }

    // ═══ E1006: modül örnekleme döngüsü ═══════════════════════════

    fn check_instance_cycles(&mut self) {
        let mut edges: HashMap<DefId, Vec<DefId>> = HashMap::new();
        for &(from, to) in &self.instance_edges {
            edges.entry(from).or_default().push(to);
        }
        let starts: Vec<DefId> = edges.keys().copied().collect();
        let mut visited: HashSet<DefId> = HashSet::new();
        let mut reported: HashSet<DefId> = HashSet::new();
        for start in starts {
            let mut stack = vec![start];
            let mut path_set = HashSet::new();
            self.dfs_cycle(
                start,
                &edges,
                &mut visited,
                &mut path_set,
                &mut stack,
                &mut reported,
            );
        }
    }

    fn dfs_cycle(
        &mut self,
        node: DefId,
        edges: &HashMap<DefId, Vec<DefId>>,
        visited: &mut HashSet<DefId>,
        path_set: &mut HashSet<DefId>,
        stack: &mut Vec<DefId>,
        reported: &mut HashSet<DefId>,
    ) {
        if path_set.contains(&node) {
            if reported.insert(node) {
                let cycle: Vec<String> = stack
                    .iter()
                    .skip_while(|d| **d != node)
                    .map(|d| self.defs[d.0 as usize].name.clone())
                    .collect();
                let span = self.defs[node.0 as usize].span;
                self.diagnostics.push(
                    Diagnostic::error(
                        ErrorCode::E1006,
                        "döngüsel modül bağımlılığı",
                        LabeledSpan::primary(span, "döngü bu modülden başlıyor"),
                        "örnekleme zincirindeki bağımlılıklardan birini kaldırın",
                    )
                    .with_note(
                        NoteKind::Note,
                        format!("döngü: {} → {}", cycle.join(" → "), cycle[0]),
                    ),
                );
            }
            return;
        }
        if !visited.insert(node) {
            return;
        }
        path_set.insert(node);
        if let Some(next) = edges.get(&node) {
            for &n in next.clone().iter() {
                stack.push(n);
                self.dfs_cycle(n, edges, visited, path_set, stack, reported);
                stack.pop();
            }
        }
        path_set.remove(&node);
    }

    // ═══ Kullanım raporlama (§9) ══════════════════════════════════

    fn report_unused(&mut self) {
        let mut warnings = Vec::new();
        for (i, data) in self.defs.iter().enumerate() {
            let def = DefId(i as u32);
            // '_' önekli isimler muaf (UX Anayasası).
            if data.name.starts_with('_') {
                continue;
            }
            // Public öğeler muaf (dışarıdan kullanılabilir).
            if data.is_public {
                continue;
            }
            if self.reads.contains(&def) {
                continue;
            }
            let (code, msg) = match data.kind {
                DefKind::Port { dir: PortDir::In } => {
                    (ErrorCode::W1001, "kullanılmayan giriş portu")
                }
                DefKind::Register => {
                    if self.writes.contains(&def) {
                        (ErrorCode::W1004, "yazılıp hiç okunmayan register")
                    } else {
                        (ErrorCode::W1004, "kullanılmayan register")
                    }
                }
                DefKind::Wire | DefKind::LocalBinding => {
                    (ErrorCode::W1001, "kullanılmayan bağlama")
                }
                DefKind::Domain => (ErrorCode::W3004, "kullanılmayan domain tanımı"),
                DefKind::Import => (ErrorCode::W1005, "kullanılmayan import"),
                _ => continue,
            };
            warnings.push(Diagnostic::warning(
                code,
                format!("{}: '{}'", msg, data.name),
                LabeledSpan::primary(data.span, "burada tanımlı, hiç okunmuyor"),
                format!("'_' öneki ekleyerek susturabilirsiniz: _{}", data.name),
            ));
        }
        self.diagnostics.extend(warnings);
    }
}

/// Modül gövdesinde isim bildiren deyimler (E1002 ön taraması).
fn declared_name(kind: &StmtKind) -> Option<&Name> {
    match kind {
        StmtKind::Reg(r) => Some(&r.name),
        StmtKind::Let(l) => Some(&l.name),
        StmtKind::Wire(w) => Some(&w.name),
        StmtKind::Instance(i) => Some(&i.name),
        _ => None,
    }
}

fn synthetic_span() -> Span {
    Span::new(FileId(u32::MAX), 0, 0)
}

/// `u9`, `i128` gibi genişlik-sonekli yerleşik tam sayı tipleri.
pub(crate) fn is_widened_int_type(name: &str) -> bool {
    let Some(rest) = name.strip_prefix(['u', 'i']) else {
        return false;
    };
    !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit())
}

// ═══ Levenshtein önerisi (§8) ═════════════════════════════════════

/// Eşik: isim uzunluğunun üçte biri (en az 1).
pub fn closest_match(name: &str, candidates: &[String]) -> Option<String> {
    let threshold = (name.chars().count() / 3).max(1);
    candidates
        .iter()
        .filter(|c| !c.starts_with('<'))
        .map(|c| (c, levenshtein(name, c)))
        .filter(|(_, d)| *d <= threshold && *d > 0)
        .min_by_key(|(_, d)| *d)
        .map(|(c, _)| c.clone())
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}
