//! Çift yönlü tip kontrolü (docs/spec/type-inference.md §2-§6) — F2a+F2b.
//!
//! `synth` (↑ "bu ifadenin tipi ne?") ve `check` (↓ "bu ifade T tipinde
//! mi?") bağlama göre seçilir: beklenen tip biliniyorsa check, değilse
//! synth. `Ty::Error` sessizce yayılır — kaskad hata üretilmez.
//!
//! F2b: ikili operatörler (§3.3). Aritmetik sonuçlar esnek genişlik
//! aralığı taşır (`Ty::UIntFlex`/`SIntFlex`, ADR-0025): `u8 + u8`
//! doğal olarak `u9`'dur ama sayaç deseni (`count <= count + 1`) taşma
//! bitini atarak operand genişliğine de uyarlanabilir. Yerleşik çağrı
//! tipleri (sync/zext/concat...) ve kontratlar sonraki fazın işidir.

use std::collections::HashMap;

use volt_ast::{
    ArrayLitKind, BinOp, Block, BlockStmt, Contract, ContractKind, ElseBranch, Expr, ExprKind, Idx,
    IfStmt, IntSuffix, ItemKind, LValue, LValueSuffix, LetDecl, MatchArm, MatchArmBody, ModuleDecl,
    Name, RegDecl, SourceFile, Stmt, StmtKind, TypeRef, TypeRefKind, UnOp,
};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use crate::consteval::{ConstEvaluator, ConstValue, MAX_ARRAY_LEN, MAX_WIDTH};
use crate::drivers::DriverTable;
use crate::resolve::{is_widened_int_type, DefId, DefKind, ResolveResult};
use crate::ty::{EnumId, ModuleId, StructId, Ty, TypeArena, TypeId};

/// Tip kontrolü çıktısı.
#[derive(Debug)]
pub struct TypeckResult {
    pub types: TypeArena,
    /// Tanım → çıkarılan/bildirilen tip.
    pub def_types: HashMap<DefId, TypeId>,
    /// İfade → tip (sonraki aşamalar ve araçlar için).
    pub expr_types: HashMap<Idx<Expr>, TypeId>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Dosyadaki modülleri ve const öğelerini tip denetiminden geçirir.
pub fn typecheck<'a>(
    ast: &'a SourceFile,
    res: &'a ResolveResult,
    ev: &mut ConstEvaluator<'a>,
) -> TypeckResult {
    let mut checker = TypeChecker {
        ast,
        res,
        ev,
        types: TypeArena::new(),
        def_types: HashMap::new(),
        expr_types: HashMap::new(),
        type_ref_cache: HashMap::new(),
        alias_stack: Vec::new(),
        diagnostics: Vec::new(),
        drivers: DriverTable::default(),
        current_group: None,
        next_group: 0,
    };
    checker.run();
    TypeckResult {
        types: checker.types,
        def_types: checker.def_types,
        expr_types: checker.expr_types,
        diagnostics: checker.diagnostics,
    }
}

struct TypeChecker<'a, 'ev> {
    ast: &'a SourceFile,
    res: &'a ResolveResult,
    /// Sınır/genişlik sabitleri için sessiz değerlendirme (bkz.
    /// `try_const_eval` — tanılar geri alınır).
    ev: &'ev mut ConstEvaluator<'a>,
    types: TypeArena,
    def_types: HashMap<DefId, TypeId>,
    expr_types: HashMap<Idx<Expr>, TypeId>,
    type_ref_cache: HashMap<Idx<TypeRef>, TypeId>,
    /// Tip takma adı döngüsü koruması.
    alias_stack: Vec<DefId>,
    diagnostics: Vec<Diagnostic>,
    drivers: DriverTable,
    /// İçinde bulunulan on/comb bloğunun sürücü grubu.
    current_group: Option<u32>,
    next_group: u32,
}

impl<'a> TypeChecker<'a, '_> {
    fn run(&mut self) {
        let ast = self.ast;
        for &item_idx in &ast.items {
            if let ItemKind::Const(c) = &ast.items_arena[item_idx].kind {
                let ty = self.resolve_type_ref(c.ty);
                if let Some(&def) = self.res.decl_spans.get(&c.name.span) {
                    self.def_types.insert(def, ty);
                }
                self.check(c.value, ty);
            }
        }
        for &item_idx in &ast.items {
            if let ItemKind::Module(m) = &ast.items_arena[item_idx].kind {
                self.check_module(m);
            }
        }
        let mut diags = Vec::new();
        self.drivers.check_multiple_drivers(self.res, &mut diags);
        self.drivers.check_write_only(self.res, &mut diags);
        self.diagnostics.extend(diags);
    }

    // ═══ Modül ve deyimler (§6) ═══════════════════════════════════

    fn check_module(&mut self, m: &ModuleDecl) {
        for p in &m.ports {
            let ty = self.resolve_type_ref(p.ty);
            if let Some(&def) = self.res.decl_spans.get(&p.name.span) {
                self.def_types.insert(def, ty);
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
        self.drivers.check_undriven_outputs(m, self.res, &mut diags);
        self.diagnostics.extend(diags);
    }

    // ═══ Kontratlar (F4a) ═════════════════════════════════════════

    /// Her kontrat ifadesi Bool olmalı (E5004); tür bazlı kapsam:
    /// requires/ensures → port, invariant → port + register,
    /// cover/assert/assume → hepsi (ihlal E1001).
    fn check_contract(&mut self, c: &Contract) {
        let ty = self.synth(c.expr);
        if !self.types.is_error(ty) && !matches!(self.types.ty(ty), Ty::Bool) {
            let shown = self.types.display(ty);
            let span = self.ast.exprs[c.expr].span;
            self.diagnostics.push(Diagnostic::error(
                ErrorCode::E5004,
                lstr!(en: "contract expression is not Bool"; tr: "kontrat ifadesi Bool değil"),
                LabeledSpan::primary(
                    span,
                    lstr!(en: "this expression has type '{shown}'"; tr: "bu ifadenin tipi '{shown}'"),
                ),
                lstr!(
                    en: "write a condition such as a comparison (x <= 2) or a bool signal";
                    tr: "karşılaştırma (x <= 2) ya da bool sinyal gibi bir koşul yazın"
                ),
            ));
        }
        self.check_contract_scope(c);
    }

    fn check_contract_scope(&mut self, c: &Contract) {
        if matches!(
            c.kind,
            ContractKind::Cover | ContractKind::Assert | ContractKind::Assume
        ) {
            return; // her sinyale erişebilir
        }
        let mut paths = Vec::new();
        collect_path_exprs(self.ast, c.expr, &mut paths);
        for p in paths {
            // Çözülemeyen isim E1001'i isim çözümlemede zaten aldı.
            let Some(&def) = self.res.resolutions.get(&p) else {
                continue;
            };
            let out_of_scope = match self.res.def_kind(def) {
                DefKind::Register => c.kind != ContractKind::Invariant,
                DefKind::Wire | DefKind::LocalBinding | DefKind::Instance => true,
                // Port, const, enum varyantı vb. her kontratta serbest.
                _ => false,
            };
            if !out_of_scope {
                continue;
            }
            let kw = contract_keyword(c.kind);
            let name = match &self.ast.exprs[p].kind {
                ExprKind::Path(path) => path
                    .segments
                    .last()
                    .map(|n| n.text.clone())
                    .unwrap_or_default(),
                _ => String::new(),
            };
            let span = self.ast.exprs[p].span;
            let help = match c.kind {
                ContractKind::Invariant => lstr!(
                    en: "'invariant' may only reference module ports and registers";
                    tr: "'invariant' yalnız modül portlarına ve register'lara erişebilir"
                ),
                _ => lstr!(
                    en: "'requires' and 'ensures' may only reference module ports";
                    tr: "'requires' ve 'ensures' yalnız modül portlarına erişebilir"
                ),
            };
            self.diagnostics.push(Diagnostic::error(
                ErrorCode::E1001,
                lstr!(
                    en: "'{name}' cannot be referenced in a '{kw}' contract";
                    tr: "'{name}' bir '{kw}' kontratında kullanılamaz"
                ),
                LabeledSpan::primary(
                    span,
                    lstr!(
                        en: "out of scope for this contract kind";
                        tr: "bu kontrat türünün kapsamı dışında"
                    ),
                ),
                help,
            ));
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
            StmtKind::On(on) => {
                let group = self.new_group();
                let prev = self.current_group.replace(group);
                self.check_block(on.body);
                self.current_group = prev;
            }
            StmtKind::Comb(block) => {
                let group = self.new_group();
                let prev = self.current_group.replace(group);
                self.check_block(*block);
                self.current_group = prev;
            }
            StmtKind::Assign(a) => self.check_assign(&a.lhs, a.rhs),
            StmtKind::For(f) => self.check_for(f),
            StmtKind::Expr(e) => {
                self.synth(*e);
            }
            StmtKind::Error => {}
        }
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
                    self.diagnostics.push(Diagnostic::error(
                        ErrorCode::E2012,
                        lstr!(en: "cannot determine register type"; tr: "register tipi belirlenemiyor"),
                        LabeledSpan::primary(
                            r.name.span,
                            lstr!(en: "type of literal initializer is ambiguous"; tr: "literal başlangıç tipi belirsiz"),
                        ),
                        lstr!(en: "write the type as reg {} : u8 = ...", r.name.text; tr: "reg {} : u8 = ... şeklinde tip yazın", r.name.text),
                    ));
                    self.types.error()
                } else {
                    // Register depolaması somut genişlik ister — esnek
                    // aritmetik sonucu doğal genişliğe sabitlenir.
                    self.types.concrete(inferred)
                }
            }
        };
        self.record_def_type(&r.name, ty);
    }

    /// §6: `let` tipi bildirilmişse check, değilse synth; soneksiz
    /// literal i32 varsayılır (W2012).
    fn handle_let(&mut self, l: &LetDecl) {
        let ty = match l.ty {
            Some(t) => {
                let ty = self.resolve_type_ref(t);
                self.check(l.value, ty);
                ty
            }
            None => {
                let ty = self.synth(l.value);
                if self.types.is_int_lit(ty) {
                    self.diagnostics.push(Diagnostic::warning(
                        ErrorCode::W2012,
                        lstr!(en: "type not specified, i32 assumed"; tr: "tip belirtilmedi, i32 varsayıldı"),
                        LabeledSpan::primary(
                            l.name.span,
                            lstr!(en: "literal type could not be resolved from context"; tr: "literal tipi bağlamdan çözülemedi"),
                        ),
                        lstr!(en: "make it explicit by writing let {} : i32 = ...", l.name.text; tr: "let {} : i32 = ... yazarak açık belirtin", l.name.text),
                    ));
                    self.types.intern(Ty::SInt { width: 32 })
                } else {
                    ty
                }
            }
        };
        self.record_def_type(&l.name, ty);
    }

    fn handle_instance(&mut self, inst: &volt_ast::InstanceDecl) {
        let def = self.res.decl_spans.get(&inst.name.span).copied();
        if let Some(prim) = def.and_then(|d| self.res.instance_builtin.get(&d).copied()) {
            self.handle_builtin_instance(inst, def, prim);
            return;
        }
        let target = def.and_then(|d| self.res.instance_module.get(&d).copied());
        let ty = match target {
            Some(module) => self.types.intern(Ty::Instance(ModuleId(module.0))),
            None => self.types.error(),
        };
        if let Some(def) = def {
            self.def_types.insert(def, ty);
        }
        for b in &inst.bindings {
            let Some(value) = b.value else { continue };
            match target.and_then(|t| self.port_type_of(t, &b.port_name.text)) {
                Some(port_ty) => self.check(value, port_ty),
                None => {
                    self.synth(value);
                }
            }
        }
    }

    /// Yerleşik stdlib primitifi örneklemesi (ADR-0027/0029): generic
    /// argüman sayısı/biçimi, sabit argüman kısıtı (E2025) ve giriş
    /// bağlama tipleri.
    fn handle_builtin_instance(
        &mut self,
        inst: &volt_ast::InstanceDecl,
        def: Option<DefId>,
        prim: crate::builtin::BuiltinPrim,
    ) {
        let (data, dim) = self.builtin_generic_args(inst, prim);
        let ty = self.types.intern(Ty::Builtin { prim, data, dim });
        if let Some(def) = def {
            self.def_types.insert(def, ty);
        }

        // Giriş bağlamaları port tablosundaki beklenen tiple denetlenir;
        // bilinmeyen/çıkış portu E1009'u isim çözümlemede aldı.
        for b in &inst.bindings {
            let Some(value) = b.value else { continue };
            match prim.port(&b.port_name.text) {
                Some(port) => {
                    let expected = self.builtin_port_type(port.kind, data, dim);
                    self.check(value, expected);
                }
                None => {
                    self.synth(value);
                }
            }
        }
    }

    /// Port türünden beklenen/okunan tip: `Data` → T, `Addr` →
    /// `u(clog2(DEPTH))`, `Dim` → `bits<DIM>`, `Taps` →
    /// `bits<LEN * width(T)>` (ADR-0029).
    fn builtin_port_type(
        &mut self,
        kind: crate::builtin::PortKind,
        data: TypeId,
        dim: u32,
    ) -> TypeId {
        use crate::builtin::PortKind;
        match kind {
            PortKind::Clock => self.types.intern(Ty::Clock),
            PortKind::Bool => self.types.bool_ty(),
            PortKind::Data => data,
            PortKind::Addr => {
                if dim < 2 {
                    return self.types.error(); // E2025/E2003 zaten üretildi
                }
                // `uN` yalnız 8/16/32/64 için var; adres her clog2(DEPTH)
                // genişliğinde ifade edilebilsin diye ham vektördür.
                self.types.intern(Ty::Bits {
                    width: dim.trailing_zeros() as u16,
                })
            }
            PortKind::Dim => {
                if dim == 0 {
                    return self.types.error();
                }
                self.types.intern(Ty::Bits { width: dim as u16 })
            }
            PortKind::Taps => {
                if self.types.is_error(data) || dim == 0 {
                    return self.types.error();
                }
                // Bool `width_of`'ta None döner ama 1 bit taşır.
                let w = self.types.width_of(data).unwrap_or(1) as u32;
                self.types.intern(Ty::Bits {
                    width: (dim * w) as u16,
                })
            }
        }
    }

    /// Generic argümanları çözer: `T` veri tipi + sabit boyut
    /// (DEPTH/WIDTH/LEN/N). Yanlış sayı E2003, literal olmayan sabit
    /// E2008, kural dışı sabit E2025 üretir. Dönüş: (T tipi, sabit).
    fn builtin_generic_args(
        &mut self,
        inst: &volt_ast::InstanceDecl,
        prim: crate::builtin::BuiltinPrim,
    ) -> (TypeId, u32) {
        use volt_ast::GenericArg;

        let want = prim.type_arg_count() + prim.const_arg_count();
        if inst.generic_args.len() != want {
            self.err_builtin_arity(inst, prim);
            return (self.types.error(), 0);
        }

        // T pozisyonel olarak ilk argümandır.
        let data = if prim.type_arg_count() == 1 {
            match &inst.generic_args[0] {
                GenericArg::Type(t) => self.resolve_type_ref(*t),
                GenericArg::Const(_) => {
                    self.err_builtin_arity(inst, prim);
                    return (self.types.error(), 0);
                }
            }
        } else {
            self.types.bool_ty() // veri portu olmayan primitifler
        };

        // Sabit argüman (T'den sonra gelir): tam sayı literali olmalı ve
        // primitifin kuralına uymalı (iki kuvveti / aralık). Parser
        // çıplak bir ismi tip sayar; sabit ADIYLA verilen değer de
        // E2008'e düşer (literal zorunlu).
        let mut dim = 0u32;
        if prim.const_arg_count() == 1 {
            let rule = prim.const_rule().expect("const_arg_count == 1");
            match &inst.generic_args[prim.type_arg_count()] {
                GenericArg::Const(e) => {
                    let span = self.ast.exprs[*e].span;
                    match self.ast.exprs[*e].kind {
                        volt_ast::ExprKind::IntLit { value, .. } => {
                            if rule.allows(value) {
                                dim = value as u32;
                            } else {
                                self.err_builtin_const_rule(prim, value, span);
                            }
                        }
                        _ => self.err_builtin_const_not_literal(prim, span),
                    }
                }
                GenericArg::Type(t) => {
                    let span = self.ast.types[*t].span;
                    self.err_builtin_const_not_literal(prim, span);
                }
            }
        }
        (data, dim)
    }

    /// E2025 — sabit generic argüman primitifin kuralına uymuyor.
    fn err_builtin_const_rule(
        &mut self,
        prim: crate::builtin::BuiltinPrim,
        value: u128,
        span: Span,
    ) {
        use crate::builtin::ConstRule;
        let (name, param) = (prim.name(), prim.const_param_name());
        let msg = match prim.const_rule() {
            Some(ConstRule::PowerOfTwo { .. }) => {
                lstr!(en: "{name} {param} must be a power of two, got {value}";
                      tr: "{name} {param} iki kuvveti olmalı, {value} verildi")
            }
            _ => lstr!(en: "{name} {param} is out of range, got {value}";
                       tr: "{name} {param} aralık dışı, {value} verildi"),
        };
        self.diagnostics.push(Diagnostic::error(
            ErrorCode::E2025,
            msg,
            LabeledSpan::primary(
                span,
                lstr!(en: "invalid size parameter"; tr: "geçersiz boyut parametresi"),
            ),
            lstr!(en: "{}", prim.const_rule_hint_en(); tr: "{}", prim.const_rule_hint_tr()),
        ));
    }

    /// E2003 — yerleşik primitifte yanlış generic argüman sayısı/biçimi.
    fn err_builtin_arity(
        &mut self,
        inst: &volt_ast::InstanceDecl,
        prim: crate::builtin::BuiltinPrim,
    ) {
        let shape = prim.generic_shape();
        self.diagnostics.push(Diagnostic::error(
            ErrorCode::E2003,
            lstr!(en: "'{}' expects {} type and {} const generic argument(s), got {}",
                      prim.name(), prim.type_arg_count(), prim.const_arg_count(),
                      inst.generic_args.len();
                  tr: "'{}' {} tip ve {} sabit generic argüman bekler, {} verildi",
                      prim.name(), prim.type_arg_count(), prim.const_arg_count(),
                      inst.generic_args.len()),
            LabeledSpan::primary(
                inst.name.span,
                lstr!(en: "wrong generic argument count"; tr: "yanlış generic argüman sayısı"),
            ),
            lstr!(en: "write it as {shape} {{ ... }}"; tr: "{shape} {{ ... }} biçiminde yazın"),
        ));
    }

    /// E2008 — sabit generic argüman literal değil (sabit ismi ya da ifade).
    fn err_builtin_const_not_literal(&mut self, prim: crate::builtin::BuiltinPrim, span: Span) {
        let (name, param) = (prim.name(), prim.const_param_name());
        self.diagnostics.push(Diagnostic::error(
            ErrorCode::E2008,
            lstr!(en: "{name} {param} must be a compile-time integer literal";
                  tr: "{name} {param} derleme zamanı tam sayı literali olmalı"),
            LabeledSpan::primary(
                span,
                lstr!(en: "not an integer literal"; tr: "tam sayı literali değil"),
            ),
            lstr!(en: "write the size directly, e.g. {}", prim.generic_shape();
                  tr: "boyutu doğrudan yazın, ör. {}", prim.generic_shape()),
        ));
    }

    fn check_for(&mut self, f: &volt_ast::ForStmt) {
        self.synth(f.start);
        self.synth(f.end);
        // Döngü değişkeni derleme zamanı tamsayısıdır (const-eval.md §8).
        let ty = self.types.int_lit();
        self.record_def_type(&f.var, ty);
        self.check_block(f.body);
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
            BlockStmt::Match(m) => {
                self.synth(m.scrutinee);
                for arm in &m.arms {
                    self.check_arm(arm);
                }
            }
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

    fn check_arm(&mut self, arm: &MatchArm) {
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
        let (target, lhs_ty) = self.lvalue_type(lhs);
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
        self.drivers
            .record(def, lhs.span, group, !lhs.suffixes.is_empty());
    }

    fn lvalue_type(&mut self, lv: &LValue) -> (Option<DefId>, TypeId) {
        let def = self.res.use_spans.get(&lv.base.span).copied();
        let mut ty = match def {
            Some(d) => self.def_type(d),
            None => self.types.error(),
        };
        for suffix in &lv.suffixes {
            ty = match suffix {
                LValueSuffix::Index(e) => {
                    self.synth(*e);
                    self.index_result(ty, *e, lv.span)
                }
                LValueSuffix::Range { hi, lo } => self.range_result(ty, *hi, *lo, lv.span),
                LValueSuffix::Field(name) => self.field_result(ty, &name.clone(), lv.span),
            };
        }
        (def, ty)
    }

    fn new_group(&mut self) -> u32 {
        self.next_group += 1;
        self.next_group
    }

    // ═══ Tanım ve tip referansı çözümleme ═════════════════════════

    fn record_def_type(&mut self, name: &Name, ty: TypeId) {
        if let Some(&def) = self.res.decl_spans.get(&name.span) {
            self.def_types.insert(def, ty);
        }
    }

    fn def_type(&mut self, def: DefId) -> TypeId {
        if let Some(&ty) = self.def_types.get(&def) {
            return ty;
        }
        let ty = self.compute_def_type(def);
        self.def_types.insert(def, ty);
        ty
    }

    fn compute_def_type(&mut self, def: DefId) -> TypeId {
        let ast = self.ast;
        match self.res.def_kind(def) {
            DefKind::Const => match self.res.item_of_def.get(&def) {
                Some(&item_idx) => match &ast.items_arena[item_idx].kind {
                    ItemKind::Const(c) => self.resolve_type_ref(c.ty),
                    _ => self.types.error(),
                },
                None => self.types.error(),
            },
            DefKind::EnumVariant { parent } => self.types.intern(Ty::Enum(EnumId(parent.0))),
            DefKind::Instance => match self.res.instance_module.get(&def) {
                Some(target) => self.types.intern(Ty::Instance(ModuleId(target.0))),
                None => self.types.error(),
            },
            DefKind::LoopVar => self.types.int_lit(),
            // Portlar/register'lar modül gezilirken kaydedilir; buraya
            // düşen her şey F2a'da tiplenmez (fn, builtin, generic...).
            _ => self.types.error(),
        }
    }

    /// AST tip referansı → arena tipi. Genişlikler sessizce sabitlenir;
    /// geçersiz genişlik tanıları check_type_positions'ta zaten verildi.
    fn resolve_type_ref(&mut self, ty_idx: Idx<TypeRef>) -> TypeId {
        if let Some(&cached) = self.type_ref_cache.get(&ty_idx) {
            return cached;
        }
        let ty = self.resolve_type_ref_uncached(ty_idx);
        self.type_ref_cache.insert(ty_idx, ty);
        ty
    }

    fn resolve_type_ref_uncached(&mut self, ty_idx: Idx<TypeRef>) -> TypeId {
        let ast = self.ast;
        match &ast.types[ty_idx].kind {
            TypeRefKind::Bool => self.types.bool_ty(),
            TypeRefKind::Clock => self.types.intern(Ty::Clock),
            TypeRefKind::Reset(spec) => {
                let spec = spec.map(Into::into);
                self.types.intern(Ty::Reset { spec })
            }
            TypeRefKind::UInt(w) => self.types.intern(Ty::UInt {
                width: u16::from(*w),
            }),
            TypeRefKind::SInt(w) => self.types.intern(Ty::SInt {
                width: u16::from(*w),
            }),
            TypeRefKind::Trit => self.types.intern(Ty::Trit),
            TypeRefKind::Bits(e) => match self.try_const_eval(*e) {
                Some(n) if (1..=i128::from(u16::MAX)).contains(&n) => {
                    self.types.intern(Ty::Bits { width: n as u16 })
                }
                _ => self.types.error(),
            },
            TypeRefKind::Array { elem, len } => {
                let elem_ty = self.resolve_type_ref(*elem);
                match self.try_const_eval(*len) {
                    Some(n) if (0..=MAX_ARRAY_LEN as i128).contains(&n) => {
                        self.types.intern(Ty::Array {
                            elem: elem_ty,
                            len: n as u32,
                        })
                    }
                    _ => self.types.error(),
                }
            }
            TypeRefKind::Tuple(items) => {
                let tys: Vec<TypeId> = items
                    .clone()
                    .iter()
                    .map(|&t| self.resolve_type_ref(t))
                    .collect();
                self.types.intern(Ty::Tuple(tys))
            }
            TypeRefKind::Path { path, .. } => {
                if path.segments.len() == 1 {
                    if let Some(ty) = self.widened_int(&path.segments[0].text) {
                        return ty;
                    }
                }
                self.resolve_named_type(ty_idx)
            }
            TypeRefKind::Error => self.types.error(),
        }
    }

    /// `u9`, `i33` gibi genişletilmiş tam sayı aileleri (parser Path verir).
    fn widened_int(&mut self, name: &str) -> Option<TypeId> {
        if !is_widened_int_type(name) {
            return None;
        }
        let (head, digits) = name.split_at(1);
        let width: u16 = match digits.parse::<u32>() {
            Ok(w) if (1..=u32::from(u16::MAX)).contains(&w) => w as u16,
            _ => return Some(self.types.error()),
        };
        Some(match head {
            "u" => self.types.intern(Ty::UInt { width }),
            _ => self.types.intern(Ty::SInt { width }),
        })
    }

    fn resolve_named_type(&mut self, ty_idx: Idx<TypeRef>) -> TypeId {
        let ast = self.ast;
        let Some(&def) = self.res.type_resolutions.get(&ty_idx) else {
            return self.types.error();
        };
        match self.res.def_kind(def) {
            DefKind::Struct => self.types.intern(Ty::Struct(StructId(def.0))),
            DefKind::Enum => self.types.intern(Ty::Enum(EnumId(def.0))),
            DefKind::TypeAlias => {
                if self.alias_stack.contains(&def) {
                    return self.types.error();
                }
                let Some(&item_idx) = self.res.item_of_def.get(&def) else {
                    return self.types.error();
                };
                let ItemKind::TypeAlias(alias) = &ast.items_arena[item_idx].kind else {
                    return self.types.error();
                };
                self.alias_stack.push(def);
                let ty = self.resolve_type_ref(alias.target);
                self.alias_stack.pop();
                ty
            }
            _ => self.types.error(),
        }
    }

    // ═══ Sentez (§3) ══════════════════════════════════════════════

    fn synth(&mut self, expr: Idx<Expr>) -> TypeId {
        let ty = self.synth_uncached(expr);
        self.expr_types.insert(expr, ty);
        ty
    }

    fn synth_uncached(&mut self, expr: Idx<Expr>) -> TypeId {
        let ast = self.ast;
        let span = ast.exprs[expr].span;
        match &ast.exprs[expr].kind {
            ExprKind::IntLit { value, suffix, .. } => match suffix {
                Some(s) => {
                    let ty = self.suffix_ty(*s);
                    self.check_literal_fits(*value, ty, span);
                    ty
                }
                None => self.types.int_lit(),
            },
            ExprKind::BoolLit(_) => self.types.bool_ty(),
            ExprKind::Path(_) => match self.res.resolutions.get(&expr) {
                Some(&def) => self.def_type(def),
                // Çözülemeyen isim E1001'i zaten aldı.
                None => self.types.error(),
            },
            ExprKind::Binary { op, lhs, rhs } => self.synth_binary(*op, *lhs, *rhs, span),
            ExprKind::Unary { op, operand } => self.synth_unary(*op, *operand, span),
            ExprKind::Index { base, index } => {
                let base_ty = self.synth(*base);
                self.synth(*index);
                self.index_result(base_ty, *index, span)
            }
            ExprKind::Range { base, hi, lo } => {
                let base_ty = self.synth(*base);
                self.range_result(base_ty, *hi, *lo, span)
            }
            ExprKind::Field { base, field } => {
                let (base, field) = (*base, field.clone());
                let base_ty = self.synth(base);
                self.field_result(base_ty, &field, span)
            }
            ExprKind::Call { args, .. } => {
                // Yerleşik çağrı tipleri F2b (sync/zext/concat...).
                for &a in args.clone().iter() {
                    self.synth(a);
                }
                self.types.error()
            }
            ExprKind::Cast { expr: inner, ty } => {
                let src = self.synth(*inner);
                let dst = self.resolve_type_ref(*ty);
                self.check_cast_legal(src, dst, span);
                dst
            }
            ExprKind::If {
                cond,
                then_expr,
                else_expr,
            } => self.synth_if(*cond, *then_expr, *else_expr, span),
            ExprKind::Match { scrutinee, arms } => {
                self.synth(*scrutinee);
                for arm in arms {
                    self.check_arm(arm);
                }
                self.types.error()
            }
            ExprKind::StructLit { fields, .. } => {
                let fields: Vec<_> = fields
                    .iter()
                    .map(|f| (f.name.text.clone(), f.value))
                    .collect();
                self.synth_struct_lit(expr, &fields)
            }
            ExprKind::ArrayLit(ArrayLitKind::List(items)) => {
                let items = items.clone();
                let Some((&first, rest)) = items.split_first() else {
                    return self.types.error();
                };
                let elem = self.synth(first);
                for &i in rest {
                    self.check(i, elem);
                }
                self.types.intern(Ty::Array {
                    elem,
                    len: items.len() as u32,
                })
            }
            ExprKind::ArrayLit(ArrayLitKind::Repeat { value, count }) => {
                let (value, count) = (*value, *count);
                let elem = self.synth(value);
                match self.try_const_eval(count) {
                    Some(n) if (0..=MAX_ARRAY_LEN as i128).contains(&n) => {
                        self.types.intern(Ty::Array {
                            elem,
                            len: n as u32,
                        })
                    }
                    _ => self.types.error(),
                }
            }
            ExprKind::TupleLit(items) => {
                let tys: Vec<TypeId> = items.clone().iter().map(|&i| self.synth(i)).collect();
                self.types.intern(Ty::Tuple(tys))
            }
            // todo! her tiple uyumludur; string F2a'da tiplenmez.
            ExprKind::Todo { .. } | ExprKind::StringLit(_) | ExprKind::Error => self.types.error(),
        }
    }

    /// §3.4 — tekli operatörler.
    fn synth_unary(&mut self, op: UnOp, operand: Idx<Expr>, span: Span) -> TypeId {
        let ot = self.synth(operand);
        match op {
            UnOp::Not => {
                self.check_is_bool(ot, span);
                self.types.bool_ty()
            }
            // Bit tersleme genişliği korur.
            UnOp::BitNot => ot,
            UnOp::Neg => match *self.types.ty(ot) {
                Ty::SInt { width } | Ty::SIntFlex { hi: width, .. } => {
                    self.types.intern(Ty::SInt {
                        width: width.saturating_add(1),
                    })
                }
                Ty::Trit => self.types.intern(Ty::Trit),
                Ty::IntLit => self.types.int_lit(),
                Ty::UInt { .. } | Ty::UIntFlex { .. } => {
                    self.diagnostics.push(Diagnostic::error(
                        ErrorCode::E2002,
                        lstr!(en: "cannot negate an unsigned value"; tr: "işaretsiz değer negatiflenemez"),
                        LabeledSpan::primary(span, lstr!(en: "signed type required"; tr: "işaretli tip gerekli")),
                        lstr!(en: "convert to a signed type such as i8/i16 first"; tr: "önce i8/i16 gibi işaretli tipe dönüştürün"),
                    ));
                    self.types.error()
                }
                _ => self.types.error(),
            },
        }
    }

    // ═══ İkili operatörler (§3.3) ═════════════════════════════════

    fn synth_binary(&mut self, op: BinOp, lhs: Idx<Expr>, rhs: Idx<Expr>, span: Span) -> TypeId {
        use BinOp::{
            Add, And, BitAnd, BitOr, BitXor, Div, Eq, Ge, Gt, Imp, Le, Lt, Mul, Ne, Or, Rem, Shl,
            Shr, Sub,
        };
        match op {
            Add | Sub | Mul | Div | Rem => self.synth_arith(op, lhs, rhs, span),
            BitAnd | BitOr | BitXor => self.synth_bitwise(lhs, rhs, span),
            Shl | Shr => self.synth_shift(lhs, rhs, span),
            Eq | Ne | Lt | Gt | Le | Ge => self.synth_comparison(lhs, rhs, span),
            // a -> b ≡ !a || b: iki operand da Bool, sonuç Bool (ADR-0034).
            And | Or | Imp => self.synth_logical(lhs, rhs),
        }
    }

    /// Aritmetik (§3.3): taşma genişlemesi. Sonuç, operand genişliği ile
    /// genişlemiş doğal genişlik arasında esnektir (ADR-0025); sayaç
    /// deseni (`count <= count + 1`) böylece taşma bitini atabilir.
    fn synth_arith(&mut self, op: BinOp, lhs: Idx<Expr>, rhs: Idx<Expr>, span: Span) -> TypeId {
        let lt = self.synth(lhs);
        let rt = self.synth(rhs);
        if self.types.is_error(lt) || self.types.is_error(rt) {
            return self.types.error();
        }
        // bits<N> aritmetiği her kombinasyonda yasak — literalden önce.
        if self.is_bits(lt) || self.is_bits(rt) {
            self.diagnostics.push(Diagnostic::error(
                ErrorCode::E2004,
                lstr!(en: "cannot perform arithmetic on bits<N>"; tr: "bits<N> tipinde aritmetik yapılamaz"),
                LabeledSpan::primary(
                    span,
                    lstr!(en: "bits is a raw bit vector, not a number"; tr: "bits ham bit vektörüdür, sayısal değil"),
                ),
                lstr!(en: "convert to a numeric type such as u8/i8"; tr: "u8/i8 gibi sayısal tipe dönüştürün"),
            ));
            return self.types.error();
        }
        match (self.types.is_int_lit(lt), self.types.is_int_lit(rt)) {
            (true, true) => return self.types.int_lit(),
            // Literal somut tarafa uyarlanır (sınır denetimiyle).
            (true, false) => {
                if !self.adapt_literal_operand(lhs, rt) {
                    return self.err_arith_incompatible(lt, rt, span);
                }
                return self.arith_result(op, rt, rt, span);
            }
            (false, true) => {
                if !self.adapt_literal_operand(rhs, lt) {
                    return self.err_arith_incompatible(lt, rt, span);
                }
                return self.arith_result(op, lt, lt, span);
            }
            (false, false) => {}
        }
        self.arith_result(op, lt, rt, span)
    }

    fn arith_result(&mut self, op: BinOp, lt: TypeId, rt: TypeId, span: Span) -> TypeId {
        if let (Some((ls, llo, lhi)), Some((rs, rlo, rhi))) =
            (self.types.int_range(lt), self.types.int_range(rt))
        {
            if ls != rs {
                self.err_sign_mismatch(span);
                return self.types.error();
            }
            let lo = llo.max(rlo);
            let hi = lhi.min(rhi);
            if lo > hi {
                self.operand_width_mismatch(lhi, rhi, if ls { "i" } else { "u" }, span);
                return self.types.error();
            }
            // Toplama/çıkarma 1 bit, çarpma genişlik kadar genişler;
            // bölme/mod genişlemez. Sonuç MAX_WIDTH ile sınırlı.
            let natural = match op {
                BinOp::Add | BinOp::Sub => clamp_width(u32::from(hi) + 1),
                BinOp::Mul => clamp_width(u32::from(hi) * 2),
                _ => hi,
            };
            return if ls {
                self.types.sint_flex(lo, natural)
            } else {
                self.types.uint_flex(lo, natural)
            };
        }
        self.trit_arith_result(op, lt, rt, span)
    }

    /// Trit kuralları (§3.3): çarpım kapalı ({-1,0,1} içinde kalır),
    /// toplam/fark i3'e taşar, ternary MAC deseninde işaretli genişlik
    /// korunur; kalan kombinasyonlar E2003.
    fn trit_arith_result(&mut self, op: BinOp, lt: TypeId, rt: TypeId, span: Span) -> TypeId {
        let l_trit = matches!(self.types.ty(lt), Ty::Trit);
        let r_trit = matches!(self.types.ty(rt), Ty::Trit);
        match (l_trit, r_trit) {
            (true, true) => match op {
                BinOp::Mul => self.types.intern(Ty::Trit),
                // +1 + +1 = +2 kümeden çıkar → i3'e genişle.
                BinOp::Add | BinOp::Sub => self.types.intern(Ty::SInt { width: 3 }),
                _ => {
                    self.err_type_mismatch_msg(
                        span,
                        &lstr!(en: "this operator is not defined for Trit"; tr: "bu operatör Trit tipinde tanımlı değil"),
                        &lstr!(en: "Trit only supports *, + and -"; tr: "Trit yalnız *, + ve - destekler"),
                    );
                    self.types.error()
                }
            },
            (true, false) | (false, true) => {
                let other = if l_trit { rt } else { lt };
                if op == BinOp::Mul {
                    if let Some((true, _, _)) = self.types.int_range(other) {
                        return other;
                    }
                }
                let shown = self.types.display(other);
                self.err_type_mismatch_msg(
                    span,
                    &lstr!(en: "this operation is not defined between Trit and '{shown}'"; tr: "Trit ile '{shown}' arasında bu işlem tanımlı değil"),
                    &lstr!(en: "Trit can only be multiplied with a signed type (iN); convert with as if needed"; tr: "Trit yalnız işaretli tiple (iN) çarpılabilir; gerekirse as ile dönüştürün"),
                );
                self.types.error()
            }
            (false, false) => self.err_arith_incompatible(lt, rt, span),
        }
    }

    /// Bit düzeyi (§3.3): genişlemez; aynı genişlik zorunlu.
    fn synth_bitwise(&mut self, lhs: Idx<Expr>, rhs: Idx<Expr>, span: Span) -> TypeId {
        let lt = self.synth(lhs);
        let rt = self.synth(rhs);
        if self.types.is_error(lt) || self.types.is_error(rt) {
            return self.types.error();
        }
        match (self.types.is_int_lit(lt), self.types.is_int_lit(rt)) {
            (true, true) => return self.types.int_lit(),
            (true, false) => {
                return if self.types.int_range(rt).is_some() {
                    self.check(lhs, rt);
                    rt
                } else {
                    self.err_bitwise_incompatible(lt, rt, span)
                };
            }
            (false, true) => {
                return if self.types.int_range(lt).is_some() {
                    self.check(rhs, lt);
                    lt
                } else {
                    self.err_bitwise_incompatible(lt, rt, span)
                };
            }
            (false, false) => {}
        }
        match (self.types.ty(lt), self.types.ty(rt)) {
            (Ty::Bool, Ty::Bool) => self.types.bool_ty(),
            (&Ty::Bits { width: a }, &Ty::Bits { width: b }) => {
                if a == b {
                    lt
                } else {
                    self.diagnostics.push(Diagnostic::error(
                        ErrorCode::E2001,
                        lstr!(en: "bit width mismatch: bits<{a}> and bits<{b}>"; tr: "bit genişliği uyumsuzluğu: bits<{a}> ve bits<{b}>"),
                        LabeledSpan::primary(span, lstr!(en: "operand widths differ"; tr: "operand genişlikleri farklı")),
                        lstr!(en: "make the operand widths equal"; tr: "operand genişliklerini eşitleyin"),
                    ));
                    self.types.error()
                }
            }
            _ => {
                if let (Some((ls, llo, lhi)), Some((rs, rlo, rhi))) =
                    (self.types.int_range(lt), self.types.int_range(rt))
                {
                    if ls != rs {
                        self.err_sign_mismatch(span);
                        return self.types.error();
                    }
                    let lo = llo.max(rlo);
                    let hi = lhi.min(rhi);
                    if lo > hi {
                        self.operand_width_mismatch(lhi, rhi, if ls { "i" } else { "u" }, span);
                        return self.types.error();
                    }
                    // GENİŞLEMEZ: ortak aralık aynen korunur.
                    return if ls {
                        self.types.sint_flex(lo, hi)
                    } else {
                        self.types.uint_flex(lo, hi)
                    };
                }
                self.err_bitwise_incompatible(lt, rt, span)
            }
        }
    }

    /// Kaydırma (§3.3): sonuç sol operandın tipi, genişlemez. Sabit
    /// miktar sol genişliğe eşit ya da büyükse W2013.
    fn synth_shift(&mut self, lhs: Idx<Expr>, rhs: Idx<Expr>, span: Span) -> TypeId {
        let lt = self.synth(lhs);
        let rt = self.synth(rhs);
        if self.types.is_error(lt) {
            return self.types.error();
        }
        let lhs_ok = self.types.int_range(lt).is_some()
            || matches!(self.types.ty(lt), Ty::Bits { .. } | Ty::IntLit);
        if !lhs_ok {
            let shown = self.types.display(lt);
            self.err_type_mismatch_msg(
                span,
                &lstr!(en: "type '{shown}' cannot be shifted"; tr: "'{shown}' tipi kaydırılamaz"),
                &lstr!(en: "shifts are only defined for uN, iN and bits<N>"; tr: "kaydırma yalnız uN, iN ve bits<N> tiplerinde tanımlı"),
            );
            return self.types.error();
        }
        let rhs_ok = self.types.is_error(rt)
            || self.types.is_int_lit(rt)
            || self.types.int_range(rt).is_some();
        if !rhs_ok {
            let shown = self.types.display(rt);
            self.err_type_mismatch_msg(
                span,
                &lstr!(en: "shift amount must be numeric, found '{shown}'"; tr: "kaydırma miktarı sayısal olmalı, '{shown}' bulundu"),
                &lstr!(en: "provide the amount as uN/iN or as a constant"; tr: "miktarı uN/iN tipinde ya da sabit olarak verin"),
            );
            // Spec: sonuç yine sol operandın tipidir.
            return lt;
        }
        if let (Some(width), Some(amount)) = (self.types.width_of(lt), self.try_const_eval(rhs)) {
            if amount >= i128::from(width) {
                self.diagnostics.push(Diagnostic::warning(
                    ErrorCode::W2013,
                    lstr!(en: "shift amount {amount} exceeds the width of {width} bits"; tr: "kaydırma miktarı {amount}, {width} bit genişliği aşıyor"),
                    LabeledSpan::primary(
                        span,
                        lstr!(en: "all bits are shifted out, the result is always 0"; tr: "tüm bitler dışarı kayar, sonuç hep 0"),
                    ),
                    lstr!(en: "keep the amount in the range 0..{width}"; tr: "miktarı 0..{width} aralığında tutun"),
                ));
            }
        }
        lt
    }

    /// Karşılaştırma (§3.3): sonuç her zaman Bool; operandlar aynı tipe
    /// birleştirilmeli, uyumsuzluk E2003.
    fn synth_comparison(&mut self, lhs: Idx<Expr>, rhs: Idx<Expr>, span: Span) -> TypeId {
        let lt = self.synth(lhs);
        let rt = self.synth(rhs);
        self.unify_for_comparison(lhs, rhs, lt, rt, span);
        self.types.bool_ty()
    }

    fn unify_for_comparison(
        &mut self,
        lhs: Idx<Expr>,
        rhs: Idx<Expr>,
        lt: TypeId,
        rt: TypeId,
        span: Span,
    ) {
        if lt == rt || self.types.is_error(lt) || self.types.is_error(rt) {
            return;
        }
        // Literal karşı tarafın tipine uyarlanır.
        if self.types.is_int_lit(lt) && self.is_literal_adaptable(rt) {
            self.check(lhs, rt);
            return;
        }
        if self.types.is_int_lit(rt) && self.is_literal_adaptable(lt) {
            self.check(rhs, lt);
            return;
        }
        if let (Some((ls, llo, lhi)), Some((rs, rlo, rhi))) =
            (self.types.int_range(lt), self.types.int_range(rt))
        {
            if ls == rs && llo.max(rlo) <= lhi.min(rhi) {
                return;
            }
        }
        let l = self.types.display(lt);
        let r = self.types.display(rt);
        self.err_type_mismatch_msg(
            span,
            &lstr!(en: "comparison operands must have the same type: '{l}' and '{r}'"; tr: "karşılaştırma operandları aynı tipte olmalı: '{l}' ile '{r}'"),
            &lstr!(en: "convert the operands to the same type with as"; tr: "operandları as ile aynı tipe getirin"),
        );
    }

    /// Mantıksal (§3.3): iki operand da Bool, sonuç Bool.
    fn synth_logical(&mut self, lhs: Idx<Expr>, rhs: Idx<Expr>) -> TypeId {
        let bool_ty = self.types.bool_ty();
        self.check(lhs, bool_ty);
        self.check(rhs, bool_ty);
        bool_ty
    }

    // ═══ İkili operatör yardımcıları ══════════════════════════════

    fn is_bits(&self, ty: TypeId) -> bool {
        matches!(self.types.ty(ty), Ty::Bits { .. })
    }

    /// Soneksiz literal bu tipe uyarlanabilir mi? (uN/iN/esnek/Trit)
    fn is_literal_adaptable(&self, ty: TypeId) -> bool {
        self.types.int_range(ty).is_some() || matches!(self.types.ty(ty), Ty::Trit)
    }

    /// Literal operandı somut sayısal tipe uyarlar; hedef sayısal
    /// değilse false döner (çağıran uyumsuzluk hatası verir).
    fn adapt_literal_operand(&mut self, expr: Idx<Expr>, target: TypeId) -> bool {
        let ok = self.is_literal_adaptable(target);
        if ok {
            self.check(expr, target);
        }
        ok
    }

    fn err_arith_incompatible(&mut self, lt: TypeId, rt: TypeId, span: Span) -> TypeId {
        let l = self.types.display(lt);
        let r = self.types.display(rt);
        self.err_type_mismatch_msg(
            span,
            &lstr!(en: "incompatible arithmetic operands: '{l}' and '{r}'"; tr: "aritmetik operandları uyumsuz: '{l}' ile '{r}'"),
            &lstr!(en: "convert the operands to the same numeric type"; tr: "operandları aynı sayısal tipe getirin"),
        );
        self.types.error()
    }

    fn err_bitwise_incompatible(&mut self, lt: TypeId, rt: TypeId, span: Span) -> TypeId {
        let l = self.types.display(lt);
        let r = self.types.display(rt);
        self.err_type_mismatch_msg(
            span,
            &lstr!(en: "bitwise operator is not defined for '{l}' and '{r}'"; tr: "bit düzeyi operatör '{l}' ile '{r}' tipinde tanımlı değil"),
            &lstr!(en: "bitwise operations require bool, uN, iN or bits<N>"; tr: "bit düzeyi işlemler bool, uN, iN ve bits<N> ister"),
        );
        self.types.error()
    }

    fn err_sign_mismatch(&mut self, span: Span) {
        self.diagnostics.push(Diagnostic::error(
            ErrorCode::E2002,
            lstr!(en: "cannot mix signed and unsigned"; tr: "işaretli ve işaretsiz karıştırılamaz"),
            LabeledSpan::primary(span, lstr!(en: "signs differ"; tr: "işaretler farklı")),
            lstr!(en: "use an explicit cast with as"; tr: "as ile açık dönüşüm yapın"),
        ));
    }

    /// E2001 — operand genişlikleri örtük birleştirilemez (§3.3, §8).
    fn operand_width_mismatch(&mut self, a: u16, b: u16, prefix: &str, span: Span) {
        self.diagnostics.push(
            Diagnostic::error(
                ErrorCode::E2001,
                lstr!(en: "bit width mismatch: {prefix}{a} and {prefix}{b}"; tr: "bit genişliği uyumsuzluğu: {prefix}{a} ve {prefix}{b}"),
                LabeledSpan::primary(span, lstr!(en: "operand widths differ"; tr: "operand genişlikleri farklı")),
                lstr!(en: "widen the narrow operand: (expr) as {prefix}{}", a.max(b); tr: "dar operandı genişletin: (ifade) as {prefix}{}", a.max(b)),
            )
            .with_note(
                NoteKind::Reason,
                lstr!(en: "different widths cannot be combined implicitly; widening requires extra wires and logic in hardware"; tr: "farklı genişlikler örtük birleştirilemez; genişletme donanımda ek tel ve mantık gerektirir"),
            ),
        );
    }

    /// §3.7 — koşullu ifade; literal dallar somut dala uyarlanır, kalan
    /// dallar aynı tipte olmalı (E2003, iki tip de mesajda gösterilir).
    fn synth_if(
        &mut self,
        cond: Idx<Expr>,
        then_expr: Idx<Expr>,
        else_expr: Idx<Expr>,
        span: Span,
    ) -> TypeId {
        let bool_ty = self.types.bool_ty();
        self.check(cond, bool_ty);
        let then_ty = self.synth(then_expr);
        let else_ty = self.synth(else_expr);
        if self.types.is_error(then_ty) || self.types.is_error(else_ty) {
            return self.types.error();
        }
        match (
            self.types.is_int_lit(then_ty),
            self.types.is_int_lit(else_ty),
        ) {
            (true, true) => return then_ty,
            (true, false) => {
                self.check(then_expr, else_ty);
                return else_ty;
            }
            (false, true) => {
                self.check(else_expr, then_ty);
                return then_ty;
            }
            (false, false) => {}
        }
        if then_ty == else_ty {
            return then_ty;
        }
        // Esnek aralıklar kesişiyorsa ortak aralık dalların birleşimidir.
        if let (Some((ts, tlo, thi)), Some((es, elo, ehi))) =
            (self.types.int_range(then_ty), self.types.int_range(else_ty))
        {
            if ts == es {
                let lo = tlo.max(elo);
                let hi = thi.min(ehi);
                if lo <= hi {
                    return if ts {
                        self.types.sint_flex(lo, hi)
                    } else {
                        self.types.uint_flex(lo, hi)
                    };
                }
            }
        }
        let t = self.types.display(then_ty);
        let e = self.types.display(else_ty);
        self.err_type_mismatch_msg(
            span,
            &lstr!(en: "if/else branches have different types: '{t}' and '{e}'"; tr: "if/else dalları farklı tipte: '{t}' ile '{e}'"),
            &lstr!(en: "make the branches the same type; convert with as if needed"; tr: "dalları aynı tipe getirin; gerekirse as ile dönüştürün"),
        );
        self.types.error()
    }

    fn synth_struct_lit(
        &mut self,
        expr: Idx<Expr>,
        fields: &[(String, Option<Idx<Expr>>)],
    ) -> TypeId {
        let ast = self.ast;
        let Some(&def) = self.res.resolutions.get(&expr) else {
            return self.types.error();
        };
        if self.res.def_kind(def) != DefKind::Struct {
            return self.types.error();
        }
        if let Some(&item_idx) = self.res.item_of_def.get(&def) {
            if let ItemKind::Struct(s) = &ast.items_arena[item_idx].kind {
                for (name, value) in fields {
                    let Some(value) = value else { continue };
                    if let Some(f) = s.fields.iter().find(|f| f.name.text == *name) {
                        let ty = self.resolve_type_ref(f.ty);
                        self.check(*value, ty);
                    } else {
                        self.synth(*value);
                    }
                }
            }
        }
        self.types.intern(Ty::Struct(StructId(def.0)))
    }

    /// §3.5 — bit/dizi indeksi. Sabit indekste sınır denetimi yapılır.
    fn index_result(&mut self, base_ty: TypeId, index: Idx<Expr>, span: Span) -> TypeId {
        match *self.types.ty(base_ty) {
            Ty::Error => self.types.error(),
            Ty::Array { elem, len } => {
                if let Some(i) = self.try_const_eval(index) {
                    if i < 0 || i >= i128::from(len) {
                        self.index_out_of_bounds(i, u64::from(len), span);
                        return self.types.error();
                    }
                }
                elem
            }
            Ty::UInt { width }
            | Ty::SInt { width }
            | Ty::Bits { width }
            | Ty::UIntFlex { hi: width, .. }
            | Ty::SIntFlex { hi: width, .. } => {
                if let Some(i) = self.try_const_eval(index) {
                    if i < 0 || i >= i128::from(width) {
                        self.index_out_of_bounds(i, u64::from(width), span);
                        return self.types.error();
                    }
                }
                self.types.bool_ty()
            }
            _ => {
                self.err_type_mismatch_msg(
                    span,
                    &lstr!(en: "bit selection is only allowed on numeric, bits or array types"; tr: "bit seçimi yalnız sayısal, bits veya dizi tipinde yapılır"),
                    &lstr!(en: "convert the value to a suitable type first"; tr: "önce değeri uygun bir tipe dönüştürün"),
                );
                self.types.error()
            }
        }
    }

    /// §3.5 — aralık seçimi; sınırlar derleme zamanı sabiti olmalı.
    fn range_result(
        &mut self,
        base_ty: TypeId,
        hi: Idx<Expr>,
        lo: Idx<Expr>,
        span: Span,
    ) -> TypeId {
        if self.types.is_error(base_ty) {
            return self.types.error();
        }
        let Some(width) = self.types.width_of(base_ty) else {
            self.err_type_mismatch_msg(
                span,
                &lstr!(en: "range selection is only allowed on numeric or bits types"; tr: "aralık seçimi yalnız sayısal veya bits tipinde yapılır"),
                &lstr!(en: "convert the value to a suitable type first"; tr: "önce değeri uygun bir tipe dönüştürün"),
            );
            return self.types.error();
        };
        match (self.try_const_eval(hi), self.try_const_eval(lo)) {
            (Some(h), Some(l)) => {
                if h < l {
                    self.diagnostics.push(Diagnostic::error(
                        ErrorCode::E2007,
                        lstr!(en: "range is reversed (hi < lo)"; tr: "aralık ters (hi < lo)"),
                        LabeledSpan::primary(span, lstr!(en: "the high bit must be written first"; tr: "yüksek bit önce yazılmalı")),
                        lstr!(en: "write [{l}:{h}]"; tr: "[{l}:{h}] yazın"),
                    ));
                    return self.types.error();
                }
                if l < 0 || h >= i128::from(width) {
                    self.diagnostics.push(Diagnostic::error(
                        ErrorCode::E2006,
                        lstr!(en: "range out of bounds (width {width})"; tr: "aralık sınır dışı (genişlik {width})"),
                        LabeledSpan::primary(span, lstr!(en: "range exceeds the width of the base"; tr: "aralık taban genişliği aşıyor")),
                        lstr!(en: "highest valid bit: {}", width - 1; tr: "geçerli en yüksek bit: {}", width - 1),
                    ));
                    return self.types.error();
                }
                self.types.intern(Ty::Bits {
                    width: (h - l + 1) as u16,
                })
            }
            _ => {
                self.diagnostics.push(Diagnostic::error(
                    ErrorCode::E2008,
                    lstr!(en: "range bounds must be compile-time constants"; tr: "aralık sınırları derleme zamanı sabiti olmalı"),
                    LabeledSpan::primary(span, lstr!(en: "non-constant bound"; tr: "değişken sınır")),
                    lstr!(en: "use x[i] +: WIDTH for a variable index"; tr: "değişken indeks için x[i] +: WIDTH kullanın"),
                ));
                self.types.error()
            }
        }
    }

    /// Alan erişimi: modül örneği portu, struct alanı veya demet indeksi.
    fn field_result(&mut self, base_ty: TypeId, field: &Name, span: Span) -> TypeId {
        let ast = self.ast;
        match self.types.ty(base_ty).clone() {
            Ty::Error => self.types.error(),
            // Port bulunamazsa E1009'u isim çözümleme verdi — sessiz.
            Ty::Instance(m) => self
                .port_type_of(DefId(m.0), &field.text)
                .unwrap_or_else(|| self.types.error()),
            // Yerleşik primitif portu: tablo üzerinden tiplenir; geçersiz
            // alan E1009'u isim çözümlemede aldı — sessiz Error.
            Ty::Builtin { prim, data, dim } => match prim.port(&field.text) {
                Some(port) => self.builtin_port_type(port.kind, data, dim),
                None => self.types.error(),
            },
            Ty::Struct(s) => {
                let field_ty =
                    self.res.item_of_def.get(&DefId(s.0)).and_then(|&item_idx| {
                        match &ast.items_arena[item_idx].kind {
                            ItemKind::Struct(decl) => decl
                                .fields
                                .iter()
                                .find(|f| f.name.text == field.text)
                                .map(|f| f.ty),
                            _ => None,
                        }
                    });
                match field_ty {
                    Some(t) => self.resolve_type_ref(t),
                    None => self.types.error(),
                }
            }
            Ty::Tuple(items) => match field.text.parse::<usize>() {
                Ok(i) if i < items.len() => items[i],
                _ => {
                    self.err_type_mismatch_msg(
                        span,
                        &lstr!(en: "tuple index exceeds the number of elements"; tr: "demet indeksi eleman sayısını aşıyor"),
                        &lstr!(en: "use a valid tuple index"; tr: "geçerli bir demet indeksi kullanın"),
                    );
                    self.types.error()
                }
            },
            _ => {
                let shown = self.types.display(base_ty);
                self.err_type_mismatch_msg(
                    span,
                    &lstr!(en: "no field access on type '{shown}'"; tr: "'{shown}' tipinde alan erişimi yok"),
                    &lstr!(en: "field access is valid on structs and module instances"; tr: "alan erişimi struct ve modül örneklerinde geçerlidir"),
                );
                self.types.error()
            }
        }
    }

    /// Hedef modülün port tipi (modüller arası akış için).
    fn port_type_of(&mut self, module_def: DefId, port: &str) -> Option<TypeId> {
        let ast = self.ast;
        let &item_idx = self.res.item_of_def.get(&module_def)?;
        let ports = match &ast.items_arena[item_idx].kind {
            ItemKind::Module(m) => &m.ports,
            ItemKind::Extern(x) => &x.ports,
            _ => return None,
        };
        let ty_idx = ports.iter().find(|p| p.name.text == port)?.ty;
        Some(self.resolve_type_ref(ty_idx))
    }

    // ═══ Kontrol modu (§4) ════════════════════════════════════════

    fn check(&mut self, expr: Idx<Expr>, expected: TypeId) {
        // Error her tiple uyumlu — ama alt ifadeler yine denetlenir.
        if self.types.is_error(expected) {
            self.synth(expr);
            return;
        }
        let ast = self.ast;
        let span = ast.exprs[expr].span;
        match &ast.exprs[expr].kind {
            ExprKind::IntLit {
                value,
                suffix: None,
                ..
            } => {
                self.check_int_lit(*value, expected, span);
                self.expr_types.insert(expr, expected);
            }
            ExprKind::If {
                cond,
                then_expr,
                else_expr,
            } => {
                let (cond, then_expr, else_expr) = (*cond, *then_expr, *else_expr);
                let bool_ty = self.types.bool_ty();
                self.check(cond, bool_ty);
                self.check(then_expr, expected);
                self.check(else_expr, expected);
                self.expr_types.insert(expr, expected);
            }
            _ => {
                let actual = self.synth(expr);
                self.expect_assignable(actual, expected, span);
            }
        }
    }

    /// Soneksiz literali beklenen tipe uyarla (§4).
    fn check_int_lit(&mut self, value: u128, expected: TypeId, span: Span) {
        match *self.types.ty(expected) {
            Ty::UInt { width } | Ty::UIntFlex { hi: width, .. } => {
                if !uint_fits(value, width) {
                    self.literal_overflow(value, expected, span);
                }
            }
            Ty::SInt { width } | Ty::SIntFlex { hi: width, .. } => {
                if !sint_fits(value, width) {
                    self.literal_overflow(value, expected, span);
                }
            }
            Ty::Trit => {
                if !matches!(value, 0 | 1) {
                    self.diagnostics.push(Diagnostic::error(
                        ErrorCode::E2011,
                        lstr!(en: "Trit literal must be {{-1, 0, +1}}"; tr: "Trit literali {{-1, 0, +1}} olmalı"),
                        LabeledSpan::primary(span, lstr!(en: "{value} is not in this set"; tr: "{value} bu kümede değil")),
                        lstr!(en: "make the value -1, 0 or 1"; tr: "değeri -1, 0 veya 1 yapın"),
                    ));
                }
            }
            Ty::Bool => {
                self.diagnostics.push(Diagnostic::error(
                    ErrorCode::E2003,
                    lstr!(en: "numeric literal in bool context"; tr: "sayısal literal bool bağlamında"),
                    LabeledSpan::primary(span, lstr!(en: "expected bool"; tr: "bool bekleniyor")),
                    lstr!(en: "write true or false"; tr: "true veya false yazın"),
                ));
            }
            _ => {
                let lit = self.types.int_lit();
                self.err_type_mismatch(expected, lit, span);
            }
        }
    }

    /// §5 — atanabilirlik: örtük daraltma DA genişleme DE yasak. Esnek
    /// aritmetik sonucu (ADR-0025) hedef genişliği aralığındaysa uyar.
    fn expect_assignable(&mut self, actual: TypeId, expected: TypeId, span: Span) {
        if actual == expected || self.types.is_error(actual) || self.types.is_error(expected) {
            return;
        }
        // Literal her sayısal tipe uyar (sınır kontrolü yapıldı).
        if self.types.is_int_lit(actual) && self.is_literal_adaptable(expected) {
            return;
        }
        if let (Some((sa, alo, ahi)), Some((se, elo, ehi))) =
            (self.types.int_range(actual), self.types.int_range(expected))
        {
            if sa != se {
                self.diagnostics.push(Diagnostic::error(
                    ErrorCode::E2002,
                    lstr!(en: "sign mismatch"; tr: "işaret uyumsuzluğu"),
                    LabeledSpan::primary(span, lstr!(en: "signed and unsigned are mixed"; tr: "işaretli ve işaretsiz karışıyor")),
                    lstr!(en: "use an explicit cast with as"; tr: "as ile açık dönüşüm yapın"),
                ));
                return;
            }
            if alo.max(elo) <= ahi.min(ehi) {
                return;
            }
            self.width_mismatch(ahi, ehi, if sa { "i" } else { "u" }, span);
            return;
        }
        match (self.types.ty(actual), self.types.ty(expected)) {
            (&Ty::Bits { width: a }, &Ty::Bits { width: b }) => {
                self.diagnostics.push(Diagnostic::error(
                    ErrorCode::E2001,
                    lstr!(
                        en: "bit width mismatch: a bits<{a}> value cannot be assigned to a bits<{b}> target";
                        tr: "bit genişliği uyumsuzluğu: bits<{a}> değeri bits<{b}> hedefe atanamaz"
                    ),
                    LabeledSpan::primary(span, lstr!(en: "widths differ"; tr: "genişlikler farklı")),
                    lstr!(en: "make the source and target widths equal"; tr: "kaynak ve hedef genişliklerini eşitleyin"),
                ));
            }
            _ => self.err_type_mismatch(expected, actual, span),
        }
    }

    /// E2001 — donanımda genişleme bedava değildir; her iki yön de açık
    /// dönüşüm ister (§5 tasarım kararı).
    fn width_mismatch(&mut self, a: u16, b: u16, prefix: &str, span: Span) {
        let (msg, label) = if a > b {
            (
                lstr!(en: "a {a}-bit value does not fit in a {b}-bit target"; tr: "{a} bit değer {b} bit hedefe sığmaz"),
                lstr!(en: "implicit narrowing is not allowed"; tr: "örtük daraltma yasak"),
            )
        } else {
            (
                lstr!(en: "a {a}-bit value does not implicitly widen to a {b}-bit target"; tr: "{a} bit değer {b} bit hedefe örtük genişlemez"),
                lstr!(en: "implicit widening is not allowed"; tr: "örtük genişleme yasak"),
            )
        };
        self.diagnostics.push(
            Diagnostic::error(
                ErrorCode::E2001,
                msg,
                LabeledSpan::primary(span, label),
                lstr!(en: "explicit cast: (expr) as {prefix}{b}"; tr: "açık dönüşüm: (ifade) as {prefix}{b}"),
            )
            .with_note(
                NoteKind::Reason,
                lstr!(en: "widening requires extra wires and logic in hardware; it must be visible"; tr: "genişletme donanımda ek tel ve mantık gerektirir; görünür olmalı"),
            ),
        );
    }

    // ═══ Tip dönüşümü (§3.6) ══════════════════════════════════════

    fn check_cast_legal(&mut self, src: TypeId, dst: TypeId, span: Span) {
        // Esnek aritmetik sonucu doğal genişliğiyle dönüştürülür.
        let src = self.types.concrete(src);
        if src == dst || self.types.is_error(src) || self.types.is_error(dst) {
            return;
        }
        let legal = match (self.types.ty(src).clone(), self.types.ty(dst).clone()) {
            // Literal açık dönüşümle her sayısal tipe gider.
            (Ty::IntLit, Ty::UInt { .. } | Ty::SInt { .. } | Ty::Bits { .. }) => true,
            // Genişletme — her zaman güvenli.
            (Ty::UInt { width: a }, Ty::UInt { width: b }) if b >= a => true,
            (Ty::SInt { width: a }, Ty::SInt { width: b }) if b >= a => true,
            // Daraltma — izinli ama uyarı (W2010).
            (Ty::UInt { width: a }, Ty::UInt { width: b })
            | (Ty::SInt { width: a }, Ty::SInt { width: b }) => {
                self.diagnostics.push(Diagnostic::warning(
                    ErrorCode::W2010,
                    lstr!(en: "{a}-bit → {b}-bit narrowing, upper bits are truncated"; tr: "{a} bit → {b} bit daraltma, üst bitler kesilir"),
                    LabeledSpan::primary(span, lstr!(en: "possible loss of information"; tr: "bilgi kaybı olabilir")),
                    lstr!(en: "if the narrowing is intentional this is fine; otherwise mask first"; tr: "bilinçli daraltma ise sorun yok; değilse önce maskeleme yapın"),
                ));
                true
            }
            // Bool ↔ 1-bit.
            (Ty::Bool, Ty::UInt { width: 1 }) | (Ty::UInt { width: 1 }, Ty::Bool) => true,
            // İşaret değişimi — açık cast ile serbest.
            (Ty::UInt { .. }, Ty::SInt { .. }) | (Ty::SInt { .. }, Ty::UInt { .. }) => true,
            // bits<N> ↔ sayısal, aynı genişlikte.
            (Ty::Bits { width: a }, Ty::UInt { width: b })
            | (Ty::UInt { width: a }, Ty::Bits { width: b })
            | (Ty::Bits { width: a }, Ty::SInt { width: b }) => a == b,
            // Trit → işaretli (genişleme, en az 2 bit).
            (Ty::Trit, Ty::SInt { width }) => width >= 2,
            // Sayısal → Trit YASAK: sessiz kırpma olur.
            _ => false,
        };
        if !legal {
            let src_s = self.types.display(src);
            let dst_s = self.types.display(dst);
            self.diagnostics.push(Diagnostic::error(
                ErrorCode::E2009,
                lstr!(en: "cast '{src_s}' → '{dst_s}' is invalid"; tr: "'{src_s}' → '{dst_s}' dönüşümü geçersiz"),
                LabeledSpan::primary(span, lstr!(en: "this cast is not defined"; tr: "bu dönüşüm tanımlı değil")),
                lstr!(en: "an intermediate cast may be needed"; tr: "ara dönüşüm gerekebilir"),
            ));
        }
    }

    // ═══ Literal sınırları ════════════════════════════════════════

    fn suffix_ty(&mut self, suffix: IntSuffix) -> TypeId {
        let ty = match suffix {
            IntSuffix::U8 => Ty::UInt { width: 8 },
            IntSuffix::U16 => Ty::UInt { width: 16 },
            IntSuffix::U32 => Ty::UInt { width: 32 },
            IntSuffix::U64 => Ty::UInt { width: 64 },
            IntSuffix::I8 => Ty::SInt { width: 8 },
            IntSuffix::I16 => Ty::SInt { width: 16 },
            IntSuffix::I32 => Ty::SInt { width: 32 },
            IntSuffix::I64 => Ty::SInt { width: 64 },
        };
        self.types.intern(ty)
    }

    fn check_literal_fits(&mut self, value: u128, ty: TypeId, span: Span) {
        let fits = match *self.types.ty(ty) {
            Ty::UInt { width } => uint_fits(value, width),
            Ty::SInt { width } => sint_fits(value, width),
            _ => true,
        };
        if !fits {
            self.literal_overflow(value, ty, span);
        }
    }

    fn literal_overflow(&mut self, value: u128, ty: TypeId, span: Span) {
        let shown = self.types.display(ty);
        let max = match *self.types.ty(ty) {
            Ty::UInt { width } | Ty::UIntFlex { hi: width, .. } if width < 128 => {
                (1u128 << width) - 1
            }
            Ty::SInt { width } | Ty::SIntFlex { hi: width, .. } if width > 0 && width <= 128 => {
                (1u128 << (width - 1)) - 1
            }
            _ => u128::MAX,
        };
        self.diagnostics.push(Diagnostic::error(
            ErrorCode::E2010,
            lstr!(en: "literal {value} does not fit in type {shown} (maximum {max})"; tr: "literal {value}, {shown} tipine sığmıyor (maksimum {max})"),
            LabeledSpan::primary(span, lstr!(en: "value is outside the type's range"; tr: "değer tip aralığının dışında")),
            lstr!(en: "use a wider type or reduce the value"; tr: "daha geniş bir tip kullanın veya değeri küçültün"),
        ));
    }

    // ═══ Yardımcılar ══════════════════════════════════════════════

    /// Sınır/genişlik denetimi için SESSİZ sabit değerlendirme:
    /// çalışma zamanı değeri sabit değilse tanı üretmeden vazgeçilir
    /// (örn. `x[i]` döngü değişkeniyle — E2021 kaskadı istenmez).
    fn try_const_eval(&mut self, expr: Idx<Expr>) -> Option<i128> {
        let before = self.ev.diagnostics.len();
        let value = self.ev.const_eval(expr);
        self.ev.diagnostics.truncate(before);
        match value {
            ConstValue::Int(n) => Some(n),
            _ => None,
        }
    }

    fn check_is_bool(&mut self, ty: TypeId, span: Span) {
        if self.types.is_error(ty) || matches!(self.types.ty(ty), Ty::Bool) {
            return;
        }
        let bool_ty = self.types.bool_ty();
        self.err_type_mismatch(bool_ty, ty, span);
    }

    fn index_out_of_bounds(&mut self, index: i128, width: u64, span: Span) {
        self.diagnostics.push(Diagnostic::error(
            ErrorCode::E2006,
            lstr!(en: "index {index} out of bounds (width {width})"; tr: "indeks {index} sınır dışı (genişlik {width})"),
            LabeledSpan::primary(span, lstr!(en: "invalid bit index"; tr: "geçersiz bit indeksi")),
            lstr!(en: "valid range: 0..{}", width.saturating_sub(1); tr: "geçerli aralık: 0..{}", width.saturating_sub(1)),
        ));
    }

    fn err_type_mismatch(&mut self, expected: TypeId, actual: TypeId, span: Span) {
        let exp = self.types.display(expected);
        let act = self.types.display(actual);
        self.err_type_mismatch_msg(
            span,
            &lstr!(en: "type mismatch: expected '{exp}', found '{act}'"; tr: "tip uyumsuzluğu: '{exp}' bekleniyor, '{act}' bulundu"),
            &lstr!(en: "adapt the value to the target type; use an explicit cast with 'as' if needed"; tr: "değeri hedef tipe uyarlayın; gerekiyorsa 'as' ile açık dönüşüm yapın"),
        );
    }

    fn err_type_mismatch_msg(&mut self, span: Span, msg: &str, help: &str) {
        self.diagnostics.push(Diagnostic::error(
            ErrorCode::E2003,
            msg,
            LabeledSpan::primary(span, lstr!(en: "mismatched types"; tr: "uyumsuz tip")),
            help,
        ));
    }
}

/// Kontrat kapsam denetimi için ifade ağacındaki Path düğümlerini toplar.
fn collect_path_exprs(ast: &SourceFile, expr: Idx<Expr>, out: &mut Vec<Idx<Expr>>) {
    match &ast.exprs[expr].kind {
        ExprKind::Path(_) => out.push(expr),
        ExprKind::Binary { lhs, rhs, .. } => {
            collect_path_exprs(ast, *lhs, out);
            collect_path_exprs(ast, *rhs, out);
        }
        ExprKind::Unary { operand, .. } => collect_path_exprs(ast, *operand, out),
        ExprKind::Index { base, index } => {
            collect_path_exprs(ast, *base, out);
            collect_path_exprs(ast, *index, out);
        }
        ExprKind::Range { base, hi, lo } => {
            collect_path_exprs(ast, *base, out);
            collect_path_exprs(ast, *hi, out);
            collect_path_exprs(ast, *lo, out);
        }
        ExprKind::Field { base, .. } => collect_path_exprs(ast, *base, out),
        ExprKind::Call { callee, args } => {
            collect_path_exprs(ast, *callee, out);
            for &a in args {
                collect_path_exprs(ast, a, out);
            }
        }
        ExprKind::Cast { expr: inner, .. } => collect_path_exprs(ast, *inner, out),
        ExprKind::If {
            cond,
            then_expr,
            else_expr,
        } => {
            collect_path_exprs(ast, *cond, out);
            collect_path_exprs(ast, *then_expr, out);
            collect_path_exprs(ast, *else_expr, out);
        }
        ExprKind::Match { scrutinee, arms } => {
            collect_path_exprs(ast, *scrutinee, out);
            for arm in arms {
                if let Some(guard) = arm.guard {
                    collect_path_exprs(ast, guard, out);
                }
                if let MatchArmBody::Expr(e) = &arm.body {
                    collect_path_exprs(ast, *e, out);
                }
            }
        }
        ExprKind::StructLit { fields, .. } => {
            for f in fields {
                if let Some(v) = f.value {
                    collect_path_exprs(ast, v, out);
                }
            }
        }
        ExprKind::ArrayLit(ArrayLitKind::List(items)) | ExprKind::TupleLit(items) => {
            for &i in items {
                collect_path_exprs(ast, i, out);
            }
        }
        ExprKind::ArrayLit(ArrayLitKind::Repeat { value, count }) => {
            collect_path_exprs(ast, *value, out);
            collect_path_exprs(ast, *count, out);
        }
        ExprKind::IntLit { .. }
        | ExprKind::BoolLit(_)
        | ExprKind::StringLit(_)
        | ExprKind::Todo { .. }
        | ExprKind::Error => {}
    }
}

/// Tanı metinlerinde kontrat anahtar kelimesi.
fn contract_keyword(kind: ContractKind) -> &'static str {
    match kind {
        ContractKind::Requires => "requires",
        ContractKind::Ensures => "ensures",
        ContractKind::Invariant => "invariant",
        ContractKind::Cover => "cover",
        ContractKind::Assert => "assert",
        ContractKind::Assume => "assume",
    }
}

/// Genişlemiş sonuç genişliği MAX_WIDTH ve u16 gösterim sınırıyla kırpılır.
fn clamp_width(w: u32) -> u16 {
    w.min(MAX_WIDTH).min(u32::from(u16::MAX)) as u16
}

/// `value` işaretsiz `width` bite sığıyor mu?
fn uint_fits(value: u128, width: u16) -> bool {
    width >= 128 || value >> width == 0
}

/// `value` işaretli `width` bite (işaret dahil) sığıyor mu?
fn sint_fits(value: u128, width: u16) -> bool {
    if width == 0 {
        return false;
    }
    width > 128 || value < 1u128 << (width - 1).min(127)
}
