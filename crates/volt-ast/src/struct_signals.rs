//! Bir modül sinyalinin struct tipi (ADR-0077 Karar 5).
//!
//! Struct tipli her sinyal SV'de yaprak başına bir sinyaldir (`p` →
//! `p_a`, `p_b`). Hangi sinyalin struct olduğu, tip bildirilmişse
//! tipinden, tipsiz `let`/`reg`'de değerinden çıkar: struct literali,
//! başka bir struct sinyali ya da sabiti, `if`/`match`, `as P`,
//! `prev`/`sync`, kullanıcı modülü örneğinin struct portu, struct öğeli
//! primitifin veri çıkışı (ADR-0087).
//!
//! Kural tek yerde (#373): sv-emit'in struct indirgemesi (yaprak
//! sinyalleri) ve volt-hir'in kısıt yürüyücüsü (senkronizör hücre adları)
//! aynı cevabı buradan okur. İkisi ayrı hesaplarken tipsiz bir `let`'in
//! `sync()`'i SV'de alan başına senkronizördü, kısıtta var olmayan tek
//! hücre.

use std::collections::HashMap;

use crate::builtin::{BuiltinPrim, PortKind};
use crate::struct_layout::{self, StructLayout};
use crate::{
    Block, BlockStmt, ElseBranch, Expr, ExprKind, GenericArg, Idx, IfStmt, ItemKind, LetDecl,
    MatchArmBody, ModuleDecl, PortDir, SourceFile, StmtKind, TypeRef, TypeRefKind,
};

/// Birimin struct tip bilgisi: düzenler, modüllerin struct portları,
/// struct sabitleri.
#[derive(Debug, Clone, Default)]
pub struct StructTypes {
    /// Düz struct adı → düzen (kurulamıyorsa `None`).
    layouts: HashMap<String, Option<StructLayout>>,
    /// Modül adı → (struct tipli port adı → struct adı).
    module_ports: HashMap<String, HashMap<String, String>>,
    /// `const K : P = …` → (struct adı, değer).
    consts: HashMap<String, (String, Idx<Expr>)>,
}

/// Bir modülün struct tipli sinyalleri ve örnekleri.
#[derive(Debug, Clone, Default)]
pub struct ModuleSignals {
    /// Struct tipli sinyal → struct adı.
    pub sigs: HashMap<String, String>,
    /// Örnek adı → hedef modülün (ya da primitifin) adı.
    pub insts: HashMap<String, String>,
    /// Struct öğeli primitif örneği → (öğe struct'ı, veri çıkış portları).
    prim_outs: HashMap<String, (String, Vec<&'static str>)>,
}

impl StructTypes {
    pub fn new(ast: &SourceFile) -> Self {
        let mut types = StructTypes::default();
        for &i in &ast.items {
            if let ItemKind::Struct(s) = &ast.items_arena[i].kind {
                if !s.is_port {
                    let l = struct_layout::layout(ast, s, &mut |e| {
                        crate::const_eval::const_int(ast, e)
                    })
                    .ok();
                    types.layouts.insert(s.name.text.clone(), l);
                }
            }
        }
        for &i in &ast.items {
            match &ast.items_arena[i].kind {
                ItemKind::Module(m) => {
                    let ports: HashMap<String, String> = m
                        .ports
                        .iter()
                        .filter_map(|p| {
                            Some((p.name.text.clone(), types.struct_of_type(ast, p.ty)?))
                        })
                        .collect();
                    types.module_ports.insert(m.name.text.clone(), ports);
                }
                ItemKind::Const(c) => {
                    if let Some(s) = types.struct_of_type(ast, c.ty) {
                        types.consts.insert(c.name.text.clone(), (s, c.value));
                    }
                }
                ItemKind::Domain(_)
                | ItemKind::Struct(_)
                | ItemKind::Enum(_)
                | ItemKind::TypeAlias(_)
                | ItemKind::Extern(_)
                | ItemKind::Fn(_)
                | ItemKind::Test(_)
                | ItemKind::Error => {}
            }
        }
        types
    }

    /// Tip, düzeni geçerli bir düz struct'ı adlandırıyorsa adı. Generic,
    /// struct dizisi ya da düzeni kurulamayan struct yapraklara açılmaz —
    /// emitter'ın tip eşlemesi onlara E0003 verir.
    pub fn struct_of_type(&self, ast: &SourceFile, ty: Idx<TypeRef>) -> Option<String> {
        let decl = struct_layout::struct_of_type(ast, ty)?;
        let target = match crate::alias::alias_chain_end(ast, ty) {
            Ok(t) | Err(t) => t,
        };
        if let TypeRefKind::Path { args, .. } = &ast.types[target].kind {
            if !args.is_empty() {
                return None;
            }
        }
        let name = decl.name.text.clone();
        self.layouts.get(&name)?.as_ref().map(|_| name)
    }

    /// Düzeni geçerli düz struct'ın düzeni.
    pub fn layout(&self, name: &str) -> Option<&StructLayout> {
        self.layouts.get(name)?.as_ref()
    }

    /// Kullanıcı modülünün struct tipli portları (port → struct adı).
    pub fn module_ports(&self, module: &str) -> Option<&HashMap<String, String>> {
        self.module_ports.get(module)
    }

    /// Struct sabiti: (struct adı, değer).
    pub fn const_struct(&self, name: &str) -> Option<&(String, Idx<Expr>)> {
        self.consts.get(name)
    }

    /// Modülün struct tipli sinyalleri: portlar, `reg`, `wire`, modül ve
    /// blok düzeyi `let`'ler. Tipsiz `let`/`reg` değerinden çıkarılır; iki
    /// tur bildirim sırası bağımlılığını çözer (`let a = b` `b`'den önce).
    pub fn module_signals(&self, ast: &SourceFile, m: &ModuleDecl) -> ModuleSignals {
        let mut env = ModuleSignals::default();
        for p in &m.ports {
            if let Some(s) = self.struct_of_type(ast, p.ty) {
                env.sigs.insert(p.name.text.clone(), s);
            }
        }
        for _ in 0..2 {
            for &stmt in &m.body {
                let entry = match &ast.stmts[stmt].kind {
                    StmtKind::Reg(r) => Some((
                        r.name.text.clone(),
                        match r.ty {
                            Some(t) => self.struct_of_type(ast, t),
                            None => self.struct_of_expr(ast, &env, r.init),
                        },
                    )),
                    StmtKind::Wire(w) => {
                        Some((w.name.text.clone(), self.struct_of_type(ast, w.ty)))
                    }
                    StmtKind::Let(l) => Some((l.name.text.clone(), self.let_struct(ast, &env, l))),
                    StmtKind::Instance(inst) => {
                        if let [seg] = inst.module_path.segments.as_slice() {
                            env.insts.insert(inst.name.text.clone(), seg.text.clone());
                            if let Some(outs) = self.prim_data_outs(ast, &seg.text, inst) {
                                env.prim_outs.insert(inst.name.text.clone(), outs);
                            }
                        }
                        None
                    }
                    StmtKind::On(on) => {
                        self.scan_block_lets(ast, &mut env, on.body);
                        None
                    }
                    StmtKind::Comb(b) => {
                        self.scan_block_lets(ast, &mut env, *b);
                        None
                    }
                    // Let bildirmeyen deyimler (modül düzeyi `for` parser'da açıldı, ADR-0056).
                    StmtKind::Assign(_)
                    | StmtKind::For(_)
                    | StmtKind::Expr(_)
                    | StmtKind::Error => None,
                };
                if let Some((name, Some(s))) = entry {
                    env.sigs.insert(name, s);
                }
            }
        }
        env
    }

    /// Struct öğeli primitif örneğinin (`SyncFifo<P, 8>`) öğe struct'ı ve
    /// veri çıkış portları (ADR-0087: `m.rd_data` bir `P` değeridir).
    fn prim_data_outs(
        &self,
        ast: &SourceFile,
        target: &str,
        inst: &crate::InstanceDecl,
    ) -> Option<(String, Vec<&'static str>)> {
        let prim = BuiltinPrim::from_name(target)?;
        let Some(GenericArg::Type(t)) = inst.generic_args.first() else {
            return None;
        };
        let s = self.struct_of_type(ast, *t)?;
        let outs = prim
            .ports()
            .iter()
            .filter(|p| p.kind == PortKind::Data && p.dir == PortDir::Out)
            .map(|p| p.name)
            .collect();
        Some((s, outs))
    }

    fn let_struct(&self, ast: &SourceFile, env: &ModuleSignals, l: &LetDecl) -> Option<String> {
        match l.ty {
            Some(t) => self.struct_of_type(ast, t),
            None => self.struct_of_expr(ast, env, l.value),
        }
    }

    /// Blok içi struct `let`'leri (ad modül çapında tekil sayılır).
    fn scan_block_lets(&self, ast: &SourceFile, env: &mut ModuleSignals, block: Idx<Block>) {
        for s in &ast.blocks[block].stmts {
            match s {
                BlockStmt::Let(l) => {
                    if let Some(st) = self.let_struct(ast, env, l) {
                        env.sigs.insert(l.name.text.clone(), st);
                    }
                }
                BlockStmt::If(i) => self.scan_if_lets(ast, env, i),
                BlockStmt::Match(m) => {
                    for arm in &m.arms {
                        if let MatchArmBody::Block(b) = arm.body {
                            self.scan_block_lets(ast, env, b);
                        }
                    }
                }
                BlockStmt::For(f) => self.scan_block_lets(ast, env, f.body),
                // Let taşımayan deyimler.
                BlockStmt::NonBlockAssign { .. }
                | BlockStmt::BlockAssign { .. }
                | BlockStmt::Error => {}
            }
        }
    }

    fn scan_if_lets(&self, ast: &SourceFile, env: &mut ModuleSignals, i: &IfStmt) {
        self.scan_block_lets(ast, env, i.then_block);
        match &i.else_branch {
            Some(ElseBranch::Block(b)) => self.scan_block_lets(ast, env, *b),
            Some(ElseBranch::If(n)) => self.scan_if_lets(ast, env, n),
            None => {}
        }
    }

    /// İfadenin struct tipi (adı); struct değilse `None`. Tip denetimi
    /// geçmiştir — yalnız struct değerli biçimler tanınır.
    pub fn struct_of_expr(
        &self,
        ast: &SourceFile,
        env: &ModuleSignals,
        e: Idx<Expr>,
    ) -> Option<String> {
        match &ast.exprs[e].kind {
            ExprKind::Path(p) if p.segments.len() == 1 => {
                let n = &p.segments[0].text;
                env.sigs
                    .get(n)
                    .cloned()
                    .or_else(|| self.consts.get(n).map(|(s, _)| s.clone()))
            }
            ExprKind::Field { base, field } => {
                if let Some(inst) = single_name(ast, *base) {
                    if let Some((s, outs)) = env.prim_outs.get(inst) {
                        return outs.contains(&field.text.as_str()).then(|| s.clone());
                    }
                    if let Some(target) = env.insts.get(inst) {
                        return self.module_ports.get(target)?.get(&field.text).cloned();
                    }
                }
                let s = self.struct_of_expr(ast, env, *base)?;
                let decl = struct_layout::struct_named(ast, &s)?;
                let f = decl.fields.iter().find(|f| f.name.text == field.text)?;
                self.struct_of_type(ast, f.ty)
            }
            ExprKind::StructLit { path, .. } => {
                let name = &path.segments.last()?.text;
                self.layout(name).map(|_| name.clone())
            }
            ExprKind::Cast { ty, .. } => self.struct_of_type(ast, *ty),
            ExprKind::If {
                then_expr,
                else_expr,
                ..
            } => self
                .struct_of_expr(ast, env, *then_expr)
                .or_else(|| self.struct_of_expr(ast, env, *else_expr)),
            // Match ifadesi `if` gibi: ilk struct tipli kol (ADR-0083 Karar 2).
            ExprKind::Match { arms, .. } => arms.iter().find_map(|a| match a.body {
                MatchArmBody::Expr(e) => self.struct_of_expr(ast, env, e),
                MatchArmBody::Block(_) => None,
            }),
            ExprKind::Call { callee, args } => match single_name(ast, *callee) {
                Some("prev" | "sync" | "sync3") => self.struct_of_expr(ast, env, *args.first()?),
                _ => None,
            },
            ExprKind::IntLit { .. }
            | ExprKind::BoolLit(_)
            | ExprKind::StringLit(_)
            | ExprKind::Path(_)
            | ExprKind::Binary { .. }
            | ExprKind::Unary { .. }
            | ExprKind::Index { .. }
            | ExprKind::Range { .. }
            | ExprKind::PartSelect { .. }
            | ExprKind::ArrayLit(_)
            | ExprKind::TupleLit(_)
            | ExprKind::Concat(_)
            | ExprKind::Todo { .. }
            | ExprKind::Error => None,
        }
    }
}

/// Tek segmentli yolun adı.
fn single_name(ast: &SourceFile, e: Idx<Expr>) -> Option<&str> {
    let ExprKind::Path(p) = &ast.exprs[e].kind else {
        return None;
    };
    (p.segments.len() == 1).then(|| p.segments[0].text.as_str())
}
