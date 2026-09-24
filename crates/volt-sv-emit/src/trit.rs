//! Trit SV eşlemesi (ADR-0003: 2 bit işaretli depolama).
//!
//! Kodlama ikiye tümleyen i2'dir: +1 = `2'sb01`, 0 = `2'sb00`,
//! -1 = `2'sb11` (`2'sb10` kullanılmaz; literal E2011, sayısal → Trit
//! dönüşümü E2009 ile dışlanır). Bu kodlamada `-t`, `t as iN` ve
//! `t + t` (i3) sıradan işaretli aritmetiktir; yalnız çarpma özeldir:
//! `Trit * x` çarpan yerine seçici üretir — +1 → x, -1 → -x, 0 → 0
//! (type-inference.md §3.3 "ternary MAC").

use volt_ast::{BinOp, Expr, ExprKind, Idx, NumBase, TypeRef, TypeRefKind, UnOp};

use crate::expr::{sv_prec, Sig, PREC_UNARY};
use crate::Emitter;

/// +1 ve -1'in SV kodlaması.
const TRIT_POS: &str = "2'sb01";
const TRIT_NEG: &str = "2'sb11";

impl Emitter<'_> {
    /// Tip `Trit` ya da `[Trit; N]` mi (dizi elemanı da Trit taşır).
    pub(crate) fn is_trit_typeref(&self, ty: Idx<TypeRef>) -> bool {
        match &self.ast.types[crate::alias::resolve(self.ast, ty)].kind {
            TypeRefKind::Trit => true,
            TypeRefKind::Array { elem, .. } => self.is_trit_typeref(*elem),
            _ => false,
        }
    }

    /// Adı Trit sinyali olarak kaydeder (port/wire/reg/let).
    pub(crate) fn note_trit(&mut self, name: &str, ty: Idx<TypeRef>) {
        if self.is_trit_typeref(ty) {
            self.trits.insert(name.to_string());
        }
    }

    /// İfade Trit değer mi üretir. Bilinmeyen biçimler `false` döner:
    /// o zaman jenerik `*` basılır — işaretli i2 kodlamasında yine
    /// aritmetik olarak doğrudur, yalnız çarpan üretir.
    pub(crate) fn is_trit(&self, idx: Idx<Expr>) -> bool {
        match &self.ast.exprs[idx].kind {
            ExprKind::Path(_) | ExprKind::Index { .. } => {
                let base = match &self.ast.exprs[idx].kind {
                    ExprKind::Index { base, .. } => *base,
                    _ => idx,
                };
                // Modül sembolü üst düzey const'u gölgeler (width_of sırası).
                crate::path_single(self.ast, base).is_some_and(|n| {
                    if self.symbols.contains_key(n) {
                        return self.trits.contains(n);
                    }
                    self.consts
                        .get(n)
                        .is_some_and(|&(ty, _)| self.is_trit_typeref(ty))
                })
            }
            ExprKind::Unary {
                op: UnOp::Neg,
                operand,
            } => self.is_trit(*operand),
            ExprKind::Binary {
                op: BinOp::Mul,
                lhs,
                rhs,
            } => self.is_trit(*lhs) && self.is_trit(*rhs),
            ExprKind::If {
                then_expr,
                else_expr,
                ..
            } => self.is_trit(*then_expr) && self.is_trit(*else_expr),
            _ => false,
        }
    }

    /// `Trit ± Trit` → i3 (taşma: +1 + +1 = +2); diğer durumlarda `None`.
    pub(crate) fn trit_sum_sig(&self, op: BinOp, lhs: Idx<Expr>, rhs: Idx<Expr>) -> Option<Sig> {
        (matches!(op, BinOp::Add | BinOp::Sub) && self.is_trit(lhs) && self.is_trit(rhs)).then_some(
            Sig {
                width: 3,
                signed: true,
            },
        )
    }

    /// `Trit * x` seçicisi; operandlardan biri Trit değilse `None`.
    /// `ctx`: çarpımın etkin bağlamı (x ve sıfır bu genişlikte basılır).
    pub(crate) fn try_emit_trit_mul(
        &mut self,
        lhs: Idx<Expr>,
        rhs: Idx<Expr>,
        ctx: Option<Sig>,
    ) -> Option<String> {
        let (t, x) = if self.is_trit(lhs) {
            (lhs, rhs)
        } else if self.is_trit(rhs) {
            (rhs, lhs)
        } else {
            return None;
        };
        let span = self.ast.exprs[x].span;
        let sel = self.emit_prec(t, None, sv_prec(BinOp::Eq) + 1, false);
        let val = self.emit_operand(x, ctx, PREC_UNARY, false);
        let zero = self.fmt_int(0, NumBase::Dec, ctx, span);
        Some(format!(
            "{sel} == {TRIT_POS} ? {val} : ({sel} == {TRIT_NEG} ? -({val}) : {zero})"
        ))
    }
}
