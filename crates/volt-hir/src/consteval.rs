//! Derleme zamanı değerlendirme (docs/spec/const-eval.md §1-§6).
//!
//! Taşma HATA, sarma değil (checked_* → E2022). Sıfıra bölme E2023,
//! geçersiz kaydırma E2024, döngüsel sabit E2020, çalışma zamanı değeri
//! E2021. Sonuçlar önbelleklenir; aynı ifade iki kez hesaplanmaz ve
//! aynı hata iki kez raporlanmaz. for açma (§8) F2'ye ertelendi.

use std::collections::HashMap;

use volt_ast::{
    ArrayLitKind, BinOp, Expr, ExprKind, Idx, ItemKind, SourceFile, TypeRef, TypeRefKind, UnOp,
};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use crate::resolve::{BuiltinKind, DefId, DefKind, ResolveResult};

pub const MAX_WIDTH: u32 = 65_536;
pub const MAX_ARRAY_LEN: usize = 1_048_576; // 1M eleman

/// Derleme zamanı değeri. `i128` ara hesap taşmasını yakalamak için.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ConstValue {
    Int(i128),
    Bool(bool),
    Array(Vec<ConstValue>),
    Tuple(Vec<ConstValue>),
    EnumVariant {
        def: DefId,
        discriminant: i128,
    },
    /// Hesaplanamadı — hata zaten raporlandı.
    Error,
}

pub struct ConstEvaluator<'a> {
    ast: &'a SourceFile,
    res: &'a ResolveResult,
    /// İfade önbelleği — aynı ifade tekrar hesaplanmasın.
    expr_cache: HashMap<Idx<Expr>, ConstValue>,
    /// Const tanımı önbelleği.
    def_cache: HashMap<DefId, ConstValue>,
    /// Şu an değerlendirilmekte olan sabitler (E2020 döngü tespiti).
    in_progress: Vec<DefId>,
    pub diagnostics: Vec<Diagnostic>,
}

impl<'a> ConstEvaluator<'a> {
    pub fn new(ast: &'a SourceFile, res: &'a ResolveResult) -> Self {
        ConstEvaluator {
            ast,
            res,
            expr_cache: HashMap::new(),
            def_cache: HashMap::new(),
            in_progress: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    /// Dosyadaki tüm const bildirimlerini değerlendirir (E2020 vb.).
    pub fn eval_all_consts(&mut self) {
        // Bildirim sırası: HashMap sırası rastgele olduğundan E2020 gibi
        // döngü tanılarının konumu koşudan koşuya değişirdi.
        let mut const_defs: Vec<DefId> = self.res.const_inits.keys().copied().collect();
        const_defs.sort_by_key(|d| d.0);
        for def in const_defs {
            let _ = self.eval_const_def(def);
        }
    }

    /// Tip pozisyonlarını denetler: bits<N>, [T; N] (E2021/E2025/E2026).
    pub fn check_type_positions(&mut self) {
        for &item_idx in &self.ast.items {
            match &self.ast.items_arena[item_idx].kind {
                ItemKind::Module(m) => {
                    for p in &m.ports {
                        self.check_type(p.ty);
                    }
                    for &stmt in &m.body {
                        match &self.ast.stmts[stmt].kind {
                            volt_ast::StmtKind::Reg(r) => {
                                if let Some(ty) = r.ty {
                                    self.check_type(ty);
                                }
                            }
                            volt_ast::StmtKind::Wire(w) => self.check_type(w.ty),
                            volt_ast::StmtKind::Let(l) => {
                                if let Some(ty) = l.ty {
                                    self.check_type(ty);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                ItemKind::Const(c) => self.check_type(c.ty),
                ItemKind::Struct(s) => {
                    for f in &s.fields {
                        self.check_type(f.ty);
                    }
                }
                ItemKind::Extern(x) => {
                    for p in &x.ports {
                        self.check_type(p.ty);
                    }
                }
                _ => {}
            }
        }
    }

    fn check_type(&mut self, ty_idx: Idx<TypeRef>) {
        let ty = &self.ast.types[ty_idx];
        match &ty.kind {
            TypeRefKind::Bits(e) => {
                let _ = self.eval_type_arg(*e);
            }
            TypeRefKind::Array { elem, len } => {
                self.check_type(*elem);
                if let ConstValue::Int(n) = self.const_eval(*len) {
                    if n < 0 || n as u128 > MAX_ARRAY_LEN as u128 {
                        let span = self.span_of(*len);
                        self.diagnostics.push(Diagnostic::error(
                            ErrorCode::E2026,
                            lstr!(
                                en: "array size out of bounds: {n}";
                                tr: "dizi boyutu sınır dışı: {n}"
                            ),
                            LabeledSpan::primary(
                                span,
                                lstr!(en: "invalid size"; tr: "geçersiz boyut"),
                            ),
                            lstr!(
                                en: "size must be in the range 0..{MAX_ARRAY_LEN}";
                                tr: "boyut 0..{MAX_ARRAY_LEN} aralığında olmalı"
                            ),
                        ));
                    }
                }
            }
            TypeRefKind::Tuple(items) => {
                for &t in items.clone().iter() {
                    self.check_type(t);
                }
            }
            _ => {}
        }
    }

    /// bits<N> / [T; N] genişlik hesabı (const-eval.md §7).
    pub fn eval_type_arg(&mut self, expr: Idx<Expr>) -> Option<u32> {
        let span = self.span_of(expr);
        match self.const_eval(expr) {
            ConstValue::Int(n) if n > 0 && n <= MAX_WIDTH as i128 => Some(n as u32),
            ConstValue::Int(n) => {
                let reason = if n <= 0 {
                    lstr!(en: "width must be positive"; tr: "genişlik pozitif olmalı")
                } else {
                    lstr!(en: "width is too large"; tr: "genişlik çok büyük")
                };
                self.diagnostics.push(
                    Diagnostic::error(
                        ErrorCode::E2025,
                        lstr!(en: "invalid width: {n}"; tr: "geçersiz genişlik: {n}"),
                        LabeledSpan::primary(span, reason),
                        lstr!(
                            en: "width must be in the range 1..={MAX_WIDTH}";
                            tr: "genişlik 1..={MAX_WIDTH} aralığında olmalı"
                        ),
                    )
                    .with_note(
                        NoteKind::Reason,
                        lstr!(
                            en: "bits<0> or a huge width cannot be synthesized";
                            tr: "bits<0> veya devasa genişlik sentezlenemez"
                        ),
                    ),
                );
                None
            }
            ConstValue::Error => Some(1), // hata kurtarma — zaten raporlandı
            _ => {
                self.diagnostics.push(Diagnostic::error(
                    ErrorCode::E2021,
                    lstr!(en: "expected a constant expression"; tr: "sabit ifade bekleniyor"),
                    LabeledSpan::primary(
                        span,
                        lstr!(en: "not a numeric constant"; tr: "sayısal sabit değil"),
                    ),
                    lstr!(
                        en: "width must be an integer constant";
                        tr: "genişlik tam sayı bir sabit olmalı"
                    ),
                ));
                None
            }
        }
    }

    // ═══ Const tanımı (§5: döngü tespiti) ═════════════════════════

    pub fn eval_const_def(&mut self, def: DefId) -> ConstValue {
        if let Some(cached) = self.def_cache.get(&def) {
            return cached.clone();
        }

        // Döngü kontrolü.
        if let Some(pos) = self.in_progress.iter().position(|d| *d == def) {
            let cycle: Vec<String> = self.in_progress[pos..]
                .iter()
                .map(|d| self.res.defs[d.0 as usize].name.clone())
                .collect();
            let span = self.res.defs[def.0 as usize].span;
            let first = cycle.first().cloned().unwrap_or_default();
            self.diagnostics.push(
                Diagnostic::error(
                    ErrorCode::E2020,
                    lstr!(en: "cyclic constant dependency"; tr: "döngüsel sabit bağımlılığı"),
                    LabeledSpan::primary(
                        span,
                        lstr!(
                            en: "while evaluating '{first}'";
                            tr: "'{first}' hesaplanırken"
                        ),
                    ),
                    lstr!(
                        en: "remove one of the dependencies";
                        tr: "bağımlılıklardan birini kaldırın"
                    ),
                )
                .with_note(
                    NoteKind::Note,
                    lstr!(
                        en: "cycle: {} → {}", cycle.join(" → "), first;
                        tr: "döngü: {} → {}", cycle.join(" → "), first
                    ),
                ),
            );
            return ConstValue::Error;
        }

        let Some(&init) = self.res.const_inits.get(&def) else {
            return ConstValue::Error;
        };

        self.in_progress.push(def);
        let result = self.const_eval(init);
        self.in_progress.pop();

        self.def_cache.insert(def, result.clone());
        result
    }

    // ═══ İfade değerlendirme (§3) ═════════════════════════════════

    pub fn const_eval(&mut self, expr: Idx<Expr>) -> ConstValue {
        if let Some(cached) = self.expr_cache.get(&expr) {
            return cached.clone();
        }
        let result = self.const_eval_uncached(expr);
        self.expr_cache.insert(expr, result.clone());
        result
    }

    fn const_eval_uncached(&mut self, expr: Idx<Expr>) -> ConstValue {
        let e = &self.ast.exprs[expr];
        let span = e.span;
        match &e.kind {
            ExprKind::IntLit { value, .. } => ConstValue::Int(*value as i128),
            ExprKind::BoolLit(b) => ConstValue::Bool(*b),

            ExprKind::Path(_) => {
                let Some(&def) = self.res.resolutions.get(&expr) else {
                    return ConstValue::Error;
                };
                match self.res.def_kind(def) {
                    DefKind::Const => self.eval_const_def(def),
                    DefKind::EnumVariant { .. } => self.variant_value(def),
                    DefKind::Error => ConstValue::Error,
                    other => {
                        self.error_not_constant(span, &describe_def_kind(other));
                        ConstValue::Error
                    }
                }
            }

            ExprKind::Binary { op, lhs, rhs } => {
                let (op, lhs, rhs) = (*op, *lhs, *rhs);
                let l = self.const_eval(lhs);
                let r = self.const_eval(rhs);
                self.eval_binop(op, l, r, span)
            }

            ExprKind::Unary { op, operand } => {
                let (op, operand) = (*op, *operand);
                let v = self.const_eval(operand);
                self.eval_unop(op, v, span)
            }

            ExprKind::If {
                cond,
                then_expr,
                else_expr,
            } => {
                let (cond, then_expr, else_expr) = (*cond, *then_expr, *else_expr);
                match self.const_eval(cond) {
                    ConstValue::Bool(true) => self.const_eval(then_expr),
                    ConstValue::Bool(false) => self.const_eval(else_expr),
                    ConstValue::Error => ConstValue::Error,
                    _ => {
                        self.type_mismatch(span, "bool");
                        ConstValue::Error
                    }
                }
            }

            ExprKind::Cast { expr: inner, .. } => {
                // Sabit değer cast'te değişmez; sığdırma tip kontrolü işi.
                self.const_eval(*inner)
            }

            ExprKind::Call { callee, args } => {
                let (callee, args) = (*callee, args.clone());
                self.eval_builtin_call(callee, &args, span)
            }

            ExprKind::ArrayLit(ArrayLitKind::List(items)) => {
                let items = items.clone();
                let mut vals = Vec::with_capacity(items.len());
                for i in items {
                    match self.const_eval(i) {
                        ConstValue::Error => return ConstValue::Error,
                        v => vals.push(v),
                    }
                }
                ConstValue::Array(vals)
            }

            ExprKind::ArrayLit(ArrayLitKind::Repeat { value, count }) => {
                let (value, count) = (*value, *count);
                let v = self.const_eval(value);
                let ConstValue::Int(n) = self.const_eval(count) else {
                    return ConstValue::Error;
                };
                if n < 0 || n as u128 > MAX_ARRAY_LEN as u128 {
                    self.diagnostics.push(Diagnostic::error(
                        ErrorCode::E2026,
                        lstr!(
                            en: "array size out of bounds: {n}";
                            tr: "dizi boyutu sınır dışı: {n}"
                        ),
                        LabeledSpan::primary(
                            span,
                            lstr!(en: "invalid repeat count"; tr: "geçersiz tekrar sayısı"),
                        ),
                        lstr!(
                            en: "size must be in the range 0..{MAX_ARRAY_LEN}";
                            tr: "boyut 0..{MAX_ARRAY_LEN} aralığında olmalı"
                        ),
                    ));
                    return ConstValue::Error;
                }
                ConstValue::Array(vec![v; n as usize])
            }

            ExprKind::TupleLit(items) => {
                let items = items.clone();
                let mut vals = Vec::with_capacity(items.len());
                for i in items {
                    match self.const_eval(i) {
                        ConstValue::Error => return ConstValue::Error,
                        v => vals.push(v),
                    }
                }
                ConstValue::Tuple(vals)
            }

            ExprKind::Index { base, index } => {
                let (base, index) = (*base, *index);
                let arr = self.const_eval(base);
                let idx = self.const_eval(index);
                match (arr, idx) {
                    (ConstValue::Error, _) | (_, ConstValue::Error) => ConstValue::Error,
                    (ConstValue::Array(items), ConstValue::Int(i)) => {
                        if i >= 0 && (i as usize) < items.len() {
                            items[i as usize].clone()
                        } else {
                            self.diagnostics.push(Diagnostic::error(
                                ErrorCode::E2029,
                                lstr!(
                                    en: "constant array index out of bounds: {i} (length {})",
                                        items.len();
                                    tr: "sabit dizi indeksi sınır dışı: {i} (uzunluk {})",
                                        items.len()
                                ),
                                LabeledSpan::primary(
                                    span,
                                    lstr!(en: "invalid index"; tr: "geçersiz indeks"),
                                ),
                                lstr!(
                                    en: "index must be in the range 0..{}", items.len();
                                    tr: "indeks 0..{} aralığında olmalı", items.len()
                                ),
                            ));
                            ConstValue::Error
                        }
                    }
                    _ => {
                        self.error_not_constant(
                            span,
                            &lstr!(en: "this expression"; tr: "bu ifade"),
                        );
                        ConstValue::Error
                    }
                }
            }

            ExprKind::Error => ConstValue::Error,

            _ => {
                self.error_not_constant(span, &lstr!(en: "this expression"; tr: "bu ifade"));
                ConstValue::Error
            }
        }
    }

    /// Enum varyantının ayrıştırıcı değeri: açık ifade ya da sıra.
    fn variant_value(&mut self, def: DefId) -> ConstValue {
        let Some(&(index, disc)) = self.res.variant_info.get(&def) else {
            return ConstValue::Error;
        };
        let discriminant = match disc {
            Some(expr) => match self.const_eval(expr) {
                ConstValue::Int(n) => n,
                _ => return ConstValue::Error,
            },
            None => index as i128,
        };
        ConstValue::EnumVariant { def, discriminant }
    }

    // ═══ İkili işlemler (§4: taşma kuralları) ═════════════════════

    fn eval_binop(&mut self, op: BinOp, l: ConstValue, r: ConstValue, span: Span) -> ConstValue {
        use ConstValue::{Bool, Error, Int};
        match (l, r) {
            (Error, _) | (_, Error) => Error,

            (Int(a), Int(b)) => {
                let v = match op {
                    BinOp::Add => a.checked_add(b),
                    BinOp::Sub => a.checked_sub(b),
                    BinOp::Mul => a.checked_mul(b),

                    BinOp::Div => {
                        if b == 0 {
                            self.division_by_zero(span);
                            return Error;
                        }
                        a.checked_div(b)
                    }
                    BinOp::Rem => {
                        if b == 0 {
                            self.division_by_zero(span);
                            return Error;
                        }
                        a.checked_rem(b)
                    }

                    BinOp::Shl => {
                        if !(0..128).contains(&b) {
                            self.shift_overflow(span, b);
                            return Error;
                        }
                        a.checked_shl(b as u32)
                    }
                    BinOp::Shr => {
                        if !(0..128).contains(&b) {
                            self.shift_overflow(span, b);
                            return Error;
                        }
                        a.checked_shr(b as u32)
                    }

                    BinOp::BitAnd => Some(a & b),
                    BinOp::BitOr => Some(a | b),
                    BinOp::BitXor => Some(a ^ b),

                    BinOp::Eq => return Bool(a == b),
                    BinOp::Ne => return Bool(a != b),
                    BinOp::Lt => return Bool(a < b),
                    BinOp::Gt => return Bool(a > b),
                    BinOp::Le => return Bool(a <= b),
                    BinOp::Ge => return Bool(a >= b),

                    BinOp::And | BinOp::Or => {
                        self.type_mismatch(span, "bool");
                        return Error;
                    }
                };

                match v {
                    Some(n) => Int(n),
                    None => {
                        self.diagnostics.push(
                            Diagnostic::error(
                                ErrorCode::E2022,
                                lstr!(en: "compile-time overflow"; tr: "derleme zamanı taşması"),
                                LabeledSpan::primary(
                                    span,
                                    lstr!(
                                        en: "the result of '{}' does not fit in i128",
                                            op.symbol();
                                        tr: "'{}' işleminin sonucu i128'e sığmıyor",
                                            op.symbol()
                                    ),
                                ),
                                lstr!(en: "use smaller values"; tr: "daha küçük değerler kullanın"),
                            )
                            .with_note(
                                NoteKind::Reason,
                                lstr!(
                                    en: "compile-time overflow is an error, it does not wrap";
                                    tr: "derleme zamanı taşma hatadır, sarmaz"
                                ),
                            ),
                        );
                        Error
                    }
                }
            }

            (Bool(a), Bool(b)) => match op {
                BinOp::And => Bool(a && b),
                BinOp::Or => Bool(a || b),
                BinOp::Eq => Bool(a == b),
                BinOp::Ne => Bool(a != b),
                _ => {
                    self.type_mismatch(span, &lstr!(en: "numeric"; tr: "sayısal"));
                    Error
                }
            },

            _ => {
                self.type_mismatch(span, &lstr!(en: "the same type"; tr: "aynı tip"));
                Error
            }
        }
    }

    fn eval_unop(&mut self, op: UnOp, v: ConstValue, span: Span) -> ConstValue {
        use ConstValue::{Bool, Error, Int};
        match (op, v) {
            (_, Error) => Error,
            (UnOp::Neg, Int(a)) => match a.checked_neg() {
                Some(n) => Int(n),
                None => {
                    self.diagnostics.push(Diagnostic::error(
                        ErrorCode::E2022,
                        lstr!(en: "compile-time overflow"; tr: "derleme zamanı taşması"),
                        LabeledSpan::primary(
                            span,
                            lstr!(
                                en: "negation does not fit in i128";
                                tr: "negatifleme i128'e sığmıyor"
                            ),
                        ),
                        lstr!(en: "use a smaller value"; tr: "daha küçük bir değer kullanın"),
                    ));
                    Error
                }
            },
            (UnOp::BitNot, Int(a)) => Int(!a),
            (UnOp::Not, Bool(b)) => Bool(!b),
            (UnOp::Not, Int(_)) => {
                self.type_mismatch(span, "bool");
                Error
            }
            _ => {
                self.type_mismatch(span, &lstr!(en: "numeric"; tr: "sayısal"));
                Error
            }
        }
    }

    // ═══ Yerleşikler (§6) ═════════════════════════════════════════

    fn eval_builtin_call(
        &mut self,
        callee: Idx<Expr>,
        args: &[Idx<Expr>],
        span: Span,
    ) -> ConstValue {
        let Some(&def) = self.res.resolutions.get(&callee) else {
            self.error_not_constant(span, &lstr!(en: "this call"; tr: "bu çağrı"));
            return ConstValue::Error;
        };
        let DefKind::Builtin(builtin) = self.res.def_kind(def) else {
            self.error_not_constant(span, &lstr!(en: "function call"; tr: "fonksiyon çağrısı"));
            return ConstValue::Error;
        };

        match builtin {
            // clog2(n) — n'i temsil etmek için gereken bit sayısı.
            BuiltinKind::Clog2 => {
                if args.len() != 1 {
                    self.diagnostics.push(Diagnostic::error(
                        ErrorCode::E2021,
                        lstr!(
                            en: "clog2 expects one argument, {} given", args.len();
                            tr: "clog2 tek argüman bekler, {} verildi", args.len()
                        ),
                        LabeledSpan::primary(
                            span,
                            lstr!(en: "wrong number of arguments"; tr: "yanlış argüman sayısı"),
                        ),
                        lstr!(en: "call it as clog2(N)"; tr: "clog2(N) biçiminde çağırın"),
                    ));
                    return ConstValue::Error;
                }
                let ConstValue::Int(n) = self.const_eval(args[0]) else {
                    return ConstValue::Error;
                };
                if n <= 0 {
                    self.diagnostics.push(Diagnostic::error(
                        ErrorCode::E2021,
                        lstr!(
                            en: "clog2 expects a positive value, {n} given";
                            tr: "clog2 pozitif değer bekler, {n} verildi"
                        ),
                        LabeledSpan::primary(
                            span,
                            lstr!(en: "invalid argument"; tr: "geçersiz argüman"),
                        ),
                        lstr!(
                            en: "give clog2 a constant of 1 or greater";
                            tr: "clog2'ye 1 veya daha büyük bir sabit verin"
                        ),
                    ));
                    return ConstValue::Error;
                }
                // clog2(1)=0, clog2(2)=1, clog2(3)=2, clog2(8)=3.
                let bits = 128 - ((n as u128) - 1).leading_zeros();
                ConstValue::Int(if n == 1 { 0 } else { bits as i128 })
            }

            // Cast benzeri yerleşikler: değer değişmiyor, sadece tip.
            BuiltinKind::Zext | BuiltinKind::Sext | BuiltinKind::Trunc => {
                if args.len() != 1 {
                    self.error_not_constant(span, &lstr!(en: "this call"; tr: "bu çağrı"));
                    return ConstValue::Error;
                }
                self.const_eval(args[0])
            }

            // Donanım fonksiyonları derleme zamanı değerlendirilemez.
            BuiltinKind::Sync
            | BuiltinKind::Sync3
            | BuiltinKind::PopCount
            | BuiltinKind::Concat
            | BuiltinKind::Replicate => {
                self.error_not_constant(
                    span,
                    &lstr!(en: "hardware function"; tr: "donanım fonksiyonu"),
                );
                ConstValue::Error
            }
        }
    }

    // ═══ Hata yardımcıları ════════════════════════════════════════

    fn span_of(&self, expr: Idx<Expr>) -> Span {
        self.ast.exprs[expr].span
    }

    fn error_not_constant(&mut self, span: Span, what: &str) {
        self.diagnostics.push(
            Diagnostic::error(
                ErrorCode::E2021,
                lstr!(en: "expected a constant expression"; tr: "sabit ifade bekleniyor"),
                LabeledSpan::primary(
                    span,
                    lstr!(
                        en: "{what} (runtime value)";
                        tr: "{what} (çalışma zamanı değeri)"
                    ),
                ),
                lstr!(
                    en: "use const NAME : u32 = ...; or add a generic parameter: \
                         module Foo<const W: u32>";
                    tr: "const AD : u32 = ...; kullanın veya generic parametre ekleyin: \
                         module Foo<const W: u32>"
                ),
            )
            .with_note(
                NoteKind::Reason,
                lstr!(
                    en: "type widths must be known at compile time";
                    tr: "tip genişlikleri derleme zamanında bilinmeli"
                ),
            ),
        );
    }

    fn type_mismatch(&mut self, span: Span, expected: &str) {
        self.diagnostics.push(Diagnostic::error(
            ErrorCode::E2003,
            lstr!(
                en: "type mismatch: expected {expected}";
                tr: "tip uyumsuzluğu: {expected} bekleniyor"
            ),
            LabeledSpan::primary(span, lstr!(en: "mismatched operand"; tr: "uyumsuz işlenen")),
            lstr!(en: "check the operand types"; tr: "işlenen tiplerini kontrol edin"),
        ));
    }

    fn division_by_zero(&mut self, span: Span) {
        self.diagnostics.push(Diagnostic::error(
            ErrorCode::E2023,
            lstr!(en: "division by zero"; tr: "sıfıra bölme"),
            LabeledSpan::primary(
                span,
                lstr!(en: "divisor is 0 at compile time"; tr: "bölen derleme zamanında 0"),
            ),
            lstr!(
                en: "make the divisor a nonzero constant";
                tr: "böleni sıfırdan farklı bir sabit yapın"
            ),
        ));
    }

    fn shift_overflow(&mut self, span: Span, amount: i128) {
        self.diagnostics.push(
            Diagnostic::error(
                ErrorCode::E2024,
                lstr!(
                    en: "invalid shift amount: {amount}";
                    tr: "geçersiz kaydırma miktarı: {amount}"
                ),
                LabeledSpan::primary(
                    span,
                    lstr!(
                        en: "shift must be in the range 0..128";
                        tr: "kaydırma 0..128 aralığında olmalı"
                    ),
                ),
                lstr!(
                    en: "use a smaller shift amount";
                    tr: "daha küçük bir kaydırma miktarı kullanın"
                ),
            )
            .with_note(
                NoteKind::Reason,
                lstr!(en: "the result exceeds the i128 range"; tr: "sonuç i128 aralığını aşıyor"),
            ),
        );
    }
}

fn describe_def_kind(kind: DefKind) -> String {
    match kind {
        DefKind::Port { .. } => lstr!(en: "this is a port"; tr: "bu bir port"),
        DefKind::Register => lstr!(en: "this is a register"; tr: "bu bir register"),
        DefKind::Wire => lstr!(en: "this is a wire"; tr: "bu bir wire"),
        DefKind::LocalBinding => lstr!(en: "this is a let binding"; tr: "bu bir let bağlaması"),
        DefKind::Instance => lstr!(en: "this is a module instance"; tr: "bu bir modül örneği"),
        DefKind::Module => lstr!(en: "this is a module"; tr: "bu bir modül"),
        DefKind::Function => lstr!(en: "this is a function"; tr: "bu bir fonksiyon"),
        DefKind::Builtin(_) => {
            lstr!(en: "this is a builtin function"; tr: "bu bir yerleşik fonksiyon")
        }
        DefKind::GenericParam => {
            lstr!(en: "this is a generic parameter"; tr: "bu bir generic parametre")
        }
        _ => lstr!(en: "this expression"; tr: "bu ifade"),
    }
}
