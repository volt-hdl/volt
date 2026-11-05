//! Üreticinin ad çözümü (name-resolution.md §2, §3.1; ADR-0081 Karar
//! 12.1 ve sonundaki "Ek: Üreticinin ad çözümü", #128).
//!
//! Emitter volt-hir'in çözüm tablosunu görmez (`emit_unit` yalnız AST
//! alır); tek parçalı bir değer yolunun hangi tanıma bağlandığını burada,
//! şartnamenin kapsam zinciriyle bulur: içten dışa blok kapsamları (süreç
//! yerelleri ve döngü değişkenleri, iç içe geçme sırasıyla), modül
//! kapsamı (port, reg, tel, modül `let`'i, örnek), kök kapsam (const).
//! Adı çözen her yer [`Emitter::resolve`]'den geçer; kendi sırasıyla
//! tabloya bakan yer kalmaz.
//!
//! **Kök kapsamlı yollar.** Bazı yollar kullanıldıkları yerin kapsamında
//! değil kök kapsamda çözülür; [`Emitter::resolve`] onlarda yereli, döngü
//! değişkenini ve modül adını atlar:
//! - açılmış fn gövdesinin const'a bağlı yolları (açılım notu,
//!   ADR-0081 Karar 12.1: fn kapsamının ebeveyni kök kapsamdır);
//! - fn bildiriminin tip ifadeleri (`uint<W>`) ve desenleri (`K =>`):
//!   açılım onları kopyalamaz, kaynak düğümleri doğrudan kullanılır;
//! - const öğelerinin değer ve tip ifadeleri (`const J = I + 1`): const
//!   kök kapsamda tanımlıdır, değeri kullanıldığı döngüde değişmez.

use std::collections::HashSet;

use volt_ast::visit::expr_children;
use volt_ast::{
    Expr, ExprKind, GenericArg, Idx, ItemKind, Pattern, PatternArgs, PatternKind, SourceFile,
    TypeRef, TypeRefKind,
};

use crate::Emitter;

/// Tek parçalı bir değer yolunun bu noktadaki çözümü.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Res {
    /// Süreç yereli (blok `let`'i): `proc.locals` indeksi.
    Local(usize),
    /// Açılmakta olan döngünün değişkeni: bu yinelemedeki değeri.
    Loop(i128),
    /// Modül adı: port, reg, tel, modül `let`'i, örnek.
    Module,
    /// Üst düzey const: bildirilen tipi ve değer ifadesi.
    Const { ty: Idx<TypeRef>, value: Idx<Expr> },
    /// Tek parçalı yol değil ya da hiçbir tanıma bağlanmıyor (enum
    /// varyantı, yerleşik adı, HIR'ın zaten tanıladığı tanımsız ad).
    Unbound,
}

impl<'a> Emitter<'a> {
    /// `idx` yolunun bu noktadaki çözümü (name-resolution.md §3.1: kapsam
    /// zinciri içten dışa). Kök kapsamlı yolda ([`root_scoped_paths`])
    /// blok ve modül kapsamları atlanır.
    pub(crate) fn resolve(&self, idx: Idx<Expr>) -> Res {
        let Some(name) = crate::path_single(self.ast, idx) else {
            return Res::Unbound;
        };
        if !self.root_paths.contains(&idx) {
            if let Some(res) = self.resolve_block(name) {
                return res;
            }
            if self.symbols.contains_key(name)
                || self.builtin_insts.contains_key(name)
                || self.user_insts.contains_key(name)
            {
                return Res::Module;
            }
        }
        match self.consts.get(name) {
            Some(&(ty, value)) => Res::Const { ty, value },
            None => Res::Unbound,
        }
    }

    /// Blok kapsamları: en içteki süreç yereli ya da döngü değişkeni.
    /// Döngü değişkeni, döngü başladığında açık olan blokların yerellerini
    /// gölgeler; döngü gövdesinde (ve daha içte) bildirilen yerel onu
    /// gölgeler.
    fn resolve_block(&self, name: &str) -> Option<Res> {
        let local = self.local_frame(name);
        let lv = self.loop_vars.iter().rev().find(|l| l.name == name);
        match (local, lv) {
            (Some((frame, _)), Some(l)) if frame < l.frame => Some(Res::Loop(l.value)),
            (Some((_, i)), _) => Some(Res::Local(i)),
            (None, Some(l)) => Some(Res::Loop(l.value)),
            (None, None) => None,
        }
    }

    /// Yol bir modül adına bağlanıyorsa adı (dizi, örnek ve sinyal
    /// tablolarının anahtarı).
    pub(crate) fn module_path(&self, idx: Idx<Expr>) -> Option<&'a str> {
        (self.resolve(idx) == Res::Module)
            .then(|| crate::path_single(self.ast, idx))
            .flatten()
    }

    /// Yol bir const'a bağlanıyorsa (ad, tip, değer).
    pub(crate) fn const_path(&self, idx: Idx<Expr>) -> Option<(&'a str, Idx<TypeRef>, Idx<Expr>)> {
        match self.resolve(idx) {
            Res::Const { ty, value } => Some((crate::path_single(self.ast, idx)?, ty, value)),
            Res::Local(_) | Res::Loop(_) | Res::Module | Res::Unbound => None,
        }
    }
}

/// Açılmakta olan döngünün değişkeni.
#[derive(Debug, Clone)]
pub(crate) struct LoopVar {
    pub(crate) name: String,
    pub(crate) value: i128,
    /// Döngü başladığında açık blok kapsamı sayısı: gövdenin kapsamı bu
    /// indekstedir.
    pub(crate) frame: usize,
}

/// Birimin kök kapsamlı yolları: const öğelerinin değer ve tip
/// ifadelerindeki bütün tek parçalı yollar; fn bildirimlerinin tip ve
/// desen ifadelerindeki yollar (gövde ifadelerininkini açılım işaretler).
pub(crate) fn root_scoped_paths(ast: &SourceFile) -> HashSet<Idx<Expr>> {
    let mut w = RootWalker {
        ast,
        out: HashSet::new(),
    };
    for &item in &ast.items {
        match &ast.items_arena[item].kind {
            ItemKind::Const(c) => {
                w.ty(c.ty);
                w.expr(c.value, true);
            }
            ItemKind::Fn(f) => {
                for p in &f.params {
                    w.ty(p.ty);
                }
                if let Some(t) = f.return_ty {
                    w.ty(t);
                }
                let block = &ast.blocks[f.body];
                for s in &block.stmts {
                    if let volt_ast::BlockStmt::Let(l) = s {
                        if let Some(t) = l.ty {
                            w.ty(t);
                        }
                        w.expr(l.value, false);
                    }
                }
                if let Some(tail) = block.tail {
                    w.expr(tail, false);
                }
            }
            ItemKind::Module(_)
            | ItemKind::Domain(_)
            | ItemKind::Struct(_)
            | ItemKind::Enum(_)
            | ItemKind::TypeAlias(_)
            | ItemKind::Extern(_)
            | ItemKind::Test(_)
            | ItemKind::Error => {}
        }
    }
    w.out
}

struct RootWalker<'s> {
    ast: &'s SourceFile,
    out: HashSet<Idx<Expr>>,
}

impl RootWalker<'_> {
    /// `all`: ağaçtaki her yol kök kapsamlı (const değeri, tip ve desen
    /// ifadesi); değilse yalnız tiplere ve desenlere inilir (fn gövdesi —
    /// parametre ve `let` bağlamalarını açılım çözer).
    fn expr(&mut self, root: Idx<Expr>, all: bool) {
        let mut stack = vec![root];
        while let Some(e) = stack.pop() {
            let kind = &self.ast.exprs[e].kind;
            match kind {
                ExprKind::Path(p) if all && p.segments.len() == 1 => {
                    self.out.insert(e);
                }
                ExprKind::Cast { ty, .. } => self.ty(*ty),
                ExprKind::Match { arms, .. } => {
                    for arm in arms {
                        self.pattern(arm.pattern);
                    }
                }
                ExprKind::IntLit { .. }
                | ExprKind::BoolLit(_)
                | ExprKind::StringLit(_)
                | ExprKind::Path(_)
                | ExprKind::Binary { .. }
                | ExprKind::Unary { .. }
                | ExprKind::Index { .. }
                | ExprKind::Range { .. }
                | ExprKind::PartSelect { .. }
                | ExprKind::Field { .. }
                | ExprKind::Call { .. }
                | ExprKind::If { .. }
                | ExprKind::StructLit { .. }
                | ExprKind::ArrayLit(_)
                | ExprKind::TupleLit(_)
                | ExprKind::Concat(_)
                | ExprKind::Todo { .. }
                | ExprKind::Error => {}
            }
            stack.extend(expr_children(kind));
        }
    }

    fn ty(&mut self, t: Idx<TypeRef>) {
        match &self.ast.types[t].kind {
            TypeRefKind::Bits(e) | TypeRefKind::UIntN(e) | TypeRefKind::SIntN(e) => {
                self.expr(*e, true);
            }
            TypeRefKind::Array { elem, len } => {
                self.ty(*elem);
                self.expr(*len, true);
            }
            TypeRefKind::Tuple(items) => {
                for &i in items {
                    self.ty(i);
                }
            }
            TypeRefKind::Path { args, .. } => {
                for a in args {
                    match a {
                        GenericArg::Type(t) => self.ty(*t),
                        GenericArg::Const(e) => self.expr(*e, true),
                    }
                }
            }
            TypeRefKind::Bool
            | TypeRefKind::Clock
            | TypeRefKind::Reset(_)
            | TypeRefKind::UInt(_)
            | TypeRefKind::SInt(_)
            | TypeRefKind::Trit
            | TypeRefKind::Error => {}
        }
    }

    fn pattern(&mut self, p: Idx<Pattern>) {
        match &self.ast.patterns[p].kind {
            PatternKind::Literal(e) => self.expr(*e, true),
            PatternKind::Tuple(items) | PatternKind::Or(items) => {
                for &i in items {
                    self.pattern(i);
                }
            }
            PatternKind::Path {
                args: Some(PatternArgs::Tuple(items)),
                ..
            } => {
                for &i in items {
                    self.pattern(i);
                }
            }
            PatternKind::Path {
                args: Some(PatternArgs::Struct(fields)),
                ..
            } => {
                for f in fields {
                    if let Some(i) = f.pattern {
                        self.pattern(i);
                    }
                }
            }
            PatternKind::Wildcard | PatternKind::Path { args: None, .. } | PatternKind::Error => {}
        }
    }
}
