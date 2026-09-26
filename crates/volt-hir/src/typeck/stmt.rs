//! Deyim seviyesi kontrol (type-inference.md §6): modül gövdesi,
//! reg/let/wire bildirimleri, bloklar ve atamalar. Atamalar sürücü
//! tablosuna (§11.1) buradan kaydedilir; analiz `crate::drivers`'tadır.

use volt_ast::{
    Block, BlockStmt, ElseBranch, Expr, ExprKind, ExternDecl, ForStmt, Idx, IfStmt, LValue,
    LValueSuffix, LetDecl, MatchArm, MatchArmBody, ModuleDecl, PortDir, RegDecl, Stmt, StmtKind,
};
use volt_diagnostics::{lstr, ErrorCode};

use super::TypeChecker;
use crate::drivers::{Coverage, DriverKind};
use crate::resolve::{DefId, DefKind};
use crate::ty::{Ty, TypeId};

impl TypeChecker<'_, '_> {
    pub(super) fn check_module(&mut self, m: &ModuleDecl) {
        for p in &m.ports {
            let ty = self.resolve_type_ref(p.ty);
            self.record_def_type(&p.name, ty);
            // Giriş portunu üst modül sürer (ADR-0073): içeride atanırsa E4001.
            if p.direction == PortDir::In {
                if let Some(&def) = self.res.decl_spans.get(&p.name.span) {
                    let group = self.new_group();
                    self.drivers
                        .record_kind(def, p.span, group, DriverKind::ParentInput);
                }
            }
        }
        for &stmt in &m.body {
            self.check_stmt(stmt);
        }
        // Kontratlar gövdeden SONRA: register/let tipleri artık kayıtlı.
        for c in &m.contracts {
            self.check_contract(c);
        }
        let mut diags = Vec::new();
        self.drivers
            .check_undriven_outputs(m, self.res, &self.ast.generate, &mut diags);
        self.diagnostics.extend(diags);
    }

    /// Extern modül portlarını tipler (ADR-0047): domain çıkarımı saat
    /// portlarını `Ty::Clock` üzerinden tanır, K8 haritası kurulabilir.
    pub(super) fn type_extern_ports(&mut self, x: &ExternDecl) {
        for p in &x.ports {
            let ty = self.resolve_type_ref(p.ty);
            self.record_def_type(&p.name, ty);
        }
    }

    fn check_stmt(&mut self, stmt_idx: Idx<Stmt>) {
        let ast = self.ast;
        match &ast.stmts[stmt_idx].kind {
            StmtKind::Reg(r) => self.handle_reg(r),
            StmtKind::Let(l) => self.handle_let(l),
            StmtKind::Wire(w) => {
                let ty = self.resolve_type_ref(w.ty);
                self.record_def_type(&w.name, ty);
            }
            StmtKind::Instance(inst) => self.handle_instance(inst),
            StmtKind::On(on) => self.check_driver_block(on.body),
            StmtKind::Comb(block) => self.check_driver_block(*block),
            StmtKind::Assign(a) => self.check_assign(&a.lhs, a.rhs),
            StmtKind::For(f) => self.check_for(f),
            StmtKind::Expr(e) => {
                self.synth(*e);
            }
            StmtKind::Error => {}
        }
    }

    /// on/comb bloğu kendi sürücü grubunda denetlenir (§11.1).
    fn check_driver_block(&mut self, block: Idx<Block>) {
        let group = self.new_group();
        let prev = self.current_group.replace(group);
        self.check_block(block);
        self.current_group = prev;
    }

    /// §6: `reg` tipi ya bildirilir ya init'ten çıkarılır; salt literal
    /// init belirsizdir (E2012).
    fn handle_reg(&mut self, r: &RegDecl) {
        let ty = match r.ty {
            Some(t) => {
                let ty = self.resolve_type_ref(t);
                self.check(r.init, ty);
                ty
            }
            None => {
                let inferred = self.synth(r.init);
                if self.types.is_int_lit(inferred) {
                    self.error(
                        ErrorCode::E2012,
                        r.name.span,
                        lstr!(en: "cannot determine register type"; tr: "register tipi belirlenemiyor"),
                        lstr!(en: "type of literal initializer is ambiguous"; tr: "literal başlangıç tipi belirsiz"),
                        lstr!(en: "write the type as reg {} : u8 = ...", r.name.text; tr: "reg {} : u8 = ... şeklinde tip yazın", r.name.text),
                    );
                    self.types.error()
                } else {
                    // Register depolaması somut genişlik ister — esnek
                    // aritmetik sonucu doğal genişliğe sabitlenir.
                    self.types.concrete(inferred)
                }
            }
        };
        self.check_struct_reset(r, ty);
        self.record_def_type(&r.name, ty);
    }

    /// §6: `let` tipi bildirilmişse check, değilse synth; soneksiz
    /// literal i32 varsayılır (W2012).
    pub(super) fn handle_let(&mut self, l: &LetDecl) {
        let ty = match l.ty {
            Some(t) => {
                let ty = self.resolve_type_ref(t);
                self.check(l.value, ty);
                ty
            }
            None => {
                let ty = self.synth(l.value);
                if self.types.is_int_lit(ty) {
                    self.warning(
                        ErrorCode::W2012,
                        l.name.span,
                        lstr!(en: "type not specified, i32 assumed"; tr: "tip belirtilmedi, i32 varsayıldı"),
                        lstr!(en: "literal type could not be resolved from context"; tr: "literal tipi bağlamdan çözülemedi"),
                        // Ad çözüme yazılmaz: `for` açılımında bağlama
                        // yeniden adlandırılır (`t_0`) ve kullanıcının
                        // yazmadığı ad sızardı (ADR-0070 §3.3).
                        lstr!(en: "write the type explicitly after the name, e.g. let x : i32 = ..."; tr: "tipi adın ardına açıkça yazın, ör. let x : i32 = ..."),
                    );
                    self.types.intern(Ty::SInt { width: 32 })
                } else {
                    ty
                }
            }
        };
        self.record_def_type(&l.name, ty);
        // Başlangıç değeri `let`in sürücüsüdür (ADR-0073): sonradan
        // yapılan atama ikinci sürücüdür.
        if let Some(&def) = self.res.decl_spans.get(&l.name.span) {
            // fn gövdesinin `let`'i sinyal değildir (ADR-0081).
            if self.res.def_kind(def) == DefKind::LocalBinding && !self.in_fn {
                let group = self.current_group.unwrap_or_else(|| self.new_group());
                self.drivers
                    .record_kind(def, l.name.span, group, DriverKind::LetInit);
            }
        }
    }

    fn check_for(&mut self, f: &ForStmt) {
        self.synth(f.start);
        self.synth(f.end);
        // Döngü değişkeni derleme zamanı tamsayısıdır (const-eval.md §8).
        let ty = self.types.int_lit();
        self.record_def_type(&f.var, ty);
        // Blok içi döngü açılır: `arr[i]` hedefi [start, end) elemanlarını
        // sürer (ADR-0073 §3). Sınırı sabit olmayan döngü kaydedilmez.
        let var = self.res.decl_spans.get(&f.var.span).copied();
        let bounds = match (self.try_const_eval(f.start), self.try_const_eval(f.end)) {
            (Some(s), Some(e)) if s <= e => u32::try_from(s).ok().zip(u32::try_from(e).ok()),
            _ => None,
        };
        if let (Some(var), Some(bounds)) = (var, bounds) {
            self.loop_bounds.insert(var, bounds);
        }
        self.check_block(f.body);
        if let Some(var) = var {
            self.loop_bounds.remove(&var);
        }
    }

    fn check_block(&mut self, block_idx: Idx<Block>) {
        let ast = self.ast;
        let block = &ast.blocks[block_idx];
        for stmt in &block.stmts {
            self.check_block_stmt(stmt);
        }
        if let Some(tail) = block.tail {
            self.synth(tail);
        }
    }

    fn check_block_stmt(&mut self, stmt: &BlockStmt) {
        match stmt {
            BlockStmt::NonBlockAssign { lhs, rhs, .. }
            | BlockStmt::BlockAssign { lhs, rhs, .. } => self.check_assign(lhs, *rhs),
            BlockStmt::If(if_stmt) => self.check_if(if_stmt),
            BlockStmt::Match(m) => self.check_match_stmt(m),
            BlockStmt::Let(l) => self.handle_let(l),
            BlockStmt::For(f) => self.check_for(f),
            BlockStmt::Error => {}
        }
    }

    fn check_if(&mut self, if_stmt: &IfStmt) {
        let bool_ty = self.types.bool_ty();
        self.check(if_stmt.cond, bool_ty);
        self.check_block(if_stmt.then_block);
        match &if_stmt.else_branch {
            Some(ElseBranch::Block(b)) => self.check_block(*b),
            Some(ElseBranch::If(nested)) => self.check_if(nested),
            None => {}
        }
    }

    pub(super) fn check_arm(&mut self, arm: &MatchArm) {
        if let Some(guard) = arm.guard {
            let bool_ty = self.types.bool_ty();
            self.check(guard, bool_ty);
        }
        match &arm.body {
            MatchArmBody::Block(b) => self.check_block(*b),
            MatchArmBody::Expr(e) => {
                self.synth(*e);
            }
        }
    }

    // ═══ Atama ve sürücü kaydı (§6, §11) ══════════════════════════

    fn check_assign(&mut self, lhs: &LValue, rhs: Idx<Expr>) {
        let (target, lhs_ty, bits) = self.lvalue_type(lhs);
        self.check(rhs, lhs_ty);
        let Some(def) = target else { return };
        if !matches!(
            self.res.def_kind(def),
            DefKind::Port { .. } | DefKind::Register | DefKind::Wire | DefKind::LocalBinding
        ) {
            return;
        }
        let group = match self.current_group {
            Some(g) => g,
            None => self.new_group(),
        };
        let partial = !lhs.suffixes.is_empty();
        let bits = bits.filter(|_| partial);
        if partial {
            self.note_coverage(def);
        }
        match self.field_path(def, lhs) {
            Some(field) => self.drivers.record_field(def, lhs.span, group, bits, field),
            None => self.drivers.record(def, lhs.span, group, partial, bits),
        }
    }

    /// Struct hedefinin baştaki alan zinciri (`p.i.x[1]` → `i.x`).
    fn field_path(&mut self, def: DefId, lhs: &LValue) -> Option<String> {
        let ty = self.def_type(def);
        if !matches!(self.types.ty(ty), Ty::Struct(_)) {
            return None;
        }
        let fields: Vec<&str> = lhs
            .suffixes
            .iter()
            .map_while(|s| match s {
                LValueSuffix::Field(n) => Some(n.text.as_str()),
                _ => None,
            })
            .collect();
        (!fields.is_empty()).then(|| fields.join("."))
    }

    /// E4012 için kısmen sürülen sinyalin genişliği ve (struct ise)
    /// yaprakları — bir kez.
    fn note_coverage(&mut self, def: DefId) {
        if self.coverage.contains_key(&def) {
            return;
        }
        let ty = self.def_type(def);
        let leaves = match self.types.ty(ty).clone() {
            Ty::Struct(sid) => match self.struct_layout(sid) {
                Some(l) => Some(
                    l.leaves
                        .iter()
                        .map(|leaf| (leaf.dotted(), leaf.lsb, leaf.width))
                        .collect(),
                ),
                None => return,
            },
            _ => None,
        };
        let Some(width) = self.bit_width(ty) else {
            return;
        };
        self.coverage.insert(def, Coverage { width, leaves });
    }

    /// Hedef tanım, hedef tipi ve sürülen bit aralığı `[lo, hi)`
    /// (ADR-0073; derleme zamanında bilinmiyorsa `None` = bütün sinyal).
    fn lvalue_type(&mut self, lv: &LValue) -> (Option<DefId>, TypeId, Option<(u32, u32)>) {
        let def = self.res.use_spans.get(&lv.base.span).copied();
        let mut ty = match def {
            Some(d) => self.def_type(d),
            None => self.types.error(),
        };
        // Taban sinyal bit 0'dan başlar; sonekler aralığı daraltır.
        let mut bits = Some((0, u32::MAX));
        for suffix in &lv.suffixes {
            bits = bits.and_then(|(lo, _)| self.suffix_bits(ty, suffix, lo));
            ty = match suffix {
                LValueSuffix::Index(e) => {
                    self.synth(*e);
                    self.index_result(ty, *e, lv.span)
                }
                LValueSuffix::Range { hi, lo } => self.range_result(ty, *hi, *lo, lv.span),
                LValueSuffix::PartSelect {
                    start,
                    width,
                    ascending,
                } => {
                    self.synth(*start);
                    self.synth(*width);
                    self.part_select_result(ty, *start, *width, *ascending, lv.span)
                }
                LValueSuffix::Field(name) => self.field_result(ty, name, lv.span),
            };
        }
        (def, ty, bits)
    }

    /// Bir lvalue sonekinin `ty` içinde seçtiği bit aralığı; `lo` tabanın
    /// başlangıç bitidir. Sabit olmayan indeks/aralık `None` verir.
    fn suffix_bits(&mut self, ty: TypeId, suffix: &LValueSuffix, lo: u32) -> Option<(u32, u32)> {
        let konst = |this: &mut Self, e: Idx<Expr>| {
            this.try_const_eval(e).and_then(|v| u32::try_from(v).ok())
        };
        let (start, width) = match suffix {
            LValueSuffix::Index(e) => {
                // Sabit indeks tek eleman; çıplak döngü değişkeni döngü
                // aralığındaki elemanların birleşimi.
                let (first, count) = match konst(self, *e) {
                    Some(i) => (i, 1),
                    None => {
                        let (start, end) = self.loop_var_bounds(*e)?;
                        (start, end - start)
                    }
                };
                let w = match *self.types.ty(ty) {
                    Ty::Array { elem, .. } => self.bit_width(elem)?,
                    _ => 1,
                };
                (first.checked_mul(w)?, count.checked_mul(w)?)
            }
            LValueSuffix::Range { hi, lo } => {
                let (h, l) = (konst(self, *hi)?, konst(self, *lo)?);
                (l, h.checked_sub(l)?.checked_add(1)?)
            }
            LValueSuffix::PartSelect {
                start,
                width,
                ascending,
            } => {
                let (s, w) = (konst(self, *start)?, konst(self, *width)?);
                let s = if *ascending {
                    s
                } else {
                    s.checked_add(1)?.checked_sub(w)?
                };
                (s, w)
            }
            // Struct alanı ortak düzenle (ADR-0077 Karar 3: ilk alan MSB).
            LValueSuffix::Field(name) => match self.types.ty(ty).clone() {
                Ty::Struct(sid) => self
                    .struct_layout(sid)?
                    .field_bits(std::slice::from_ref(&name.text))?,
                _ => return None,
            },
        };
        let begin = lo.checked_add(start)?;
        Some((begin, begin.checked_add(width)?))
    }

    /// İfade, sınırları sabit bir blok içi döngünün değişkeniyse `[start, end)`.
    fn loop_var_bounds(&self, e: Idx<Expr>) -> Option<(u32, u32)> {
        let ExprKind::Path(path) = &self.ast.exprs[e].kind else {
            return None;
        };
        let [seg] = path.segments.as_slice() else {
            return None;
        };
        let def = self.res.use_spans.get(&seg.span)?;
        self.loop_bounds.get(def).copied()
    }

    /// Sürücü analizi için depolama genişliği (bool 1, Trit 2, dizi
    /// eleman × uzunluk, enum kodlama genişliği, struct düzen genişliği
    /// `W` — ADR-0077); düzeni bilinmeyen tipler `None`.
    fn bit_width(&mut self, ty: TypeId) -> Option<u32> {
        match self.types.ty(ty).clone() {
            Ty::Bool => Some(1),
            Ty::Trit => Some(2),
            Ty::Array { elem, len } => self.bit_width(elem)?.checked_mul(len),
            Ty::Struct(sid) => self.struct_layout(sid).map(|l| l.width),
            Ty::Enum(e) => self.types.enum_width(e).map(u32::from),
            _ => self.types.width_of(ty).map(u32::from),
        }
    }

    pub(super) fn new_group(&mut self) -> u32 {
        self.next_group += 1;
        self.next_group
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{codes, def_ty};

    #[test]
    fn register_type_comes_from_annotation_or_initializer() {
        let src = "module M {\n    in  clk : clock\n    in  a : u8\n    out y : u8\n\n    reg r : u8 = 0\n    reg q = a\n\n    on clk {\n        r <= a\n        q <= a\n    }\n    y = r\n}\n";
        assert_eq!(def_ty(src, "r"), "u8");
        assert_eq!(def_ty(src, "q"), "u8");
    }

    #[test]
    fn bare_literal_register_is_ambiguous_e2012() {
        let src = "module M {\n    in  clk : clock\n    out y : u8\n\n    reg r = 0\n\n    on clk { r <= r }\n    y = 0\n}\n";
        assert!(codes(src).contains(&"E2012"));
    }

    #[test]
    fn bare_literal_let_defaults_to_i32_with_w2012() {
        let src = "module M {\n    in  a : u8\n    out y : u8\n\n    let _x = 42\n\n    y = a\n}\n";
        assert!(codes(src).contains(&"W2012"));
        assert_eq!(def_ty(src, "_x"), "i32");
    }

    #[test]
    fn assignment_checks_rhs_against_target_and_if_condition_is_bool() {
        let narrow = "module M {\n    in  a : u16\n    out y : u8\n\n    y = a\n}\n";
        assert!(codes(narrow).contains(&"E2001"));
        let cond = "module M {\n    in  a : u8\n    out y : u8\n\n    comb {\n        if a { y = a } else { y = 0 }\n    }\n}\n";
        assert!(codes(cond).contains(&"E2003"));
    }

    #[test]
    fn assignments_in_two_blocks_are_recorded_as_separate_drivers() {
        let src = "module M {\n    in  a : u8\n    out y : u8\n\n    comb { y = a }\n    comb { y = a }\n}\n";
        assert!(codes(src).contains(&"E4001"));
    }
}
