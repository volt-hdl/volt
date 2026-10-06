//! Tipsiz `let`'in tel genişliği (type-inference.md §3.3, ADR-0025).
//!
//! Tip denetimi aritmetik sonuca esnek bir genişlik aralığı verir:
//! `u8 + u8` → `[8, 9]`, `u8 * u8` → `[8, 16]`. Kullanıcıya gösterilen ve
//! ifade bağlamında geçerli olan genişlik aralığın üst ucu (doğal
//! genişlik) olduğundan tipsiz `let` teli o genişlikte bildirilir; dar
//! bir hedefe atama taşma bitini AÇIKÇA atar. Emitter tip denetimi
//! sonuçlarını kullanmadığı için (ADR-0041) aralık burada aynı kurallarla
//! yeniden hesaplanır: operandların ortak aralığı kesişimle bulunur,
//! toplama/çıkarma 1 bit, çarpma iki kat genişler; bölme, mod, bit düzeyi
//! işlemler ve kaydırma genişlemez.

use volt_ast::{BinOp, Expr, ExprKind, Idx, UnOp};

use crate::expr::Sig;
use crate::Emitter;

/// Sonuç genişliğinin üst sınırı (volt-hir `consteval::MAX_WIDTH`).
const MAX_WIDTH: u32 = 65_536;

/// Yalnız soneksiz literallerden oluşan tipsiz `let`in tipi
/// (type-inference.md §5, W2012): `i32`.
const UNSIZED_LITERAL: Sig = Sig {
    width: 32,
    signed: true,
};

/// Tam sayı ifadesinin genişlik aralığı `lo..=hi` (tip denetimindeki
/// `UIntFlex`/`SIntFlex` karşılığı); somut tipte `lo == hi`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Range {
    signed: bool,
    lo: u32,
    hi: u32,
}

impl Range {
    fn exact(sig: Sig) -> Self {
        Range {
            signed: sig.signed,
            lo: sig.width,
            hi: sig.width,
        }
    }

    fn sig(self) -> Sig {
        Sig {
            width: self.hi,
            signed: self.signed,
        }
    }

    /// İki aralığın ortak kısmı; işaret farkı ya da boş kesişim `None`
    /// (tip denetimi orada zaten hata verir).
    fn meet(self, other: Range) -> Option<Range> {
        let lo = self.lo.max(other.lo);
        let hi = self.hi.min(other.hi);
        (self.signed == other.signed && lo <= hi).then_some(Range {
            signed: self.signed,
            lo,
            hi,
        })
    }
}

impl Emitter<'_> {
    /// Tipsiz `let`'in tel imzası (doğal genişlik) ve esnek aralığın alt
    /// ucu. Alt uç saklanır ki zincirli `let`ler tip denetimiyle aynı
    /// ortak genişlikte buluşsun (`let t = s ^ a`, s: u8..u9 → u8).
    pub(crate) fn untyped_let_sig(&mut self, value: Idx<Expr>) -> Option<(Sig, u32)> {
        match self.int_range(value) {
            Some(range) => Some((range.sig(), range.lo)),
            None => self
                .width_of(value)
                .or_else(|| self.is_unsized_literal(value).then_some(UNSIZED_LITERAL))
                .map(|sig| (sig, sig.width)),
        }
    }

    /// Tip denetiminin `IntLit` sonucu veren ifadesi: soneksiz literal,
    /// onun `-`/`~`'i, iki literalin aritmetik, bit ya da kaydırma işlemi,
    /// iki dalı literal olan `if` (#80; W2012 bunu `i32` sayar).
    fn is_unsized_literal(&self, idx: Idx<Expr>) -> bool {
        match &self.ast.exprs[idx].kind {
            ExprKind::IntLit { suffix: None, .. } => true,
            ExprKind::Unary {
                op: UnOp::Neg | UnOp::BitNot,
                operand,
            } => self.is_unsized_literal(*operand),
            ExprKind::Binary { op, lhs, rhs } => {
                matches!(
                    op,
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
                ) && self.is_unsized_literal(*lhs)
                    && self.is_unsized_literal(*rhs)
            }
            ExprKind::If {
                then_expr,
                else_expr,
                ..
            } => self.is_unsized_literal(*then_expr) && self.is_unsized_literal(*else_expr),
            ExprKind::IntLit {
                suffix: Some(_), ..
            }
            | ExprKind::Unary { .. }
            | ExprKind::BoolLit(_)
            | ExprKind::StringLit(_)
            | ExprKind::Path(_)
            | ExprKind::Index { .. }
            | ExprKind::Range { .. }
            | ExprKind::PartSelect { .. }
            | ExprKind::Field { .. }
            | ExprKind::Call { .. }
            | ExprKind::Cast { .. }
            | ExprKind::Match { .. }
            | ExprKind::StructLit { .. }
            | ExprKind::ArrayLit(_)
            | ExprKind::TupleLit(_)
            | ExprKind::Concat(_)
            | ExprKind::Todo { .. }
            | ExprKind::Error => false,
        }
    }

    /// İfadenin doğal genişliği (aralığın üst ucu); aralık bilinmiyorsa
    /// kaba genişlik çıkarımı.
    pub(crate) fn natural_sig(&mut self, idx: Idx<Expr>) -> Option<Sig> {
        match self.int_range(idx) {
            Some(range) => Some(range.sig()),
            None => self.width_of(idx),
        }
    }

    /// İşlemin yapıldığı genişlik: aralığın alt ucu (esnek olmayan
    /// ifadede en geniş operand, eski kaba çıkarımla aynı).
    pub(crate) fn operation_sig(&mut self, idx: Idx<Expr>) -> Option<Sig> {
        match self.int_range(idx) {
            Some(range) => Some(Sig {
                width: range.lo,
                signed: range.signed,
            }),
            None => self.width_of(idx),
        }
    }

    fn int_range(&mut self, idx: Idx<Expr>) -> Option<Range> {
        let ast = self.ast;
        match &ast.exprs[idx].kind {
            ExprKind::Binary { op, lhs, rhs } => self.binary_range(*op, *lhs, *rhs),
            ExprKind::Unary {
                op: UnOp::Neg,
                operand,
            } => {
                // `-x` somut `i(N+1)`dir (esnek değil); işaretsiz operand
                // tip denetiminde E2002.
                let r = self.int_range(*operand)?;
                let width = (r.hi + 1).min(MAX_WIDTH);
                r.signed.then_some(Range {
                    signed: true,
                    lo: width,
                    hi: width,
                })
            }
            ExprKind::Unary {
                op: UnOp::BitNot,
                operand,
            } => self.int_range(*operand),
            ExprKind::If {
                then_expr,
                else_expr,
                ..
            } => {
                let (t, e) = (*then_expr, *else_expr);
                match (self.int_range(t), self.int_range(e)) {
                    (Some(a), Some(b)) => a.meet(b),
                    (Some(a), None) | (None, Some(a)) => Some(a),
                    (None, None) => None,
                }
            }
            ExprKind::Path(path) if path.segments.len() == 1 => {
                let name = &path.segments[0].text;
                let global = self.inline_notes.global_paths.contains(&idx);
                let lo = match self.local(name).filter(|_| !global) {
                    Some(l) => Some(l.lo),
                    None if !global => self.flex_lets.get(name).copied(),
                    None => None,
                };
                let sig = self.width_of(idx)?;
                Some(Range {
                    signed: sig.signed,
                    lo: lo.unwrap_or(sig.width).min(sig.width),
                    hi: sig.width,
                })
            }
            ExprKind::IntLit { .. }
            | ExprKind::BoolLit(_)
            | ExprKind::StringLit(_)
            | ExprKind::Path(_)
            | ExprKind::Unary { .. }
            | ExprKind::Index { .. }
            | ExprKind::Range { .. }
            | ExprKind::PartSelect { .. }
            | ExprKind::Field { .. }
            | ExprKind::Call { .. }
            | ExprKind::Cast { .. }
            | ExprKind::Match { .. }
            | ExprKind::StructLit { .. }
            | ExprKind::ArrayLit(_)
            | ExprKind::TupleLit(_)
            | ExprKind::Concat(_)
            | ExprKind::Todo { .. }
            | ExprKind::Error => self.width_of(idx).map(Range::exact),
        }
    }

    fn binary_range(&mut self, op: BinOp, lhs: Idx<Expr>, rhs: Idx<Expr>) -> Option<Range> {
        match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => {
                // Trit toplamı/çarpımı kendi kuralını taşır (ADR-0062).
                if let Some(sig) = self.trit_sum_sig(op, lhs, rhs) {
                    return Some(Range::exact(sig));
                }
                let common = self.operand_meet(lhs, rhs)?;
                let natural = match op {
                    BinOp::Add | BinOp::Sub => common.hi + 1,
                    BinOp::Mul => common.hi * 2,
                    BinOp::Div | BinOp::Rem => common.hi,
                    BinOp::BitAnd
                    | BinOp::BitOr
                    | BinOp::BitXor
                    | BinOp::Shl
                    | BinOp::Shr
                    | BinOp::Eq
                    | BinOp::Ne
                    | BinOp::Lt
                    | BinOp::Gt
                    | BinOp::Le
                    | BinOp::Ge
                    | BinOp::And
                    | BinOp::Or
                    | BinOp::Imp => unreachable!("dış kol yalnız aritmetik işleçleri geçirir"),
                };
                Some(Range {
                    hi: natural.min(MAX_WIDTH),
                    ..common
                })
            }
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => self.operand_meet(lhs, rhs),
            // Kaydırma: sol operandın tipi (§3.3, ADR-0036).
            BinOp::Shl | BinOp::Shr => self.int_range(lhs),
            // Karşılaştırma ve mantıksal işleçler: bool.
            BinOp::Eq
            | BinOp::Ne
            | BinOp::Lt
            | BinOp::Gt
            | BinOp::Le
            | BinOp::Ge
            | BinOp::And
            | BinOp::Or
            | BinOp::Imp => Some(Range::exact(Sig::BIT)),
        }
    }

    /// Operandların ortak aralığı; soneksiz literal (genişliği yok)
    /// karşı tarafa uyar.
    fn operand_meet(&mut self, lhs: Idx<Expr>, rhs: Idx<Expr>) -> Option<Range> {
        match (self.int_range(lhs), self.int_range(rhs)) {
            (Some(l), Some(r)) => l.meet(r),
            (Some(x), None) | (None, Some(x)) => Some(x),
            (None, None) => None,
        }
    }
}

/// Okunan genişlik: bağlam telden darsa yalnız alt bitler okunur.
fn demand(width: u32, ctx: Option<Sig>) -> u32 {
    match ctx {
        Some(c) if c.width < width => c.width,
        _ => width,
    }
}

/// Taşma biti hiç okunmayan esnek `let` bildirimi: Verilator -Wall
/// UNUSEDSIGNAL susturmasıyla sarılır (struct yaprağıyla aynı biçim).
pub(crate) fn silence_unused_carry(ind: &str, name: &str, line: &str) -> String {
    format!(
        "{ind}// carry bit of '{name}' unused in this module (ADR-0025)\n\
         {ind}// verilator lint_off UNUSEDSIGNAL\n\
         {line}\n\
         {ind}// verilator lint_on UNUSEDSIGNAL"
    )
}

impl Emitter<'_> {
    /// Modül `let`ine başvuru: esnekse okunan genişlik kaydedilir.
    pub(crate) fn note_let_use(&mut self, name: &str, ctx: Option<Sig>) {
        if !self.flex_lets.contains_key(name) || self.local(name).is_some() {
            return;
        }
        let Some(sig) = self.symbols.get(name).copied() else {
            return;
        };
        let read = demand(sig.width, ctx);
        let entry = self.flex_demand.entry(name.to_string()).or_insert(0);
        *entry = (*entry).max(read);
    }

    /// Süreç yereline başvuru: okunan genişlik kaydedilir.
    pub(crate) fn note_local_use(&mut self, name: &str, ctx: Option<Sig>) {
        let Some(i) = self.local_index(name) else {
            return;
        };
        let local = &mut self.proc.locals[i];
        local.read = local.read.max(demand(local.sig.width, ctx));
    }

    /// Gövde üretildikten sonra: taşma biti hiç okunmayan modül
    /// `let`lerinin bildirimleri susturulur.
    pub(crate) fn silence_unread_carries(&mut self, chunks: &mut [String]) {
        for (name, line) in std::mem::take(&mut self.flex_decls) {
            let Some(sig) = self.symbols.get(&name).copied() else {
                continue;
            };
            if self.flex_demand.get(&name).copied().unwrap_or(0) >= sig.width {
                continue;
            }
            let wrapped = silence_unused_carry("    ", &name, &line);
            if let Some(chunk) = chunks.iter_mut().find(|c| c.lines().any(|l| l == line)) {
                *chunk = chunk
                    .lines()
                    .map(|l| if l == line { wrapped.as_str() } else { l })
                    .collect::<Vec<_>>()
                    .join("\n");
            }
        }
    }
}
