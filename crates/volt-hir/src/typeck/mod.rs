//! Çift yönlü tip kontrolü (docs/spec/type-inference.md §2-§6) — F2a+F2b.
//!
//! `synth` (↑ "bu ifadenin tipi ne?") ve `check` (↓ "bu ifade T tipinde
//! mi?") bağlama göre seçilir: beklenen tip biliniyorsa check, değilse
//! synth. `Ty::Error` sessizce yayılır — kaskad hata üretilmez.
//!
//! Modüller spec bölümlerini izler:
//!
//! | modül      | sorumluluk                                     | spec         |
//! |------------|------------------------------------------------|--------------|
//! | `mod`      | public API, ana akış, tanım tipleri            | §2, §3.2     |
//! | `type_ref` | AST tip referansı → arena tipi                 | §1           |
//! | `synth`    | literal, yol, tekli, if, dizi/struct literali  | §3.1-2,4,7   |
//! | `binop`    | aritmetik, bit, kaydırma, karşılaştırma        | §3.3         |
//! | `cast`     | `as` dönüşümü                                  | §3.6         |
//! | `select`   | indeks, aralık, parça ve alan seçimi           | §3.5         |
//! | `check`    | kontrol modu, literal çözümleme, atanabilirlik | §4, §5       |
//! | `stmt`     | modül gövdesi, reg/let, bloklar, atama         | §6           |
//! | `instance` | modül ve yerleşik primitif örneklemesi         | §6           |
//! | `contract` | kontrat ifadesi ve kapsamı                     | F4a          |
//! | `width`    | tam sayı aralığı buluşması, genişlik sınırları | §1, §3.3     |
//! | `diag`     | ortak tanı yardımcıları                        | §7, §8       |
//!
//! Sürücü analizi (§11) `crate::drivers`'tadır; `stmt` atamaları oraya
//! kaydeder, `run`/`check_module` denetimleri tetikler.

mod binop;
mod cast;
mod check;
mod contract;
mod diag;
mod instance;
mod select;
mod stmt;
mod synth;
mod type_ref;
mod width;

use std::collections::HashMap;

use volt_ast::{Expr, Idx, ItemKind, Name, SourceFile, TypeRef};
use volt_diagnostics::Diagnostic;

use crate::consteval::{ConstEvaluator, ConstValue};
use crate::drivers::DriverTable;
use crate::resolve::{DefId, DefKind, ResolveResult};
use crate::ty::{EnumId, ModuleId, Ty, TypeArena, TypeId};

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

impl TypeChecker<'_, '_> {
    fn run(&mut self) {
        let ast = self.ast;
        for &item_idx in &ast.items {
            if let ItemKind::Const(c) = &ast.items_arena[item_idx].kind {
                let ty = self.resolve_type_ref(c.ty);
                self.record_def_type(&c.name, ty);
                self.check(c.value, ty);
            }
        }
        for &item_idx in &ast.items {
            match &ast.items_arena[item_idx].kind {
                ItemKind::Module(m) => self.check_module(m),
                ItemKind::Extern(x) => self.type_extern_ports(x),
                _ => {}
            }
        }
        let mut diags = Vec::new();
        self.drivers.check_multiple_drivers(self.res, &mut diags);
        self.drivers.check_write_only(self.res, &mut diags);
        self.diagnostics.extend(diags);
    }

    // ═══ Tanım tipleri (§3.2) ═════════════════════════════════════

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
}

/// Modül içi birim testleri için ortak yardımcılar: kaynak → tam analiz.
#[cfg(test)]
mod testutil {
    use volt_diagnostics::Diagnostic;
    use volt_syntax::{parse, FileId};

    use crate::{analyze, AnalysisResult};

    fn analyzed(src: &str) -> AnalysisResult {
        let parsed = parse(FileId(0), src);
        assert!(
            parsed.diagnostics.is_empty(),
            "kaynak ayrışmalı: {:?}",
            parsed.error_codes()
        );
        analyze(&parsed.ast)
    }

    /// Yalnız tip denetleyicinin ürettiği tanılar.
    pub(super) fn diagnostics(src: &str) -> Vec<Diagnostic> {
        analyzed(src).typeck.diagnostics
    }

    /// Tip denetleyici tanı kodları, üretim sırasıyla.
    pub(super) fn codes(src: &str) -> Vec<&'static str> {
        diagnostics(src).iter().map(|d| d.code.as_str()).collect()
    }

    /// İsme göre tanımın çıkarılan tipinin metin gösterimi.
    pub(super) fn def_ty(src: &str, name: &str) -> String {
        let result = analyzed(src);
        let (def, _) = result
            .resolve
            .def_by_name(name)
            .unwrap_or_else(|| panic!("'{name}' tanımı bulunmalı"));
        let id = *result
            .typeck
            .def_types
            .get(&def)
            .unwrap_or_else(|| panic!("'{name}' için tip kaydı bulunmalı"));
        result.typeck.types.display(id)
    }

    /// Kayıtlı tüm ifade tiplerinin metin gösterimleri (sırasız).
    pub(super) fn expr_type_names(src: &str) -> Vec<String> {
        let result = analyzed(src);
        result
            .typeck
            .expr_types
            .values()
            .map(|&t| result.typeck.types.display(t))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::{codes, def_ty};

    #[test]
    fn top_level_const_is_checked_against_its_declared_type() {
        assert!(codes("const BIG : u8 = 300\n").contains(&"E2010"));
        assert!(codes("const OK : u8 = 200\n").is_empty());
    }

    #[test]
    fn const_reference_takes_the_declared_const_type() {
        let src = "const K : u16 = 7\n\nmodule M {\n    in  a : u8\n    out y : u8\n\n    let _k = K\n    y = a\n}\n";
        assert_eq!(def_ty(src, "_k"), "u16");
    }

    #[test]
    fn driver_analysis_runs_after_all_modules() {
        let src = "module M {\n    in  a : u8\n    out y : u8\n    out z : u8\n\n    y = a\n}\n";
        assert!(codes(src).contains(&"E4002"));
    }
}
