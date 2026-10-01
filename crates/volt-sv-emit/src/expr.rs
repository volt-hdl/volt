//! İfade üretimi ve kaba genişlik çıkarımı (sv-mapping.md §5, §6, §10).
//!
//! F0 kuralı: ifade genişliği = operandların en genişi. Gerçek tip
//! çıkarımı (taşma genişlemesi dahil) F2'de HIR'a taşınacak.
//!
//! ADR-0041: genişleme AÇIK basılır. Aritmetik operand, hedef bağlamdan
//! dar ve aynı işaretli bir atom (sinyal, dizi elemanı, örnek çıkışı)
//! ise SystemVerilog boyut dönüşümüyle sarılır: `32'(taps_r[0])`.
//! Literaller doğrudan bağlam genişliğiyle boyutlanır. Böylece
//! Verilator `-Wall` WIDTHEXPAND uyarısı üretmez ve genişleme SV
//! bağlam kurallarına değil metne yazılır.

use volt_ast::{BinOp, Expr, ExprKind, Idx, MatchArmBody, NumBase, TypeRefKind, UnOp};
use volt_diagnostics::{lstr, ErrorCode};

use crate::Emitter;

/// Sinyal genişliği ve işareti.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sig {
    pub width: u32,
    pub signed: bool,
}

impl Sig {
    /// Tek bitlik işaretsiz sinyal (reset, senkronizör aşaması).
    pub(crate) const BIT: Sig = Sig {
        width: 1,
        signed: false,
    };

    /// `logic [7:0]` / `logic signed [15:0]` / `logic` (§2).
    pub fn decl_type(&self) -> String {
        match (self.width, self.signed) {
            (1, false) => "logic".to_string(),
            (w, false) => format!("logic [{}:0]", w - 1),
            (w, true) => format!("logic signed [{}:0]", w - 1),
        }
    }

    /// `wire ` bildirimindeki aralık öneki: `[8:0] ` veya boş.
    pub fn wire_prefix(&self) -> String {
        match (self.width, self.signed) {
            (1, false) => String::new(),
            (w, false) => format!("[{}:0] ", w - 1),
            (w, true) => format!("signed [{}:0] ", w - 1),
        }
    }
}

/// Bildirilen imza bağlamdan dar ve aynı işaretliyse bağlam kazanır
/// (literal doğrudan hedef genişliğiyle boyutlanır — ADR-0041).
pub(crate) fn widen_sig(declared: Option<Sig>, ctx: Option<Sig>) -> Option<Sig> {
    match (declared, ctx) {
        (Some(d), Some(c)) if c.signed == d.signed && c.width > d.width => Some(c),
        (d, c) => d.or(c),
    }
}

/// SV çıktısında parantez kararı için öncelik — HEDEF dilin tablosu
/// (IEEE 1800-2017 §11.3.2, Tablo 11-2), Volt'unki DEĞİL (ADR-0057).
/// İki tablo tek yerde ayrışır: Volt'ta `&` `^` `|` karşılaştırmadan
/// sıkı bağlanır (ADR-0013 §2.2), SV'de C mirası olarak gevşek. Volt
/// sırası kullanılırsa `(a & b) == 0` parantezsiz basılır ve SV bunu
/// `a & (b == 0)` okur — sessiz yanlış derleme.
pub(crate) fn sv_prec(op: BinOp) -> u8 {
    match op {
        // İmplikasyon SV'de `!a || b` olarak açıldığından || düzeyinde.
        BinOp::Imp => 1,
        BinOp::Or => 1,
        BinOp::And => 2,
        BinOp::BitOr => 3,
        BinOp::BitXor => 4,
        BinOp::BitAnd => 5,
        BinOp::Eq | BinOp::Ne => 6,
        BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => 7,
        BinOp::Shl | BinOp::Shr => 8,
        BinOp::Add | BinOp::Sub => 9,
        BinOp::Mul | BinOp::Div | BinOp::Rem => 10,
    }
}

fn is_bitwise(op: BinOp) -> bool {
    matches!(op, BinOp::BitOr | BinOp::BitXor | BinOp::BitAnd)
}

fn is_comparison(op: BinOp) -> bool {
    matches!(
        op,
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge
    )
}

pub(crate) const PREC_TERNARY: u8 = 0;
pub(crate) const PREC_UNARY: u8 = 11;
pub(crate) const PREC_ATOM: u8 = 12;

/// Negatif literal (`-16'sd2`) operand konumunda parantezlenir.
fn lit_prec(text: &str) -> u8 {
    if text.starts_with('-') {
        PREC_TERNARY
    } else {
        PREC_ATOM
    }
}

/// `9'(a)` biçimi: ondalık genişlik + `'(` (boyut dönüşümü).
fn starts_with_size_cast(text: &str) -> bool {
    let digits = text.bytes().take_while(u8::is_ascii_digit).count();
    digits > 0 && text[digits..].starts_with("'(")
}

fn is_arith(op: BinOp) -> bool {
    matches!(
        op,
        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem
    )
}

fn is_comparison_or_logical(op: BinOp) -> bool {
    matches!(
        op,
        BinOp::Eq
            | BinOp::Ne
            | BinOp::Lt
            | BinOp::Gt
            | BinOp::Le
            | BinOp::Ge
            | BinOp::And
            | BinOp::Or
            | BinOp::Imp
    )
}

impl<'a> Emitter<'a> {
    // ═══ Sabit değerlendirme (bits<N>, aralık genişliği) ══════════

    /// Derleme zamanı tam sayı değeri (i128 — negatif sabitler dahil).
    pub(crate) fn eval_const(&self, idx: Idx<Expr>) -> Option<i128> {
        self.eval_const_depth(idx, 0)
    }

    /// En içteki (gölgeleyen) döngü değişkeninin değeri.
    pub(crate) fn loop_var(&self, name: &str) -> Option<i128> {
        self.loop_vars
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, v)| *v)
    }

    /// Derinlik sınırı, döngüsel const zincirinde (HIR tanısı üretilmiş
    /// olsa da) yığın taşmasını önler.
    fn eval_const_depth(&self, idx: Idx<Expr>, depth: u32) -> Option<i128> {
        const MAX_CONST_DEPTH: u32 = 64;
        if depth > MAX_CONST_DEPTH {
            return None;
        }
        match &self.ast.exprs[idx].kind {
            ExprKind::IntLit { value, .. } => i128::try_from(*value).ok(),
            ExprKind::Unary {
                op: UnOp::Neg,
                operand,
            } => self.eval_const_depth(*operand, depth + 1)?.checked_neg(),
            // Mantıksal değer 0/1 (bool tipli const, `const N = if … `).
            ExprKind::BoolLit(_) | ExprKind::Unary { op: UnOp::Not, .. } => {
                self.eval_const_bool(idx, depth).map(i128::from)
            }
            ExprKind::Binary { op, .. } if is_comparison_or_logical(*op) => {
                self.eval_const_bool(idx, depth).map(i128::from)
            }
            // `const N = if C { a } else { b }` (ADR-0083 Gelecek iş 6):
            // HIR `consteval` ile aynı seçim; koşul çözülemezse `None`.
            ExprKind::If {
                cond,
                then_expr,
                else_expr,
            } => {
                let taken = if self.eval_const_bool(*cond, depth + 1)? {
                    *then_expr
                } else {
                    *else_expr
                };
                self.eval_const_depth(taken, depth + 1)
            }
            ExprKind::Binary { op, lhs, rhs } => {
                let l = self.eval_const_depth(*lhs, depth + 1)?;
                let r = self.eval_const_depth(*rhs, depth + 1)?;
                match op {
                    BinOp::Add => l.checked_add(r),
                    BinOp::Sub => l.checked_sub(r),
                    BinOp::Mul => l.checked_mul(r),
                    BinOp::Div => l.checked_div(r),
                    BinOp::Rem => l.checked_rem(r),
                    // Bit işlemleri yalnız negatif olmayan değerde (genişlik
                    // bağlamsız ikiye tümleyen belirsiz); `>>` mantıksal.
                    _ if l < 0 || r < 0 => None,
                    BinOp::BitAnd => Some(l & r),
                    BinOp::BitOr => Some(l | r),
                    BinOp::BitXor => Some(l ^ r),
                    BinOp::Shl => l
                        .checked_shl(u32::try_from(r).ok()?)
                        .filter(|v| v >> r == l),
                    BinOp::Shr => Some(if r >= 127 { 0 } else { l >> r }),
                    // Karşılaştırma/mantıksal işleçler sayı üretmez (eval_const_bool).
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
            // Modül sinyalleri gölgeler; döngü değişkeni const'tan önce —
            // açılmış fn gövdesinin const'u hariç (ADR-0081 hijyen).
            ExprKind::Path(p) if p.segments.len() == 1 => {
                let name = &p.segments[0].text;
                let global = self.inline_notes.global_paths.contains(&idx);
                if !global && self.symbols.contains_key(name) {
                    return None;
                }
                if let Some(v) = self.loop_var(name).filter(|_| !global) {
                    return Some(v);
                }
                let &(_, value) = self.consts.get(name)?;
                self.eval_const_depth(value, depth + 1)
            }
            // Sabit dizi elemanı: `COEFFS[k]`, k sabit (ADR-0041).
            ExprKind::Index { base, index } => {
                let name = crate::path_single(self.ast, *base)?;
                let i = self.eval_const_depth(*index, depth + 1)?;
                self.const_array_element(name, i)
            }
            // `const N : u32 = match K { … }` (ADR-0083 Karar 5): ilk
            // eşleşen kolun değeri; muhafızlı ya da çözülemeyen desen `None`.
            ExprKind::Match { scrutinee, arms } => {
                let key = self.const_match_key(*scrutinee, depth + 1)?;
                for arm in arms {
                    if arm.guard.is_some() {
                        return None;
                    }
                    if self.const_pattern_matches(arm.pattern, key, depth + 1)? {
                        let volt_ast::MatchArmBody::Expr(body) = arm.body else {
                            return None;
                        };
                        return self.eval_const_depth(body, depth + 1);
                    }
                }
                None
            }
            ExprKind::StringLit(_)
            | ExprKind::Path(_)
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

    /// Derleme zamanı mantıksal değer: literal, karşılaştırma, `&&`/`||`/
    /// `->`, `!`, bool const'u. Başka bir şey 0/1 değerlendiriyorsa o.
    fn eval_const_bool(&self, idx: Idx<Expr>, depth: u32) -> Option<bool> {
        const MAX_CONST_DEPTH: u32 = 64;
        if depth > MAX_CONST_DEPTH {
            return None;
        }
        match &self.ast.exprs[idx].kind {
            ExprKind::BoolLit(b) => Some(*b),
            ExprKind::Unary {
                op: UnOp::Not,
                operand,
            } => Some(!self.eval_const_bool(*operand, depth + 1)?),
            ExprKind::Binary { op, lhs, rhs } if is_comparison_or_logical(*op) => {
                if matches!(op, BinOp::And | BinOp::Or | BinOp::Imp) {
                    let l = self.eval_const_bool(*lhs, depth + 1)?;
                    let r = self.eval_const_bool(*rhs, depth + 1)?;
                    return Some(match op {
                        BinOp::And => l && r,
                        BinOp::Or => l || r,
                        BinOp::Imp => !l || r,
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
                        | BinOp::Ge => {
                            unreachable!("üstteki matches! yalnız &&, || ve -> işleçlerini geçirir")
                        }
                    });
                }
                let l = self.eval_const_depth(*lhs, depth + 1)?;
                let r = self.eval_const_depth(*rhs, depth + 1)?;
                Some(match op {
                    BinOp::Eq => l == r,
                    BinOp::Ne => l != r,
                    BinOp::Lt => l < r,
                    BinOp::Gt => l > r,
                    BinOp::Le => l <= r,
                    BinOp::Ge => l >= r,
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
                    | BinOp::Imp => unreachable!(
                        "muhafız yalnız karşılaştırma/mantıksal işleç geçirir; mantıksallar yukarıda döndü"
                    ),
                })
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
            | ExprKind::Error => match self.eval_const_depth(idx, depth + 1)? {
                0 => Some(false),
                1 => Some(true),
                _ => None,
            },
        }
    }

    /// Sabit match'in sınananı: tamsayı sabiti ya da enum varyantının
    /// kodu (varyant yolu veya enum tipli `const`).
    fn const_match_key(&self, idx: Idx<Expr>, depth: u32) -> Option<i128> {
        if let ExprKind::BoolLit(b) = self.ast.exprs[idx].kind {
            return Some(i128::from(b));
        }
        if let ExprKind::Path(p) = &self.ast.exprs[idx].kind {
            if let Some((decl, i)) = self.enum_variant_of_path(p) {
                return i128::try_from(*self.enum_layout(decl)?.values.get(i)?).ok();
            }
            if let [seg] = p.segments.as_slice() {
                if !self.symbols.contains_key(&seg.text) {
                    if let Some(&(ty, value)) = self.consts.get(&seg.text) {
                        if self.enum_of_type(ty).is_some() {
                            return self.const_match_key(value, depth + 1);
                        }
                    }
                }
            }
        }
        self.eval_const_depth(idx, depth)
    }

    /// Desen sabit değerle eşleşiyor mu? (`None`: değerlendirilemez.)
    fn const_pattern_matches(
        &self,
        pat: Idx<volt_ast::Pattern>,
        key: i128,
        depth: u32,
    ) -> Option<bool> {
        match &self.ast.patterns[pat].kind {
            volt_ast::PatternKind::Wildcard => Some(true),
            volt_ast::PatternKind::Or(alts) => {
                for &a in alts {
                    if self.const_pattern_matches(a, key, depth)? {
                        return Some(true);
                    }
                }
                Some(false)
            }
            volt_ast::PatternKind::Literal(e) => Some(self.const_match_key(*e, depth)? == key),
            volt_ast::PatternKind::Path {
                path: p,
                args: None,
            } => {
                let (decl, i) = self.enum_variant_of_path(p)?;
                Some(i128::try_from(*self.enum_layout(decl)?.values.get(i)?).ok()? == key)
            }
            volt_ast::PatternKind::Path { .. }
            | volt_ast::PatternKind::Tuple(_)
            | volt_ast::PatternKind::Error => None,
        }
    }

    // ═══ Genişlik çıkarımı (kaba) ═════════════════════════════════

    pub(crate) fn width_of(&mut self, idx: Idx<Expr>) -> Option<Sig> {
        let ast = self.ast;
        match &ast.exprs[idx].kind {
            ExprKind::IntLit { suffix, .. } => suffix.map(suffix_sig),
            ExprKind::BoolLit(_) => Some(Sig {
                width: 1,
                signed: false,
            }),
            ExprKind::Path(path) if path.segments.len() >= 2 => self.enum_variant_sig(path),
            ExprKind::Path(path) => {
                let name = path.segments.first()?;
                let global = self.inline_notes.global_paths.contains(&idx);
                // Blok `let`'i (süreç yereli) modül adlarını gölgeler.
                if let Some(l) = self.local(&name.text).filter(|_| !global) {
                    return Some(l.sig);
                }
                if let Some(sig) = self.symbols.get(&name.text).copied().filter(|_| !global) {
                    return Some(sig);
                }
                // Döngü değişkeni bağlamla boyutlanan literal gibidir.
                if !global && self.loop_var(&name.text).is_some() {
                    return None;
                }
                // Üst düzey const: genişlik bildirilen tipinden gelir.
                let &(ty, _) = self.consts.get(&name.text)?;
                if matches!(
                    ast.types[crate::alias::resolve(ast, ty)].kind,
                    TypeRefKind::Array { .. }
                ) {
                    return None; // dizi sabiti: yalnız indeksle kullanılır
                }
                let span = ast.exprs[idx].span;
                self.sig_of_typeref(ty, span)
            }
            ExprKind::Binary { op, lhs, rhs } => {
                if is_comparison_or_logical(*op) {
                    return Some(Sig {
                        width: 1,
                        signed: false,
                    });
                }
                // Kaydırma sonucu SOL operandın genişlik ve işaretini taşır
                // (ADR-0036) — miktar operandı sonucu etkilemez. İşaret bilgisi
                // doğru olmalı ki `(x as i32) >> n` üstündeki `as u32` cast'i
                // $unsigned sınırını üretsin (SV bağlam sızıntısına karşı).
                if matches!(op, BinOp::Shl | BinOp::Shr) {
                    let lhs = *lhs;
                    return self.width_of(lhs);
                }
                let (lhs, rhs) = (*lhs, *rhs);
                if let Some(sig) = self.trit_sum_sig(*op, lhs, rhs) {
                    return Some(sig);
                }
                match (self.width_of(lhs), self.width_of(rhs)) {
                    (Some(l), Some(r)) => Some(Sig {
                        width: l.width.max(r.width),
                        signed: l.signed && r.signed,
                    }),
                    (Some(s), None) | (None, Some(s)) => Some(s),
                    (None, None) => None,
                }
            }
            ExprKind::Unary { op: UnOp::Not, .. } => Some(Sig {
                width: 1,
                signed: false,
            }),
            ExprKind::Unary { operand, .. } => {
                let operand = *operand;
                self.width_of(operand)
            }
            // Dizi tabanında indeks ELEMANI seçer (ADR-0035); dizi sabiti
            // de eleman imzasını taşır (ADR-0041); bit seçimi 1 bittir.
            ExprKind::Index { base, .. } => {
                let base = *base;
                if let Some(name) = crate::path_single(self.ast, base) {
                    if self.array_dims.contains_key(name) {
                        return self.symbols.get(name).copied();
                    }
                    // Paketlenmiş port/wire dizisi: eleman imzası (ADR-0056).
                    if let Some(&(elem, _)) = self.packed_arrays.get(name) {
                        return Some(elem);
                    }
                    let name = name.to_string();
                    if let Some((sig, _, _)) = self.const_array_info(&name) {
                        return Some(sig);
                    }
                }
                Some(Sig {
                    width: 1,
                    signed: false,
                })
            }
            ExprKind::Range { hi, lo, .. } => {
                let (hi, lo) = (self.eval_const(*hi)?, self.eval_const(*lo)?);
                Some(Sig {
                    width: (hi.saturating_sub(lo) + 1) as u32,
                    signed: false,
                })
            }
            ExprKind::PartSelect { width, .. } => {
                let w = self.eval_const(*width)?;
                Some(Sig {
                    width: w as u32,
                    signed: false,
                })
            }
            ExprKind::Cast { ty, .. } => {
                let span = ast.exprs[idx].span;
                let ty = *ty;
                self.sig_of_typeref(ty, span)
            }
            ExprKind::If {
                then_expr,
                else_expr,
                ..
            } => {
                let (t, e) = (*then_expr, *else_expr);
                self.width_of(t).or_else(|| self.width_of(e))
            }
            // Yerleşik primitif ya da kullanıcı modülü örneği portu.
            ExprKind::Field { base, field } => {
                let (base, field) = (*base, field.text.clone());
                self.builtin_field_sig(base, &field)
                    .or_else(|| self.user_field_sig(base, &field))
            }
            // prev(x[, N]) (ADR-0040): argümanın genişliği.
            ExprKind::Call { callee, args } if self.is_prev_call(*callee) => {
                let x = args.first().copied()?;
                self.width_of(x)
            }
            ExprKind::Call { .. } | ExprKind::Error => None,
            // Struct indirgemesinin birleştirmesi (ADR-0077): yaprak
            // genişliklerinin toplamı, işaretsiz.
            ExprKind::Concat(parts) => {
                let tys: Vec<_> = parts.iter().map(|&(_, t)| t).collect();
                let mut width = 0;
                for t in tys {
                    width += self.leaf_sig(t, ast.exprs[idx].span)?.width;
                }
                Some(Sig {
                    width,
                    signed: false,
                })
            }
            // Match ifadesi `if` gibi: ilk genişliği bilinen kol (ADR-0083).
            ExprKind::Match { arms, .. } => arms.iter().find_map(|a| match a.body {
                MatchArmBody::Expr(e) => self.width_of(e),
                MatchArmBody::Block(_) => None,
            }),
            // F1 parser yapıları — SV üretimi sonraki aşamalarda
            ExprKind::StringLit(_)
            | ExprKind::StructLit { .. }
            | ExprKind::ArrayLit(_)
            | ExprKind::TupleLit(_)
            | ExprKind::Todo { .. } => None,
        }
    }

    /// Yerleşik primitif alan erişiminin genişliği: taban tek segmentli
    /// bir örnek adıysa port tablosundan okunur (ADR-0027/0029).
    pub(crate) fn builtin_field_sig(&self, base: Idx<Expr>, field: &str) -> Option<Sig> {
        use volt_ast::builtin::PortKind;
        let inst = crate::path_single(self.ast, base)?;
        let info = self.builtin_insts.get(inst)?;
        let port = info.prim.port(field)?;
        Some(match port.kind {
            PortKind::Data => info.data,
            PortKind::Bool | PortKind::Clock => Sig {
                width: 1,
                signed: false,
            },
            PortKind::Addr => Sig {
                width: info.dim.trailing_zeros(),
                signed: false,
            },
            PortKind::Dim => Sig {
                width: info.dim as u32,
                signed: false,
            },
            PortKind::Taps => Sig {
                width: info.dim as u32 * info.data.width,
                signed: false,
            },
        })
    }

    // ═══ Açık genişleme (ADR-0041) ════════════════════════════════

    /// Genişletme sarmalanabilir atom mu: modül sinyali, dizi elemanı,
    /// örnek çıkışı, `prev()`. Literal ve sabitler bağlamla boyutlanır,
    /// cast kendi genişliğini taşır, bileşik ifadeye bağlam içeri akar.
    fn is_widen_atom(&self, idx: Idx<Expr>) -> bool {
        match &self.ast.exprs[idx].kind {
            ExprKind::Path(p) => p
                .segments
                .first()
                .is_some_and(|n| self.symbols.contains_key(&n.text)),
            // Dizi sabiti elemanı literale katlanır — sarılmaz.
            ExprKind::Index { base, .. } => {
                !crate::path_single(self.ast, *base).is_some_and(|n| self.is_const_array(n))
            }
            ExprKind::Field { .. } => true,
            ExprKind::Call { callee, .. } => self.is_prev_call(*callee),
            ExprKind::IntLit { .. }
            | ExprKind::BoolLit(_)
            | ExprKind::StringLit(_)
            | ExprKind::Binary { .. }
            | ExprKind::Unary { .. }
            | ExprKind::Range { .. }
            | ExprKind::PartSelect { .. }
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

    /// `text` (idx'in çıktısı) hedef bağlamdan dar ve aynı işaretli bir
    /// atomsa `W'(text)` boyut dönüşümüyle sarılır. `whole_shift`:
    /// atama konumunda kaydırma sonucu da bütün olarak sarılır (sol
    /// operanda bağlam itilmez — ADR-0036 işaret kuralı korunur).
    fn widen_if_narrow(
        &mut self,
        idx: Idx<Expr>,
        text: String,
        ctx: Option<Sig>,
        whole_shift: bool,
    ) -> String {
        let Some(target) = ctx else { return text };
        let is_shift = matches!(
            &self.ast.exprs[idx].kind,
            ExprKind::Binary {
                op: BinOp::Shl | BinOp::Shr,
                ..
            }
        );
        let atom = self.is_widen_atom(idx) || self.is_local_path(idx);
        if !(atom || (whole_shift && is_shift)) {
            return text;
        }
        match self.width_of(idx) {
            Some(src) if src.width < target.width && src.signed == target.signed => {
                format!("{}'({text})", target.width)
            }
            // Doğal genişlikteki tipsiz `let` dar hedefe: taşma biti
            // bilerek atılır (ADR-0025), kesme açık yazılır.
            Some(src) if atom && src.width > target.width && src.signed == target.signed => {
                format!("{}'({text})", target.width)
            }
            _ => text,
        }
    }

    /// Bit düzeyi operand bağlamdan farklı genişlikteyse (tipsiz `let`in
    /// esnek aralığı, ADR-0025) boyut dönüşümüyle uydurulur.
    fn fit_bitwise_operand(
        &mut self,
        op: BinOp,
        operand: Idx<Expr>,
        text: String,
        ctx: Option<Sig>,
    ) -> String {
        if is_bitwise(op) {
            self.widen_if_narrow(operand, text, ctx, false)
        } else {
            text
        }
    }

    /// Süreç yereline (blok `let`i) başvuru mu?
    fn is_local_path(&self, idx: Idx<Expr>) -> bool {
        crate::path_single(self.ast, idx).is_some_and(|n| {
            self.local(n).is_some() && !self.inline_notes.global_paths.contains(&idx)
        })
    }

    /// Atama konumu (assign / let / `<=` / `=` / port bağlama): RHS
    /// hedef imzasıyla basılır, dar atom ya da kaydırma sonucu sarılır.
    pub(crate) fn emit_assigned(&mut self, idx: Idx<Expr>, target: Option<Sig>) -> String {
        let text = self.emit_expr(idx, target);
        self.widen_if_narrow(idx, text, target, true)
    }

    /// Aritmetik/tekli/ternary operandı: bağlamla basılır, dar atom sarılır.
    pub(crate) fn emit_operand(
        &mut self,
        idx: Idx<Expr>,
        ctx: Option<Sig>,
        parent_prec: u8,
        is_right: bool,
    ) -> String {
        let text = self.emit_prec(idx, ctx, parent_prec, is_right);
        self.widen_if_narrow(idx, text, ctx, false)
    }

    // ═══ İfade üretimi ════════════════════════════════════════════

    /// Üst düzey ifade — dış parantez yok.
    pub(crate) fn emit_expr(&mut self, idx: Idx<Expr>, ctx: Option<Sig>) -> String {
        self.emit_prec(idx, ctx, PREC_TERNARY, false)
    }

    /// İndeks/aralık/kaydırma miktarı gibi konumlar: literal ve sabit
    /// ifadeler çıplak yazılır (`taps_r[i - 1]` → `taps_r[3]`).
    pub(crate) fn emit_plain(&mut self, idx: Idx<Expr>) -> String {
        match &self.ast.exprs[idx].kind {
            ExprKind::IntLit { value, .. } => value.to_string(),
            ExprKind::Path(p) if p.segments.len() == 1 => {
                match self.loop_var(&p.segments[0].text) {
                    Some(v) => v.to_string(),
                    None => self.emit_prec(idx, None, PREC_ATOM, false),
                }
            }
            ExprKind::BoolLit(_)
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
            | ExprKind::Error => match self.eval_const(idx) {
                Some(v) => v.to_string(),
                None => self.emit_prec(idx, None, PREC_ATOM, false),
            },
        }
    }

    pub(crate) fn emit_prec(
        &mut self,
        idx: Idx<Expr>,
        ctx: Option<Sig>,
        parent_prec: u8,
        is_right_operand: bool,
    ) -> String {
        let ast = self.ast;
        let span = ast.exprs[idx].span;
        // ADR-0081 ikame kipi: tipsiz fn `let`i tel kipindeki telinin
        // genişliğinde hesaplanır.
        if self.inline_notes.self_sized.contains(&idx) && self.self_sizing.insert(idx) {
            let sig = self.natural_sig(idx);
            let inner = self.emit_prec(idx, sig, PREC_TERNARY, false);
            self.self_sizing.remove(&idx);
            return match sig {
                Some(sig) => format!("{}'({inner})", sig.width),
                None => inner,
            };
        }
        let (text, my_prec) = match &ast.exprs[idx].kind {
            ExprKind::IntLit {
                value,
                suffix,
                base,
            } => {
                let sig = widen_sig(suffix.map(suffix_sig), ctx);
                let value = i128::try_from(*value).unwrap_or(i128::MAX);
                let text = self.fmt_int(value, *base, sig, span);
                let prec = lit_prec(&text);
                (text, prec)
            }
            ExprKind::BoolLit(true) => ("1'b1".to_string(), PREC_ATOM),
            ExprKind::BoolLit(false) => ("1'b0".to_string(), PREC_ATOM),
            ExprKind::Path(path) if self.enum_variant_of_path(path).is_some() => {
                let name = self.emit_enum_variant(path).unwrap_or_default();
                (name, PREC_ATOM)
            }
            // Blok `let`'i: sürecin yeniden adlandırılmış yereli (ADR-0083).
            ExprKind::Path(path)
                if path.segments.len() == 1
                    && !self.inline_notes.global_paths.contains(&idx)
                    && self.local(&path.segments[0].text).is_some() =>
            {
                self.note_local_use(&path.segments[0].text, ctx);
                let sv = self.local(&path.segments[0].text).map(|l| l.sv.clone());
                (sv.unwrap_or_default(), PREC_ATOM)
            }
            ExprKind::Path(path) => {
                // Üst düzey const referansı boyutlandırılmış literale
                // katlanır — üretilen RTL'de tanımsız isim kalmaz. Açılmış
                // fn gövdesindeki const çağıranın aynı adlı sinyaline
                // bağlanmaz (ADR-0081 hijyen).
                let global = self.inline_notes.global_paths.contains(&idx);
                match self.fold_const_path(path, ctx, span, global) {
                    Some(folded) => {
                        let prec = lit_prec(&folded);
                        (folded, prec)
                    }
                    None => {
                        if let [seg] = path.segments.as_slice() {
                            if !global {
                                self.note_let_use(&seg.text, ctx);
                            }
                        }
                        let name = path
                            .segments
                            .iter()
                            .map(|n| n.text.clone())
                            .collect::<Vec<_>>()
                            .join("::");
                        (name, PREC_ATOM)
                    }
                }
            }
            ExprKind::Unary { op, operand } => {
                let operand = *operand;
                let sym = op.symbol();
                let inner = if matches!(op, UnOp::Not) {
                    self.emit_prec(operand, None, PREC_UNARY, false)
                } else {
                    self.emit_operand(operand, ctx, PREC_UNARY, false)
                };
                // Yosys (0.66) tekli işleçten hemen sonraki boyut
                // dönüşümünü (`-9'(a)`, `~9'(a)`) "Static cast with zero or
                // negative size" ile reddeder; parantez anlamı değiştirmez
                // (ADR-0079 §1.3).
                if starts_with_size_cast(&inner) {
                    (format!("{sym}({inner})"), PREC_UNARY)
                } else {
                    (format!("{sym}{inner}"), PREC_UNARY)
                }
            }
            ExprKind::Binary { op, lhs, rhs } => {
                let (op, lhs, rhs) = (*op, *lhs, *rhs);
                // Trit * x → çarpansız seçici (ADR-0003, bkz. trit.rs).
                let mul_ctx = (op == BinOp::Mul)
                    .then(|| self.arith_ctx(idx, ctx))
                    .and_then(|c| self.try_emit_trit_mul(lhs, rhs, c));
                match mul_ctx {
                    Some(text) => (text, PREC_TERNARY),
                    None => (self.emit_binary(idx, op, lhs, rhs, ctx), sv_prec(op)),
                }
            }
            ExprKind::Index { base, index } => {
                let (base, index) = (*base, *index);
                let packed =
                    crate::path_single(ast, base).and_then(|n| self.packed_arrays.get(n).copied());
                match self.try_emit_const_array_index(base, index, ctx, span) {
                    Some(folded) => {
                        let prec = lit_prec(&folded);
                        (folded, prec)
                    }
                    // Paketlenmiş port/wire dizisi elemanı (ADR-0056):
                    // `a[W*i +: W]`; işaretli eleman `$signed(...)` ile
                    // sarılır ki part-select'in işaretsizliği sızmasın.
                    None if packed.is_some() => {
                        let (elem, _) = packed.expect("packed");
                        let b = self.emit_prec(base, None, PREC_ATOM, false);
                        let i = self.emit_plain(index);
                        let sel = format!("{b}{}", crate::packed_select(&i, elem.width));
                        if elem.signed {
                            (format!("$signed({sel})"), PREC_ATOM)
                        } else {
                            (sel, PREC_ATOM)
                        }
                    }
                    None => {
                        let b = self.emit_prec(base, None, PREC_ATOM, false);
                        let i = self.emit_plain(index);
                        (format!("{b}[{i}]"), PREC_ATOM)
                    }
                }
            }
            ExprKind::Range { base, hi, lo } => {
                let (base, hi, lo) = (*base, *hi, *lo);
                let b = self.emit_prec(base, None, PREC_ATOM, false);
                let hi = self.emit_plain(hi);
                let lo = self.emit_plain(lo);
                (format!("{b}[{hi}:{lo}]"), PREC_ATOM)
            }
            ExprKind::PartSelect {
                base,
                start,
                width,
                ascending,
            } => {
                let (base, start, width, asc) = (*base, *start, *width, *ascending);
                let b = self.emit_prec(base, None, PREC_ATOM, false);
                let s = self.emit_plain(start);
                let w = self.emit_plain(width);
                let op = if asc { "+:" } else { "-:" };
                (format!("{b}[{s} {op} {w}]"), PREC_ATOM)
            }
            ExprKind::Field { base, field } => {
                let base = *base;
                let name = field.text.clone();
                // Yerleşik primitif ya da kullanıcı modülü çıkışı:
                // `f.rd_data` → `f_rd_data`.
                match crate::path_single(ast, base).filter(|inst| {
                    self.builtin_insts.contains_key(*inst) || self.user_insts.contains_key(*inst)
                }) {
                    Some(inst) => (format!("{inst}_{name}"), PREC_ATOM),
                    None => {
                        let b = self.emit_prec(base, None, PREC_ATOM, false);
                        (format!("{b}.{name}"), PREC_ATOM)
                    }
                }
            }
            // prev(x[, N]) (ADR-0040): $past ya da yardımcı reg.
            ExprKind::Call { callee, args } if self.is_prev_call(*callee) => {
                let args = args.clone();
                (self.emit_prev(idx, &args, ctx), PREC_ATOM)
            }
            // ADR-0090 §3: köprü yalnız modül düzeyinde, bağlamanın ya da
            // atamanın bütün sağ tarafıyken üretilir.
            ExprKind::Call { .. } if crate::is_sync_call(self.ast, idx) => {
                self.future(
                    span,
                    &lstr!(
                        en: "sync() inside an expression or a block (write it at module level as the whole right-hand side: 'let s = sync(x, clk)' or 's = sync(x, clk)')";
                        tr: "ifade ya da blok içinde sync() (modül düzeyinde sağ tarafın tamamı olarak yazın: 'let s = sync(x, clk)' ya da 's = sync(x, clk)')"
                    ),
                );
                ("1'b0".to_string(), PREC_ATOM)
            }
            ExprKind::Call { .. } => {
                self.future(
                    span,
                    &lstr!(
                        en: "function calls inside expressions";
                        tr: "ifade içinde fonksiyon çağrıları"
                    ),
                );
                ("1'b0".to_string(), PREC_ATOM)
            }
            ExprKind::Cast { expr, ty } => {
                let (expr, ty) = (*expr, *ty);
                (self.emit_cast(idx, expr, ty, span), PREC_ATOM)
            }
            ExprKind::If {
                cond,
                then_expr,
                else_expr,
            } => {
                let (cond, then_expr, else_expr) = (*cond, *then_expr, *else_expr);
                let one_bit = Some(Sig {
                    width: 1,
                    signed: false,
                });
                let c = self.emit_prec(cond, one_bit, PREC_UNARY, false);
                let t = self.emit_operand(then_expr, ctx, PREC_UNARY, false);
                // İç içe ternary parantezlenir (§5.4)
                let e = if matches!(ast.exprs[else_expr].kind, ExprKind::If { .. }) {
                    format!("({})", self.emit_prec(else_expr, ctx, PREC_TERNARY, false))
                } else {
                    self.emit_operand(else_expr, ctx, PREC_UNARY, false)
                };
                (format!("{c} ? {t} : {e}"), PREC_TERNARY)
            }
            // İç konum → üçlü zincir (ADR-0083 Karar 10.2).
            ExprKind::Match { .. } => self.emit_match_ternary(idx, ctx),
            ExprKind::Error => ("1'b0".to_string(), PREC_ATOM), // parse tanısı zaten var
            // Struct yaprakları MSB'den (ADR-0077 Karar 3/5): her öğe kendi
            // yaprak genişliğinde yazılır (literal yaprak boyutlanır).
            ExprKind::Concat(parts) => {
                let parts = parts.clone();
                let items: Vec<String> = parts
                    .iter()
                    .map(|&(e, t)| {
                        let sig = self.leaf_sig(t, span);
                        self.emit_prec(e, sig, PREC_TERNARY, false)
                    })
                    .collect();
                (format!("{{{}}}", items.join(", ")), PREC_ATOM)
            }
            // Dizi literalleri (ADR-0035): tekrar → '{default: v},
            // liste → '{a, b, ...}. Reg init/reset konumunda kullanılır.
            ExprKind::ArrayLit(volt_ast::ArrayLitKind::Repeat { value, .. }) => {
                let value = *value;
                let v = self.emit_prec(value, ctx, PREC_TERNARY, false);
                (format!("'{{default: {v}}}"), PREC_ATOM)
            }
            ExprKind::ArrayLit(volt_ast::ArrayLitKind::List(items)) => {
                let items = items.clone();
                let parts = items
                    .iter()
                    .map(|&i| self.emit_prec(i, ctx, PREC_TERNARY, false))
                    .collect::<Vec<_>>()
                    .join(", ");
                (format!("'{{{parts}}}"), PREC_ATOM)
            }
            // SV eşlemesi henüz olmayan ifade türleri (ADR-0070: türün adı).
            ExprKind::StringLit(_)
            | ExprKind::StructLit { .. }
            | ExprKind::TupleLit(_)
            | ExprKind::Todo { .. } => {
                let what = match &self.ast.exprs[idx].kind {
                    ExprKind::StringLit(_) => {
                        lstr!(en: "string literals in hardware"; tr: "donanımda string literalleri")
                    }
                    ExprKind::StructLit { .. } => {
                        lstr!(en: "struct literals"; tr: "struct literalleri")
                    }
                    ExprKind::TupleLit(_) => {
                        lstr!(en: "tuple literals"; tr: "tuple literalleri")
                    }
                    ExprKind::Todo { .. } => {
                        lstr!(en: "'todo!()' in hardware"; tr: "donanımda 'todo!()'")
                    }
                    ExprKind::IntLit { .. }
                    | ExprKind::BoolLit(_)
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
                    | ExprKind::ArrayLit(_)
                    | ExprKind::Concat(_)
                    | ExprKind::Error => {
                        unreachable!("dış kol yalnız StringLit/StructLit/TupleLit/Todo geçirir")
                    }
                };
                // ADR-0077: struct literali yalnız indirgenemeyen bir
                // sinyalin (struct dizisi, generic struct, bundle) içinde
                // kalır — o sinyal E0003'ünü aldı; ikinci tanı kaskaddır.
                let cascade = matches!(self.ast.exprs[idx].kind, ExprKind::StructLit { .. })
                    && self.diagnostics.iter().any(|d| d.code == ErrorCode::E0003);
                if !cascade {
                    self.future(span, &what);
                }
                ("1'b0".to_string(), PREC_ATOM)
            }
        };

        // Sol-birleşmeli operatörlerde eşit öncelikli SAĞ operand parantezlenir
        let needs_paren =
            my_prec < parent_prec || (my_prec == parent_prec && is_right_operand && my_prec > 0);
        if needs_paren && my_prec < PREC_ATOM {
            format!("({text})")
        } else {
            text
        }
    }

    /// İkili operatör. Aritmetikte etkin bağlam = max(kendi genişliğim,
    /// dış bağlam) — SV bağlam-belirlenimli semantiğin metne dökümü;
    /// dar atom operandlar `W'(x)` ile açıkça genişletilir (ADR-0041).
    fn emit_binary(
        &mut self,
        idx: Idx<Expr>,
        op: BinOp,
        lhs: Idx<Expr>,
        rhs: Idx<Expr>,
        ctx: Option<Sig>,
    ) -> String {
        let prec = sv_prec(op);
        let operand_ctx = match op {
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                self.width_of(lhs).or_else(|| self.width_of(rhs))
            }
            BinOp::And | BinOp::Or | BinOp::Imp => Some(Sig {
                width: 1,
                signed: false,
            }),
            // Kaydırmada sol operanda dış bağlam itilmez (ADR-0036).
            BinOp::Shl | BinOp::Shr => self.width_of(lhs),
            BinOp::Add
            | BinOp::Sub
            | BinOp::Mul
            | BinOp::Div
            | BinOp::Rem
            | BinOp::BitAnd
            | BinOp::BitOr
            | BinOp::BitXor => self.arith_ctx(idx, ctx),
        };
        // İmplikasyonun SV ifade karşılığı yok — `!a || b` açılımı
        // (ADR-0034); sol operand ! altında kalsın diye parantezlenir.
        if op == BinOp::Imp {
            let l = self.emit_prec(lhs, operand_ctx, PREC_UNARY, false);
            let r = self.emit_prec(rhs, operand_ctx, prec, true);
            return format!("!{l} || {r}");
        }
        let widen = is_arith(op);
        let l = if widen {
            self.emit_operand(lhs, operand_ctx, prec, false)
        } else {
            let text = self.emit_prec(lhs, operand_ctx, prec, false);
            let text = self.paren_comparison_under_bitwise(op, lhs, text);
            self.fit_bitwise_operand(op, lhs, text, operand_ctx)
        };
        let r = if matches!(op, BinOp::Shl | BinOp::Shr) {
            let inner = self.emit_plain(rhs);
            // kaydırma miktarı atom değilse parantezle
            if matches!(&self.ast.exprs[rhs].kind, ExprKind::Binary { .. }) {
                format!("({inner})")
            } else {
                inner
            }
        } else if widen {
            self.emit_operand(rhs, operand_ctx, prec, true)
        } else {
            let text = self.emit_prec(rhs, operand_ctx, prec, true);
            let text = self.paren_comparison_under_bitwise(op, rhs, text);
            self.fit_bitwise_operand(op, rhs, text, operand_ctx)
        };
        // İşaretli sağ kaydırma aritmetiktir (ADR-0036): SV'de
        // `>>` her zaman mantıksal; işaret ancak `>>>` ile korunur.
        let sym = if op == BinOp::Shr && self.width_of(lhs).is_some_and(|s| s.signed) {
            ">>>"
        } else {
            op.symbol()
        };
        format!("{l} {sym} {r}")
    }

    /// Aritmetik/bit düzeyi operand bağlamı: max(işlem genişliğim, dış
    /// bağlam), işaret ifadenin kendisinden (ADR-0041). İşlem genişliği
    /// operandların ortak genişliğidir; tipsiz `let`in esnek aralığı
    /// (ADR-0025) karşı operandla bu genişlikte buluşur.
    fn arith_ctx(&mut self, idx: Idx<Expr>, ctx: Option<Sig>) -> Option<Sig> {
        match (self.operation_sig(idx), ctx) {
            (Some(own), Some(c)) => Some(Sig {
                width: own.width.max(c.width),
                signed: own.signed,
            }),
            (own, c) => own.or(c),
        }
    }

    /// Bit düzeyi operatörün karşılaştırma operandı: SV tablosu parantez
    /// İSTEMEZ (`a & b == 0` zaten `a & (b == 0)` okunur) ama iki dilin
    /// ayrıştığı tek düzey burası olduğundan okur hangi tabloyu aklında
    /// tutarsa tutsun aynı ağacı görsün diye parantez basılır (ADR-0057).
    fn paren_comparison_under_bitwise(
        &self,
        parent: BinOp,
        operand: Idx<Expr>,
        text: String,
    ) -> String {
        let comparison = matches!(
            &self.ast.exprs[operand].kind,
            ExprKind::Binary { op, .. } if is_comparison(*op)
        );
        if is_bitwise(parent) && comparison {
            format!("({text})")
        } else {
            text
        }
    }

    /// Üst düzey const referansını boyutlandırılmış literale katlar.
    /// Modül sinyalleri, örnekler ve döngü değişkeni aynı adı gölgeler;
    /// döngü değişkeni bağlamla boyutlanan literal olur; dizi sabiti
    /// çıplak kullanılamaz (E2005 — sessiz sızıntı yasak).
    fn fold_const_path(
        &mut self,
        path: &volt_ast::Path,
        ctx: Option<Sig>,
        span: volt_span::Span,
        global: bool,
    ) -> Option<String> {
        if path.segments.len() != 1 {
            return None;
        }
        let name = &path.segments[0].text;
        if !global
            && (self.symbols.contains_key(name)
                || self.builtin_insts.contains_key(name)
                || self.user_insts.contains_key(name))
        {
            return None;
        }
        if let Some(v) = self.loop_var(name).filter(|_| !global) {
            return Some(self.fmt_int(v, NumBase::Dec, ctx, span));
        }
        let &(ty, value_idx) = self.consts.get(name)?;
        if matches!(
            self.ast.types[crate::alias::resolve(self.ast, ty)].kind,
            TypeRefKind::Array { .. }
        ) {
            self.error(
                ErrorCode::E2005,
                lstr!(
                    en: "constant array '{name}' cannot be used as a value; index it (e.g. {name}[i])";
                    tr: "'{name}' sabit dizisi değer olarak kullanılamaz; indeksleyin (ör. {name}[i])"
                ),
                span,
                &lstr!(
                    en: "a whole-array constant is only valid as a register initializer";
                    tr: "bütün dizi sabiti yalnız register başlangıç değeri olabilir"
                ),
            );
            return None;
        }
        // Enum tipli const kullanım yerinde varyant adıyla iner (ADR-0074).
        if self.enum_of_type(ty).is_some() {
            return Some(self.emit_prec(value_idx, None, PREC_ATOM, false));
        }
        // Skaler const SV'de bildirilmez; katlanamazsa çıplak ad tanımsız
        // kalırdı ("Volt tamam, çıktı geçersiz") — açık tanı (ADR-0083).
        let Some(value) = self.eval_const(value_idx) else {
            self.future(
                span,
                &lstr!(
                    en: "constant '{name}' whose value the SystemVerilog emitter cannot fold";
                    tr: "değeri SystemVerilog üreticisinin katlayamadığı '{name}' sabiti"
                ),
            );
            return None;
        };
        let base = if let ExprKind::IntLit { base, .. } = &self.ast.exprs[value_idx].kind {
            *base
        } else {
            NumBase::Dec
        };
        let declared = self.sig_of_typeref(ty, span);
        let sig = widen_sig(declared, ctx);
        Some(self.fmt_int(value, base, sig, span))
    }

    /// Zero/sign-extend veya daraltma (§6). Basit isimde ADR-0036'nın
    /// `{{n{x[msb]}}, x}` / `x[w-1:0]` biçimleri korunur; bileşik
    /// operandda SV boyut dönüşümü `W'(expr)` basılır (ADR-0041).
    fn emit_cast(
        &mut self,
        cast_idx: Idx<Expr>,
        operand: Idx<Expr>,
        ty: Idx<volt_ast::TypeRef>,
        span: volt_span::Span,
    ) -> String {
        let Some(target) = self.sig_of_typeref(ty, span) else {
            return self.emit_prec(operand, None, PREC_ATOM, false);
        };
        // ADR-0081 ikame kipi: argüman/sonuç parametre ya da dönüş
        // tipinin genişliğinde hesaplanır — SV boyut dönüşümü işleneni
        // atama bağlamında (hedef genişlikte) değerlendirir.
        if self.inline_notes.sized_casts.contains(&cast_idx) {
            let inner = self.emit_prec(operand, Some(target), PREC_TERNARY, false);
            return format!("{}'({inner})", target.width);
        }
        let src = self.width_of(operand);
        // Soneksiz literal (`0 as bits<8>`): hedef genişliğinde
        // boyutlandırılmış literal — ara genişlik yoktur (ADR-0051).
        if src.is_none() && matches!(self.ast.exprs[operand].kind, ExprKind::IntLit { .. }) {
            return self.emit_prec(operand, Some(target), PREC_ATOM, false);
        }
        let inner = self.emit_prec(operand, src, PREC_ATOM, false);

        let Some(src) = src else {
            self.error(
                ErrorCode::E2005,
                lstr!(
                    en: "cannot determine the width of the cast source";
                    tr: "dönüşüm kaynağının genişliği belirlenemiyor"
                ),
                span,
                &lstr!(
                    en: "specify the operand's type explicitly";
                    tr: "operandın tipini açıkça belirtin"
                ),
            );
            return inner;
        };

        // Enum kodu sıfır genişletmeyle çıkar: `N'(e)` (ADR-0074 kural 5).
        if self.enum_of_expr(operand).is_some() {
            return if target.width > src.width {
                format!("{}'({inner})", target.width)
            } else {
                inner
            };
        }
        let simple = matches!(&self.ast.exprs[operand].kind, ExprKind::Path(_));
        if target.width > src.width {
            let n = target.width - src.width;
            match (src.signed, simple) {
                // sign-extend: {{N{a[msb]}}, a} — basit isimde
                (true, true) => format!("{{{{{n}{{{inner}[{}]}}}}, {inner}}}", src.width - 1),
                (false, _) => format!("{{{{{n}{{1'b0}}}}, {inner}}}"),
                (true, false) => format!("{}'({inner})", target.width),
            }
        } else if target.width == src.width {
            // Aynı genişlikte işaret DEĞİŞİYORSA no-op değildir (ADR-0036):
            // $signed/$unsigned sarmalayıcısı hem karşılaştırma/kaydırma
            // semantiğini kurar hem de SV işaret-bağlamı sızıntısını kesen
            // öz-belirlenimli (self-determined) bir sınır oluşturur.
            if target.signed && !src.signed {
                format!("$signed({inner})")
            } else if !target.signed && src.signed {
                format!("$unsigned({inner})")
            } else {
                inner
            }
        } else if simple {
            // daraltma: basit isim dilimlenir
            format!("{inner}[{}:0]", target.width - 1)
        } else {
            format!("{}'({inner})", target.width)
        }
    }

    /// Sayısal literal boyutlandırma (§10). Negatif değer `-16'sd2`.
    pub(crate) fn fmt_int(
        &mut self,
        value: i128,
        base: NumBase,
        sig: Option<Sig>,
        span: volt_span::Span,
    ) -> String {
        let Some(sig) = sig else {
            self.error(
                ErrorCode::E2005,
                lstr!(
                    en: "cannot determine the width of the literal '{value}'";
                    tr: "'{value}' literalinin genişliği belirlenemiyor"
                ),
                span,
                &lstr!(
                    en: "assign to a target type or add a suffix (e.g. 42u8)";
                    tr: "hedef tipe atayın veya sonek ekleyin (ör. 42u8)"
                ),
            );
            return value.to_string();
        };
        let w = sig.width;
        let neg = if value < 0 { "-" } else { "" };
        let mag = value.unsigned_abs();
        match base {
            NumBase::Hex => format!("{neg}{w}'h{mag:X}"),
            NumBase::Bin => format!("{neg}{w}'b{mag:b}"),
            NumBase::Oct => format!("{neg}{w}'o{mag:o}"),
            NumBase::Dec if sig.signed => format!("{neg}{w}'sd{mag}"),
            NumBase::Dec => format!("{neg}{w}'d{mag}"),
        }
    }
}

fn suffix_sig(suffix: volt_ast::IntSuffix) -> Sig {
    use volt_ast::IntSuffix::*;
    match suffix {
        U8 => Sig {
            width: 8,
            signed: false,
        },
        U16 => Sig {
            width: 16,
            signed: false,
        },
        U32 => Sig {
            width: 32,
            signed: false,
        },
        U64 => Sig {
            width: 64,
            signed: false,
        },
        I8 => Sig {
            width: 8,
            signed: true,
        },
        I16 => Sig {
            width: 16,
            signed: true,
        },
        I32 => Sig {
            width: 32,
            signed: true,
        },
        I64 => Sig {
            width: 64,
            signed: true,
        },
    }
}
