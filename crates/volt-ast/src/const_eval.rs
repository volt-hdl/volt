//! Derleme zamanı tamsayı sabiti — tek değerlendirici.
//!
//! SV üreticisi (`Emitter::eval_const`: port, reg ve tel genişliği, dizi
//! uzunluğu, sabit indeks, döngü sınırı), enum ve struct düzeni, testbench
//! (`sim_width`), test dili (`sim_port`) ve LSP hover bu modülü çağırır.
//! Önceden iki değerlendirici vardı ve ayrışmıştı: struct düzeni, testbench
//! ve test dili 32 düzeyde duruyor, `if`/`match`/bit işlemlerini
//! bilmiyordu; SV üreticisinin port genişliği 64 düzeye iniyordu. 33 ile 64
//! sabitlik bir zincirle verilen port SV'de 4 bit, test dilinde bilinmiyordu
//! ve 4 bitlik porta 31 yazan test geçiyordu (#376, ADR-0132). Anlam ve
//! sınır SV üreticisinin port genişliğini hesaplayan kopyasınınkidir.
//!
//! Adın anlamı çağırana göre değişir (döngü değişkeni, yerel, gölgeleme):
//! [`Scope`]. Yalnız üst düzey sabitler için [`TopLevel`].

use std::cell::Cell;

use crate::{
    alias, enum_layout, ArrayLitKind, BinOp, EnumDecl, Expr, ExprKind, Idx, ItemKind, MatchArmBody,
    Path, Pattern, PatternKind, SourceFile, TypeRef, TypeRefKind, UnOp,
};

/// Değerlendirmenin en çok iç içeliği: her işlem, her sabit adı, her dizi
/// başvurusu bir düzey. Döngüsel `const` HIR'da E2020'dir; sınır savunmadır
/// ve aşıldığında sonuç [`Unknown::TooDeep`] olur (değer tahmin edilmez).
pub const MAX_CONST_DEPTH: u32 = 64;

/// `[v; N]` dizi sabitinin açılabilecek en uzun hâli.
pub const MAX_TABLE_LEN: u32 = 4096;

/// Tek parçalı bir adın değerlendirildiği yerdeki anlamı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binding {
    /// Değeri bilinen ad (açılmakta olan döngünün değişkeni).
    Value(i128),
    /// Üst düzey `const`: bildirilen tipi ve değer ifadesi.
    Const { ty: Idx<TypeRef>, value: Idx<Expr> },
    /// Sabit olmayan ad (sinyal, yerel) ya da hiçbir tanıma bağlanmayan ad.
    Other,
}

/// Adların çözüldüğü kapsam.
pub trait Scope {
    /// `path`'in (yol ifadesi) bu kapsamdaki anlamı; tek parçalı olmayan
    /// yol [`Binding::Other`]'dır.
    fn bind(&self, path: Idx<Expr>) -> Binding;
}

/// Yalnız üst düzey `const`'lar: tip ve düzen sabitleri.
pub struct TopLevel<'a>(pub &'a SourceFile);

impl Scope for TopLevel<'_> {
    fn bind(&self, path: Idx<Expr>) -> Binding {
        let ast = self.0;
        let ExprKind::Path(p) = &ast.exprs[path].kind else {
            return Binding::Other;
        };
        let [seg] = p.segments.as_slice() else {
            return Binding::Other;
        };
        ast.items
            .iter()
            .find_map(|&i| match &ast.items_arena[i].kind {
                ItemKind::Const(c) if c.name.text == seg.text => Some(Binding::Const {
                    ty: c.ty,
                    value: c.value,
                }),
                _ => None,
            })
            .unwrap_or(Binding::Other)
    }
}

/// Değerin neden bilinmediği.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unknown {
    /// İfade sabit değil, ya da değer hesaplanamıyor (taşma, sıfıra bölme,
    /// sınır dışı indeks).
    NotConstant,
    /// İfade [`MAX_CONST_DEPTH`] düzeyden derin.
    TooDeep,
}

/// `e`'nin `scope`'taki değeri.
pub fn eval(ast: &SourceFile, scope: &dyn Scope, e: Idx<Expr>) -> Result<i128, Unknown> {
    let ev = Eval::new(ast, scope);
    let v = ev.int(e, 0);
    ev.result(v)
}

/// Üst düzey sabit ifadesinin değeri (tip ve düzen sabitleri).
pub fn const_int(ast: &SourceFile, e: Idx<Expr>) -> Option<i128> {
    eval(ast, &TopLevel(ast), e).ok()
}

/// Tip genişliği (`uint<N>`, `sint<N>`, `bits<N>`): N en az 1 olan ve
/// `u32`'ye sığan sabit. SV üreticisi ve test dili port genişliğini bununla
/// hesaplar.
pub fn width(ast: &SourceFile, scope: &dyn Scope, e: Idx<Expr>) -> Result<u32, Unknown> {
    let n = eval(ast, scope, e)?;
    u32::try_from(n)
        .ok()
        .filter(|&w| w >= 1)
        .ok_or(Unknown::NotConstant)
}

/// Dizi sabitinin elemanları: `[a, b, c]`, `[v; N]` ya da başka bir dizi
/// sabitinin adı.
pub fn array_elements(ast: &SourceFile, scope: &dyn Scope, value: Idx<Expr>) -> Option<Vec<i128>> {
    Eval::new(ast, scope).elements(value, 0)
}

/// `base` dizi sabitine bağlanıyorsa `i`. elemanı.
pub fn array_element(
    ast: &SourceFile,
    scope: &dyn Scope,
    base: Idx<Expr>,
    i: i128,
) -> Option<i128> {
    Eval::new(ast, scope).element(base, i, 0)
}

struct Eval<'a> {
    ast: &'a SourceFile,
    scope: &'a dyn Scope,
    too_deep: Cell<bool>,
}

impl<'a> Eval<'a> {
    fn new(ast: &'a SourceFile, scope: &'a dyn Scope) -> Self {
        Eval {
            ast,
            scope,
            too_deep: Cell::new(false),
        }
    }

    fn result(&self, v: Option<i128>) -> Result<i128, Unknown> {
        match v {
            Some(v) => Ok(v),
            None if self.too_deep.get() => Err(Unknown::TooDeep),
            None => Err(Unknown::NotConstant),
        }
    }

    /// Sınır aşıldı mı (aşıldıysa not edilir).
    fn past_limit(&self, depth: u32) -> bool {
        let past = depth > MAX_CONST_DEPTH;
        if past {
            self.too_deep.set(true);
        }
        past
    }

    fn int(&self, e: Idx<Expr>, depth: u32) -> Option<i128> {
        if self.past_limit(depth) {
            return None;
        }
        match &self.ast.exprs[e].kind {
            ExprKind::IntLit { value, .. } => i128::try_from(*value).ok(),
            ExprKind::Unary {
                op: UnOp::Neg,
                operand,
            } => self.int(*operand, depth + 1)?.checked_neg(),
            // Mantıksal değer 0/1 (bool tipli const, `const N = if … `).
            ExprKind::BoolLit(_) | ExprKind::Unary { op: UnOp::Not, .. } => {
                self.bool(e, depth).map(i128::from)
            }
            ExprKind::Binary { op, .. } if is_comparison_or_logical(*op) => {
                self.bool(e, depth).map(i128::from)
            }
            // `const N = if C { a } else { b }` (ADR-0083): koşul
            // çözülemezse değer yok.
            ExprKind::If {
                cond,
                then_expr,
                else_expr,
            } => {
                let taken = if self.bool(*cond, depth + 1)? {
                    *then_expr
                } else {
                    *else_expr
                };
                self.int(taken, depth + 1)
            }
            ExprKind::Binary { op, lhs, rhs } => {
                let l = self.int(*lhs, depth + 1)?;
                let r = self.int(*rhs, depth + 1)?;
                arith(*op, l, r)
            }
            ExprKind::Path(_) => match self.scope.bind(e) {
                Binding::Value(v) => Some(v),
                Binding::Const { value, .. } => self.int(value, depth + 1),
                Binding::Other => None,
            },
            // Sabit dizi elemanı: `COEFFS[k]`, k sabit (ADR-0041).
            ExprKind::Index { base, index } => {
                let i = self.int(*index, depth + 1)?;
                self.element(*base, i, depth + 1)
            }
            // `const N : u32 = match K { … }` (ADR-0083): ilk eşleşen kolun
            // değeri; muhafızlı ya da çözülemeyen desen değer vermez.
            ExprKind::Match { scrutinee, arms } => {
                let key = self.match_key(*scrutinee, depth + 1)?;
                for arm in arms {
                    if arm.guard.is_some() {
                        return None;
                    }
                    if self.pattern_matches(arm.pattern, key, depth + 1)? {
                        let MatchArmBody::Expr(body) = arm.body else {
                            return None;
                        };
                        return self.int(body, depth + 1);
                    }
                }
                None
            }
            ExprKind::StringLit(_)
            | ExprKind::Unary { .. }
            | ExprKind::Range { .. }
            | ExprKind::PartSelect { .. }
            | ExprKind::Field { .. }
            | ExprKind::Call { .. }
            | ExprKind::Cast { .. }
            | ExprKind::StructLit { .. }
            | ExprKind::ArrayLit(_)
            | ExprKind::TupleLit(_)
            | ExprKind::Concat(_)
            | ExprKind::Todo { .. }
            | ExprKind::Error => None,
        }
    }

    /// Mantıksal değer: literal, karşılaştırma, `&&`/`||`/`->`, `!`; başka
    /// bir ifade 0 ya da 1 değerindeyse o.
    fn bool(&self, e: Idx<Expr>, depth: u32) -> Option<bool> {
        if self.past_limit(depth) {
            return None;
        }
        match &self.ast.exprs[e].kind {
            ExprKind::BoolLit(b) => Some(*b),
            ExprKind::Unary {
                op: UnOp::Not,
                operand,
            } => Some(!self.bool(*operand, depth + 1)?),
            ExprKind::Binary { op, lhs, rhs } if is_logical(*op) => {
                let l = self.bool(*lhs, depth + 1)?;
                let r = self.bool(*rhs, depth + 1)?;
                logic(*op, l, r)
            }
            ExprKind::Binary { op, lhs, rhs } if is_comparison_or_logical(*op) => {
                let l = self.int(*lhs, depth + 1)?;
                let r = self.int(*rhs, depth + 1)?;
                compare(*op, l, r)
            }
            ExprKind::IntLit { .. }
            | ExprKind::StringLit(_)
            | ExprKind::Path(_)
            | ExprKind::Binary { .. }
            | ExprKind::Unary { .. }
            | ExprKind::Index { .. }
            | ExprKind::Range { .. }
            | ExprKind::PartSelect { .. }
            | ExprKind::Field { .. }
            | ExprKind::Call { .. }
            | ExprKind::Cast { .. }
            | ExprKind::If { .. }
            | ExprKind::Match { .. }
            | ExprKind::StructLit { .. }
            | ExprKind::ArrayLit(_)
            | ExprKind::TupleLit(_)
            | ExprKind::Concat(_)
            | ExprKind::Todo { .. }
            | ExprKind::Error => match self.int(e, depth + 1)? {
                0 => Some(false),
                1 => Some(true),
                _ => None,
            },
        }
    }

    /// Sabit `match`'in sınananı: tamsayı ya da bool sabiti, ya da enum
    /// varyantının kodu (varyant yolu veya enum tipli `const`).
    fn match_key(&self, e: Idx<Expr>, depth: u32) -> Option<i128> {
        if self.past_limit(depth) {
            return None;
        }
        if let ExprKind::BoolLit(b) = self.ast.exprs[e].kind {
            return Some(i128::from(b));
        }
        if let ExprKind::Path(p) = &self.ast.exprs[e].kind {
            if let Some((decl, i)) = variant_of_path(self.ast, p) {
                return self.variant_code(decl, i, depth);
            }
            if let Binding::Const { ty, value } = self.scope.bind(e) {
                if enum_layout::enum_of_type(self.ast, ty).is_some() {
                    return self.match_key(value, depth + 1);
                }
            }
        }
        self.int(e, depth)
    }

    /// Desen sabit anahtarla eşleşiyor mu? (`None`: değerlendirilemez.)
    fn pattern_matches(&self, pat: Idx<Pattern>, key: i128, depth: u32) -> Option<bool> {
        match &self.ast.patterns[pat].kind {
            PatternKind::Wildcard => Some(true),
            PatternKind::Or(alts) => {
                for &a in alts {
                    if self.pattern_matches(a, key, depth)? {
                        return Some(true);
                    }
                }
                Some(false)
            }
            PatternKind::Literal(e) => Some(self.match_key(*e, depth)? == key),
            PatternKind::Path {
                path: p,
                args: None,
            } => {
                let (decl, i) = variant_of_path(self.ast, p)?;
                Some(self.variant_code(decl, i, depth)? == key)
            }
            PatternKind::Path { .. } | PatternKind::Tuple(_) | PatternKind::Error => None,
        }
    }

    /// Enum'un `i`. varyantının kodu (geçerli kodlama tablosundan).
    fn variant_code(&self, decl: &EnumDecl, i: usize, depth: u32) -> Option<i128> {
        let layout = enum_layout::valid_layout(self.ast, decl, &mut |e| self.int(e, depth + 1))?;
        i128::try_from(*layout.values.get(i)?).ok()
    }

    fn element(&self, base: Idx<Expr>, i: i128, depth: u32) -> Option<i128> {
        let Binding::Const { ty, value } = self.scope.bind(base) else {
            return None;
        };
        if !self.is_array(ty) {
            return None;
        }
        let elems = self.elements(value, depth + 1)?;
        usize::try_from(i).ok().and_then(|i| elems.get(i).copied())
    }

    fn elements(&self, value: Idx<Expr>, depth: u32) -> Option<Vec<i128>> {
        if self.past_limit(depth) {
            return None;
        }
        match &self.ast.exprs[value].kind {
            ExprKind::ArrayLit(ArrayLitKind::List(items)) => {
                items.iter().map(|&e| self.int(e, depth + 1)).collect()
            }
            ExprKind::ArrayLit(ArrayLitKind::Repeat { value, count }) => {
                let v = self.int(*value, depth + 1)?;
                let n = usize::try_from(self.int(*count, depth + 1)?).ok()?;
                (n as u128 <= u128::from(MAX_TABLE_LEN)).then(|| vec![v; n])
            }
            ExprKind::Path(_) => {
                let Binding::Const { ty, value: inner } = self.scope.bind(value) else {
                    return None;
                };
                if !self.is_array(ty) {
                    return None;
                }
                self.elements(inner, depth + 1)
            }
            ExprKind::IntLit { .. }
            | ExprKind::BoolLit(_)
            | ExprKind::StringLit(_)
            | ExprKind::Binary { .. }
            | ExprKind::Unary { .. }
            | ExprKind::Index { .. }
            | ExprKind::Range { .. }
            | ExprKind::PartSelect { .. }
            | ExprKind::Field { .. }
            | ExprKind::Call { .. }
            | ExprKind::Cast { .. }
            | ExprKind::If { .. }
            | ExprKind::Match { .. }
            | ExprKind::StructLit { .. }
            | ExprKind::TupleLit(_)
            | ExprKind::Concat(_)
            | ExprKind::Todo { .. }
            | ExprKind::Error => None,
        }
    }

    /// Tip (takma adlar izlenerek) bir dizi mi?
    fn is_array(&self, ty: Idx<TypeRef>) -> bool {
        let (Ok(end) | Err(end)) = alias::alias_chain_end(self.ast, ty);
        matches!(self.ast.types[end].kind, TypeRefKind::Array { .. })
    }
}

/// `State::Idle` yolu (son iki parça) → (enum, varyant sırası).
fn variant_of_path<'a>(ast: &'a SourceFile, path: &Path) -> Option<(&'a EnumDecl, usize)> {
    let n = path.segments.len();
    if n < 2 {
        return None;
    }
    let decl = enum_layout::enum_named(ast, &path.segments[n - 2].text)?;
    let i = decl
        .variants
        .iter()
        .position(|v| v.name.text == path.segments[n - 1].text)?;
    Some((decl, i))
}

fn is_logical(op: BinOp) -> bool {
    matches!(op, BinOp::And | BinOp::Or | BinOp::Imp)
}

fn is_comparison_or_logical(op: BinOp) -> bool {
    is_logical(op)
        || matches!(
            op,
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge
        )
}

/// Tamsayı işlemi: taşma ve sıfıra bölme `None`. Bit işlemleri yalnız
/// negatif olmayan değerde (genişlik bağlamı yok, ikiye tümleyen belirsiz);
/// `<<` değer bitini kaybederse `None`, `>>` mantıksal.
fn arith(op: BinOp, l: i128, r: i128) -> Option<i128> {
    match op {
        BinOp::Add => l.checked_add(r),
        BinOp::Sub => l.checked_sub(r),
        BinOp::Mul => l.checked_mul(r),
        BinOp::Div => l.checked_div(r),
        BinOp::Rem => l.checked_rem(r),
        _ if l < 0 || r < 0 => None,
        BinOp::BitAnd => Some(l & r),
        BinOp::BitOr => Some(l | r),
        BinOp::BitXor => Some(l ^ r),
        BinOp::Shl => l
            .checked_shl(u32::try_from(r).ok()?)
            .filter(|v| v >> r == l),
        BinOp::Shr => Some(if r >= 127 { 0 } else { l >> r }),
        // Karşılaştırma ve mantıksal işleçler sayı üretmez (`bool`).
        BinOp::Eq
        | BinOp::Ne
        | BinOp::Lt
        | BinOp::Gt
        | BinOp::Le
        | BinOp::Ge
        | BinOp::And
        | BinOp::Or
        | BinOp::Imp => None,
    }
}

fn compare(op: BinOp, l: i128, r: i128) -> Option<bool> {
    match op {
        BinOp::Eq => Some(l == r),
        BinOp::Ne => Some(l != r),
        BinOp::Lt => Some(l < r),
        BinOp::Gt => Some(l > r),
        BinOp::Le => Some(l <= r),
        BinOp::Ge => Some(l >= r),
        BinOp::Add
        | BinOp::Sub
        | BinOp::Mul
        | BinOp::Div
        | BinOp::Rem
        | BinOp::BitAnd
        | BinOp::BitOr
        | BinOp::BitXor
        | BinOp::Shl
        | BinOp::Shr
        | BinOp::And
        | BinOp::Or
        | BinOp::Imp => None,
    }
}

fn logic(op: BinOp, l: bool, r: bool) -> Option<bool> {
    match op {
        BinOp::And => Some(l && r),
        BinOp::Or => Some(l || r),
        BinOp::Imp => Some(!l || r),
        BinOp::Add
        | BinOp::Sub
        | BinOp::Mul
        | BinOp::Div
        | BinOp::Rem
        | BinOp::BitAnd
        | BinOp::BitOr
        | BinOp::BitXor
        | BinOp::Shl
        | BinOp::Shr
        | BinOp::Eq
        | BinOp::Ne
        | BinOp::Lt
        | BinOp::Gt
        | BinOp::Le
        | BinOp::Ge => None,
    }
}
