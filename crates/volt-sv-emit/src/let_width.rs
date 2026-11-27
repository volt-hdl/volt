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
//!
//! Trit aritmetiği (`-t`, Trit'li çarpım, iki Trit'in toplamı ve farkı)
//! burada hesaplanmaz: genişliği ve işareti tip denetiminin tablosundan
//! okunur (#463, ADR-0145; tablo ADR-0141). Tablo cevap vermezse E2005;
//! üretici kendi hesabına dönmez.

use volt_ast::checked::CheckedValue;
use volt_ast::const_eval::MAX_WIDTH;
use volt_ast::{BinOp, Expr, ExprKind, Idx, UnOp};
use volt_diagnostics::{lstr, ErrorCode};

use crate::expr::{value_arms, Sig};
use crate::scope::Res;
use crate::Emitter;

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
    pub(crate) fn is_unsized_literal(&self, idx: Idx<Expr>) -> bool {
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
            // Kolları literal olan `if` ve match ifadesi (`value_arms`; kol
            // listesi boş olmaz: `if` iki dallıdır, ifade kolu olmayan match
            // üreticiye gelmeden E0001/E0014 alır).
            kind @ (ExprKind::If { .. } | ExprKind::Match { .. }) => value_arms(kind)
                .unwrap_or_default()
                .into_iter()
                .all(|e| self.is_unsized_literal(e)),
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

    /// Karşılaştırmanın yapıldığı genişlik (ADR-0106). Tip denetimi
    /// doğal genişlikleri farklı iki değişkeni reddettiği (E2031) için
    /// buraya gelen yanlar ya eşit doğal genişlikte ya da biri sabittir:
    /// - eşit doğal genişlik: o genişlik (`a + b == c + d` → 9 bit);
    /// - geniş yan sabit ve dar yana sığıyor: dar yanın genişliği (sayaç
    ///   sınırı `r == LIMIT - 1`, önceki çıktıyla aynı);
    /// - dar yan sabit ya da literal: geniş yanın doğal genişliği
    ///   (`a + b == 5` → `9'd5`, taşma biti atılmaz).
    pub(crate) fn comparison_ctx(&mut self, lhs: Idx<Expr>, rhs: Idx<Expr>) -> Option<Sig> {
        match (self.int_range(lhs), self.int_range(rhs)) {
            (Some(l), Some(r)) if l.hi == r.hi => Some(l.sig()),
            (Some(l), Some(r)) => {
                let (wide, wide_range, narrow_range) = if l.hi > r.hi {
                    (lhs, l, r)
                } else {
                    (rhs, r, l)
                };
                if self.eval_const(wide).is_some() {
                    Some(narrow_range.sig())
                } else {
                    Some(wide_range.sig())
                }
            }
            (Some(x), None) | (None, Some(x)) => Some(x.sig()),
            (None, None) => None,
        }
    }

    fn int_range(&mut self, idx: Idx<Expr>) -> Option<Range> {
        if self.is_trit_arith(idx) {
            return self.checked_trit_range(idx);
        }
        let ast = self.ast;
        match &ast.exprs[idx].kind {
            ExprKind::Binary { op, lhs, rhs } => self.binary_range(*op, *lhs, *rhs),
            ExprKind::Unary {
                op: UnOp::Neg,
                operand,
            } => {
                // `-x` somut `i(N+1)`dir (esnek değil); işaretsiz operand
                // tip denetiminde E2002. `-t` (t : Trit) yukarıda, tablodan.
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
            // `if` ve match ifadesi aynı kural (type-inference.md §3.7,
            // §3.9, `value_arms`): literal kol aralık vermez, somut kolların
            // aralıkları kesişir (tip denetiminin `synth_if`/`synth_match`'i).
            kind @ (ExprKind::If { .. } | ExprKind::Match { .. }) => {
                let mut ranges = value_arms(kind)
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|e| self.int_range(e))
                    .collect::<Vec<_>>()
                    .into_iter();
                let first = ranges.next()?;
                ranges.try_fold(first, Range::meet)
            }
            // Esnek aralığın alt ucu yalnız tipsiz yerelde ve tipsiz modül
            // `let`'inde vardır; ad kapsam zinciriyle çözülür (scope.rs).
            ExprKind::Path(_) => {
                let lo = match self.resolve(idx) {
                    Res::Local(i) => Some(self.proc.locals[i].lo),
                    Res::Module => self
                        .module_path(idx)
                        .and_then(|n| self.flex_lets.get(n).copied()),
                    Res::Loop(_) | Res::Const { .. } | Res::Unbound => None,
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
            | ExprKind::Unary { .. }
            | ExprKind::Index { .. }
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
            | ExprKind::Error => self.width_of(idx).map(Range::exact),
        }
    }

    fn binary_range(&mut self, op: BinOp, lhs: Idx<Expr>, rhs: Idx<Expr>) -> Option<Range> {
        match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => {
                // Trit toplamı, farkı ve çarpımı buraya gelmez (`int_range`,
                // tablodan).
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

    /// Trit aritmetiği mi (type-inference.md §3.3, §3.4): Trit'in eksisi
    /// (`-t`) ya da en az bir yanı Trit olan çarpım, toplam ve fark (`t * u`,
    /// `t * x`, `x * t`, `t ± u`). Trit ile Trit olmayan bir değişkenin
    /// toplamı tip denetiminde hatadır; literal Trit'in tipini alır
    /// (`t + 1`, §4). Trit'i tanıyan, çarpımın seçici olarak basılıp
    /// basılmayacağına da karar veren `is_trit`'tir (trit.rs).
    fn is_trit_arith(&self, idx: Idx<Expr>) -> bool {
        match &self.ast.exprs[idx].kind {
            ExprKind::Unary {
                op: UnOp::Neg,
                operand,
            } => self.is_trit(*operand),
            ExprKind::Binary {
                op: BinOp::Add | BinOp::Sub | BinOp::Mul,
                lhs,
                rhs,
            } => self.is_trit(*lhs) || self.is_trit(*rhs),
            ExprKind::Unary { .. }
            | ExprKind::Binary { .. }
            | ExprKind::IntLit { .. }
            | ExprKind::BoolLit(_)
            | ExprKind::StringLit(_)
            | ExprKind::Path(_)
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
            | ExprKind::Error => false,
        }
    }

    /// Trit aritmetiğinin aralığı: tip denetiminin ifadeye verdiği
    /// genişlik ve işaret (#463, ADR-0145). Somut tipte aralık tek
    /// genişliktir; esnek işlenenli çarpımda (`t * (a + b)`) tablo doğal
    /// genişliği taşır, alt uç da odur (ADR-0144 §2: aralığı Trit'te
    /// tanımlı değil). Cevap yoksa E2005 ve `None`.
    ///
    /// `Trit × iN`'de okunan, iN yanının değeridir (§3.3: çarpım iN'dir).
    /// Çarpımın kendi kaydı değil: çarpım itilen bir aritmetiğin
    /// işleneniyse tip denetimi ona hedef tipi kaydeder (ADR-0041,
    /// `widen_operands`); çarpım ise iN'de hesaplanır ve sarar (ADR-0145
    /// Karar 8).
    fn checked_trit_range(&mut self, idx: Idx<Expr>) -> Option<Range> {
        let read = self.trit_product_int_side(idx).unwrap_or(idx);
        match self.checked_value(read) {
            Some(CheckedValue::Bits { width, signed }) => Some(Range::exact(Sig { width, signed })),
            Some(CheckedValue::Struct(_) | CheckedValue::NoWidth) | None => {
                self.error(
                    ErrorCode::E2005,
                    lstr!(
                        en: "cannot determine the width of this Trit operation";
                        tr: "bu Trit işleminin genişliği belirlenemiyor"
                    ),
                    self.ast.exprs[idx].span,
                    &lstr!(
                        en: "give the operands a type with a width, for example x as i8";
                        tr: "işlenenlere genişliği olan bir tip verin, örneğin x as i8"
                    ),
                );
                None
            }
        }
    }

    /// Tip denetiminin ifadeye verdiği değer (ADR-0141). fn açılımının
    /// kopyası (ADR-0081) gövdedeki özgün düğümün değerini okur: fn generic
    /// değildir, gövde parametrelerin bildirilen tipleriyle bir kez denetlenir.
    pub(crate) fn checked_value(&self, idx: Idx<Expr>) -> Option<CheckedValue> {
        let table = self.checked?;
        let span = self.ast.exprs[idx].span;
        table.expr(idx, span).or_else(|| {
            // Köken gövdedeki özgün düğümdür; kopyada aynı indekste durur.
            // Konum kökeninkidir: parametrenin yerine geçen kopya argümanın
            // konumunu taşır (ADR-0081).
            let origin = *self.inline_notes.origins.get(&idx)?;
            table.expr(origin, self.ast.exprs[origin].span)
        })
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

    /// Süreç yereline (`proc.locals[i]`) başvuru: okunan genişlik
    /// kaydedilir.
    pub(crate) fn note_local_use(&mut self, i: usize, ctx: Option<Sig>) {
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
