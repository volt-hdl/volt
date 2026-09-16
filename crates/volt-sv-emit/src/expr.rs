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

use volt_ast::{BinOp, Expr, ExprKind, Idx, NumBase, TypeRefKind, UnOp};
use volt_diagnostics::{lstr, ErrorCode};

use crate::Emitter;

/// Sinyal genişliği ve işareti.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sig {
    pub width: u32,
    pub signed: bool,
}

impl Sig {
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

/// SV çıktısında parantez kararı için öncelik (Volt tablosuyla uyumlu,
/// operator-precedence.md §3'ten türetildi).
fn sv_prec(op: BinOp) -> u8 {
    match op {
        // İmplikasyon SV'de `!a || b` olarak açıldığından || düzeyinde.
        BinOp::Imp => 1,
        BinOp::Or => 1,
        BinOp::And => 2,
        BinOp::Eq | BinOp::Ne => 3,
        BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => 4,
        BinOp::BitOr => 5,
        BinOp::BitXor => 6,
        BinOp::BitAnd => 7,
        BinOp::Shl | BinOp::Shr => 8,
        BinOp::Add | BinOp::Sub => 9,
        BinOp::Mul | BinOp::Div | BinOp::Rem => 10,
    }
}

const PREC_TERNARY: u8 = 0;
const PREC_UNARY: u8 = 11;
const PREC_ATOM: u8 = 12;

/// Negatif literal (`-16'sd2`) operand konumunda parantezlenir.
fn lit_prec(text: &str) -> u8 {
    if text.starts_with('-') {
        PREC_TERNARY
    } else {
        PREC_ATOM
    }
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
            ExprKind::Binary { op, lhs, rhs } => {
                let l = self.eval_const_depth(*lhs, depth + 1)?;
                let r = self.eval_const_depth(*rhs, depth + 1)?;
                match op {
                    BinOp::Add => l.checked_add(r),
                    BinOp::Sub => l.checked_sub(r),
                    BinOp::Mul => l.checked_mul(r),
                    BinOp::Div => l.checked_div(r),
                    _ => None,
                }
            }
            // Modül sinyalleri gölgeler; döngü değişkeni const'tan önce.
            ExprKind::Path(p) if p.segments.len() == 1 => {
                let name = &p.segments[0].text;
                if self.symbols.contains_key(name) {
                    return None;
                }
                if let Some(v) = self.loop_var(name) {
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
            _ => None,
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
            ExprKind::Path(path) => {
                let name = path.segments.first()?;
                if let Some(sig) = self.symbols.get(&name.text).copied() {
                    return Some(sig);
                }
                // Döngü değişkeni bağlamla boyutlanan literal gibidir.
                if self.loop_var(&name.text).is_some() {
                    return None;
                }
                // Üst düzey const: genişlik bildirilen tipinden gelir.
                let &(ty, _) = self.consts.get(&name.text)?;
                if matches!(ast.types[ty].kind, TypeRefKind::Array { .. }) {
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
            // F1 parser yapıları — SV üretimi sonraki aşamalarda
            ExprKind::StringLit(_)
            | ExprKind::Match { .. }
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
            _ => false,
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
        if !(self.is_widen_atom(idx) || (whole_shift && is_shift)) {
            return text;
        }
        match self.width_of(idx) {
            Some(src) if src.width < target.width && src.signed == target.signed => {
                format!("{}'({text})", target.width)
            }
            _ => text,
        }
    }

    /// Atama konumu (assign / let / `<=` / `=` / port bağlama): RHS
    /// hedef imzasıyla basılır, dar atom ya da kaydırma sonucu sarılır.
    pub(crate) fn emit_assigned(&mut self, idx: Idx<Expr>, target: Option<Sig>) -> String {
        let text = self.emit_expr(idx, target);
        self.widen_if_narrow(idx, text, target, true)
    }

    /// Aritmetik/tekli/ternary operandı: bağlamla basılır, dar atom sarılır.
    fn emit_operand(
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
            _ => match self.eval_const(idx) {
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
            ExprKind::Path(path) => {
                // Üst düzey const referansı boyutlandırılmış literale
                // katlanır — üretilen RTL'de tanımsız isim kalmaz.
                match self.fold_const_path(path, ctx, span) {
                    Some(folded) => {
                        let prec = lit_prec(&folded);
                        (folded, prec)
                    }
                    None => {
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
                (format!("{sym}{inner}"), PREC_UNARY)
            }
            ExprKind::Binary { op, lhs, rhs } => {
                let (op, lhs, rhs) = (*op, *lhs, *rhs);
                (self.emit_binary(idx, op, lhs, rhs, ctx), sv_prec(op))
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
            ExprKind::Call { .. } => {
                self.future(
                    span,
                    &lstr!(
                        en: "function calls (including sync)";
                        tr: "fonksiyon çağrıları (sync dahil)"
                    ),
                );
                ("1'b0".to_string(), PREC_ATOM)
            }
            ExprKind::Cast { expr, ty } => {
                let (expr, ty) = (*expr, *ty);
                (self.emit_cast(expr, ty, span), PREC_ATOM)
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
                let e = match &ast.exprs[else_expr].kind {
                    ExprKind::If { .. } => {
                        format!("({})", self.emit_prec(else_expr, ctx, PREC_TERNARY, false))
                    }
                    _ => self.emit_operand(else_expr, ctx, PREC_UNARY, false),
                };
                (format!("{c} ? {t} : {e}"), PREC_TERNARY)
            }
            ExprKind::Error => ("1'b0".to_string(), PREC_ATOM), // parse tanısı zaten var
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
            // F1 parser yapıları — SV üretimi sonraki aşamalarda
            ExprKind::StringLit(_)
            | ExprKind::Match { .. }
            | ExprKind::StructLit { .. }
            | ExprKind::TupleLit(_)
            | ExprKind::Todo { .. } => {
                self.future(
                    span,
                    &lstr!(
                        en: "SV generation of this expression kind";
                        tr: "bu ifade türünün SV üretimi"
                    ),
                );
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
            _ => match (self.width_of(idx), ctx) {
                (Some(own), Some(c)) => Some(Sig {
                    width: own.width.max(c.width),
                    signed: own.signed,
                }),
                (own, c) => own.or(c),
            },
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
            self.emit_prec(lhs, operand_ctx, prec, false)
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
            self.emit_prec(rhs, operand_ctx, prec, true)
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

    /// Üst düzey const referansını boyutlandırılmış literale katlar.
    /// Modül sinyalleri, örnekler ve döngü değişkeni aynı adı gölgeler;
    /// döngü değişkeni bağlamla boyutlanan literal olur; dizi sabiti
    /// çıplak kullanılamaz (E2005 — sessiz sızıntı yasak).
    fn fold_const_path(
        &mut self,
        path: &volt_ast::Path,
        ctx: Option<Sig>,
        span: volt_span::Span,
    ) -> Option<String> {
        if path.segments.len() != 1 {
            return None;
        }
        let name = &path.segments[0].text;
        if self.symbols.contains_key(name)
            || self.builtin_insts.contains_key(name)
            || self.user_insts.contains_key(name)
        {
            return None;
        }
        if let Some(v) = self.loop_var(name) {
            return Some(self.fmt_int(v, NumBase::Dec, ctx, span));
        }
        let &(ty, value_idx) = self.consts.get(name)?;
        if matches!(self.ast.types[ty].kind, TypeRefKind::Array { .. }) {
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
        let value = self.eval_const(value_idx)?;
        let base = match &self.ast.exprs[value_idx].kind {
            ExprKind::IntLit { base, .. } => *base,
            _ => NumBase::Dec,
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
        operand: Idx<Expr>,
        ty: Idx<volt_ast::TypeRef>,
        span: volt_span::Span,
    ) -> String {
        let Some(target) = self.sig_of_typeref(ty, span) else {
            return self.emit_prec(operand, None, PREC_ATOM, false);
        };
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
