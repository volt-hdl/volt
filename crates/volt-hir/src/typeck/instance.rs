//! Modül ve yerleşik primitif örneklemesi (type-inference.md §6;
//! ADR-0027/0029 stdlib primitifleri): port bağlamaları hedef portun
//! tipiyle check modunda denetlenir.

use volt_ast::{ExprKind, GenericArg, InstanceDecl, ItemKind, PortBinding, PortDir};
use volt_diagnostics::{lstr, ErrorCode};
use volt_span::Span;

use super::TypeChecker;
use crate::builtin::{BuiltinPrim, ConstRule, PortKind};
use crate::drivers::DriverKind;
use crate::resolve::{DefId, DefKind};
use crate::ty::{ModuleId, Ty, TypeId};

impl TypeChecker<'_, '_> {
    pub(super) fn handle_instance(&mut self, inst: &InstanceDecl) {
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
            if let Some(t) = target {
                self.record_shared_line(inst, t, b);
            }
            let Some(value) = b.value else { continue };
            match target.and_then(|t| self.port_type_of(t, &b.port_name.text)) {
                Some(port_ty) => self.check(value, port_ty),
                None => {
                    self.synth(value);
                }
            }
        }
    }

    /// Alt modülün inout/opendrain portuna bağlanan hat üç durumlu bir
    /// sürücü alır (ADR-0073): aynı hatta push-pull atama E4001 olur.
    fn record_shared_line(&mut self, inst: &InstanceDecl, module_def: DefId, b: &PortBinding) {
        let ast = self.ast;
        let Some(&item_idx) = self.res.item_of_def.get(&module_def) else {
            return;
        };
        let ports = match &ast.items_arena[item_idx].kind {
            ItemKind::Module(m) => &m.ports,
            ItemKind::Extern(x) => &x.ports,
            // Örneklenemeyen öğeler (hedef hatası isim çözümlemede).
            ItemKind::Domain(_)
            | ItemKind::Fn(_)
            | ItemKind::Struct(_)
            | ItemKind::Enum(_)
            | ItemKind::Const(_)
            | ItemKind::TypeAlias(_)
            | ItemKind::Test(_)
            | ItemKind::Error => return,
        };
        let Some(port) = ports.iter().find(|p| p.name.text == b.port_name.text) else {
            return;
        };
        if !matches!(port.direction, PortDir::InOut | PortDir::OpenDrain) {
            return;
        }
        let line_span = match b.value {
            Some(v) => ast.exprs[v].span,
            None => b.port_name.span,
        };
        let Some(&def) = self.res.use_spans.get(&line_span) else {
            return;
        };
        if !matches!(self.res.def_kind(def), DefKind::Wire | DefKind::Port { .. }) {
            return;
        }
        let group = self.new_group();
        let kind = DriverKind::SharedLine {
            instance: inst.name.text.clone(),
            port: b.port_name.text.clone(),
        };
        self.drivers.record_kind(def, b.span, group, kind);
    }

    /// Yerleşik stdlib primitifi örneklemesi (ADR-0027/0029): generic
    /// argüman sayısı/biçimi, sabit argüman kısıtı (E2025) ve giriş
    /// bağlama tipleri.
    fn handle_builtin_instance(
        &mut self,
        inst: &InstanceDecl,
        def: Option<DefId>,
        prim: BuiltinPrim,
    ) {
        let (data, dim) = self.builtin_generic_args(inst, prim);
        self.check_memory_element(inst, prim, data);
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
    pub(super) fn builtin_port_type(&mut self, kind: PortKind, data: TypeId, dim: u32) -> TypeId {
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
                // Bool `width_of`'ta None döner ama 1 bit taşır; struct/enum
                // T paketlenmiş genişliğiyle (ADR-0087).
                let w = self.types.signal_width(data).unwrap_or(1) as u32;
                self.types.intern(Ty::Bits {
                    width: (dim * w) as u16,
                })
            }
        }
    }

    /// RAM ailesinin (`Ram`, `DualPortRam`, `AsyncDualPortRam`) öğesi enum
    /// ya da `Trit` yaprağı taşıyamaz (ADR-0087): hiç yazılmamış adres
    /// belleğin başlangıç içeriğini — ham bitleri — döndürür, bu da
    /// geçerli bir varyant olmayabilir. ADR-0077'nin `raw as P` yasağıyla
    /// (E2009) aynı gerekçe; FIFO ailesi yalnız yazılmış değerleri ve
    /// T'nin varsayılan kodlamasıyla reset'lenen register'ı gösterir.
    fn check_memory_element(&mut self, inst: &InstanceDecl, prim: BuiltinPrim, data: TypeId) {
        if !matches!(
            prim,
            BuiltinPrim::Ram | BuiltinPrim::DualPortRam | BuiltinPrim::AsyncDualPortRam
        ) {
            return;
        }
        let leaf = match *self.types.ty(data) {
            Ty::Enum(_) | Ty::Trit => Some(String::new()),
            Ty::Struct(s) => self.struct_layout(s).and_then(|l| {
                l.leaves
                    .iter()
                    .find(|l| l.kind != volt_ast::struct_layout::LeafKind::Plain)
                    .map(|l| l.dotted())
            }),
            // Düz (enum/Trit olmayan) veri: bellek yaprağı sorunsuz.
            Ty::Bool
            | Ty::UInt { .. }
            | Ty::SInt { .. }
            | Ty::Bits { .. }
            | Ty::Clock
            | Ty::Reset { .. }
            | Ty::Array { .. }
            | Ty::Tuple(_)
            | Ty::Instance(_)
            | Ty::Builtin { .. }
            | Ty::Delayed { .. }
            | Ty::IntLit
            | Ty::UIntFlex { .. }
            | Ty::SIntFlex { .. }
            | Ty::Error => None,
        };
        let Some(field) = leaf else {
            return;
        };
        let (name, t) = (prim.name(), self.show(data));
        let w = self.types.signal_width(data).unwrap_or(1);
        let span = match inst.generic_args.first() {
            Some(GenericArg::Type(ty)) => self.ast.types[*ty].span,
            _ => inst.name.span,
        };
        let what = if field.is_empty() {
            lstr!(en: "'{t}' is an enum or Trit"; tr: "'{t}' bir enum ya da Trit")
        } else {
            lstr!(en: "field '{field}' of '{t}' is an enum or Trit"; tr: "'{t}' tipinin '{field}' alanı enum ya da Trit")
        };
        self.error(
            ErrorCode::E2009,
            span,
            lstr!(en: "{name}<{t}, ...> is invalid: {what}, and a never-written address reads back raw bits that may be no valid value"; tr: "{name}<{t}, ...> geçersiz: {what} ve hiç yazılmamış adres geçerli olmayabilecek ham bitler döndürür"),
            lstr!(en: "memory contents are not initialised"; tr: "bellek içeriği başlatılmaz"),
            lstr!(en: "store the raw bits ({name}<u{w}, ...> with 'value as u{w}') and decode the enum field explicitly after reading (ADR-0077)"; tr: "ham bitleri saklayın ({name}<u{w}, ...>, 'değer as u{w}') ve okuduktan sonra enum alanını açıkça çözün (ADR-0077)"),
        );
    }

    /// Generic argümanları çözer: `T` veri tipi + sabit boyut
    /// (DEPTH/WIDTH/LEN/N). Yanlış sayı E2003, literal olmayan sabit
    /// E2008, kural dışı sabit E2025 üretir. Dönüş: (T tipi, sabit).
    fn builtin_generic_args(&mut self, inst: &InstanceDecl, prim: BuiltinPrim) -> (TypeId, u32) {
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
        let dim = if prim.const_arg_count() == 1 {
            self.builtin_const_arg(inst, prim)
        } else {
            0
        };
        (data, dim)
    }

    /// Sabit argüman (T'den sonra gelir): tam sayı literali olmalı ve
    /// primitifin kuralına uymalı (iki kuvveti / aralık). Parser çıplak
    /// bir ismi tip sayar; sabit ADIYLA verilen değer de E2008'e düşer
    /// (literal zorunlu). Geçersizse 0 döner.
    fn builtin_const_arg(&mut self, inst: &InstanceDecl, prim: BuiltinPrim) -> u32 {
        let rule = prim.const_rule().expect("const_arg_count == 1");
        match &inst.generic_args[prim.type_arg_count()] {
            GenericArg::Const(e) => {
                let span = self.ast.exprs[*e].span;
                match self.ast.exprs[*e].kind {
                    ExprKind::IntLit { value, .. } if rule.allows(value) => return value as u32,
                    ExprKind::IntLit { value, .. } => {
                        self.err_builtin_const_rule(prim, value, span);
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
                    | ExprKind::Error => self.err_builtin_const_not_literal(prim, span),
                }
            }
            GenericArg::Type(t) => {
                let span = self.ast.types[*t].span;
                self.err_builtin_const_not_literal(prim, span);
            }
        }
        0
    }

    /// E2025 — sabit generic argüman primitifin kuralına uymuyor.
    fn err_builtin_const_rule(&mut self, prim: BuiltinPrim, value: u128, span: Span) {
        let (name, param) = (prim.name(), prim.const_param_name());
        let msg = match prim.const_rule() {
            Some(ConstRule::PowerOfTwo { .. }) => {
                lstr!(en: "{name} {param} must be a power of two, got {value}";
                      tr: "{name} {param} iki kuvveti olmalı, {value} verildi")
            }
            _ => lstr!(en: "{name} {param} is out of range, got {value}";
                       tr: "{name} {param} aralık dışı, {value} verildi"),
        };
        self.error(
            ErrorCode::E2025,
            span,
            msg,
            lstr!(en: "invalid size parameter"; tr: "geçersiz boyut parametresi"),
            lstr!(en: "{}", prim.const_rule_hint_en(); tr: "{}", prim.const_rule_hint_tr()),
        );
    }

    /// E2003 — yerleşik primitifte yanlış generic argüman sayısı/biçimi.
    fn err_builtin_arity(&mut self, inst: &InstanceDecl, prim: BuiltinPrim) {
        let shape = prim.generic_shape();
        self.error(
            ErrorCode::E2003,
            inst.name.span,
            lstr!(en: "'{}' expects {} type and {} const generic argument(s), got {}",
                      prim.name(), prim.type_arg_count(), prim.const_arg_count(),
                      inst.generic_args.len();
                  tr: "'{}' {} tip ve {} sabit generic argüman bekler, {} verildi",
                      prim.name(), prim.type_arg_count(), prim.const_arg_count(),
                      inst.generic_args.len()),
            lstr!(en: "wrong generic argument count"; tr: "yanlış generic argüman sayısı"),
            lstr!(en: "write it as {shape} {{ ... }}"; tr: "{shape} {{ ... }} biçiminde yazın"),
        );
    }

    /// E2008 — sabit generic argüman literal değil (sabit ismi ya da ifade).
    fn err_builtin_const_not_literal(&mut self, prim: BuiltinPrim, span: Span) {
        let (name, param) = (prim.name(), prim.const_param_name());
        self.error(
            ErrorCode::E2008,
            span,
            lstr!(en: "{name} {param} must be a compile-time integer literal";
                  tr: "{name} {param} derleme zamanı tam sayı literali olmalı"),
            lstr!(en: "not an integer literal"; tr: "tam sayı literali değil"),
            lstr!(en: "write the size directly, e.g. {}", prim.generic_shape();
                  tr: "boyutu doğrudan yazın, ör. {}", prim.generic_shape()),
        );
    }

    /// Hedef modülün port tipi (modüller arası akış için).
    pub(super) fn port_type_of(&mut self, module_def: DefId, port: &str) -> Option<TypeId> {
        let ast = self.ast;
        let &item_idx = self.res.item_of_def.get(&module_def)?;
        let ports = match &ast.items_arena[item_idx].kind {
            ItemKind::Module(m) => &m.ports,
            ItemKind::Extern(x) => &x.ports,
            // Örneklenemeyen öğeler (hedef hatası isim çözümlemede).
            ItemKind::Domain(_)
            | ItemKind::Fn(_)
            | ItemKind::Struct(_)
            | ItemKind::Enum(_)
            | ItemKind::Const(_)
            | ItemKind::TypeAlias(_)
            | ItemKind::Test(_)
            | ItemKind::Error => return None,
        };
        let ty_idx = ports.iter().find(|p| p.name.text == port)?.ty;
        Some(self.resolve_type_ref(ty_idx))
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::codes;

    const CHILD: &str = "module Child {\n    in  d : u8\n    out q : u8\n\n    q = d\n}\n\n";

    #[test]
    fn port_binding_is_checked_against_the_target_port_type() {
        let ok = format!(
            "{CHILD}module Top {{\n    in  a : u8\n    out y : u8\n\n    let c = Child {{ d: a }}\n    y = c.q\n}}\n"
        );
        assert!(codes(&ok).is_empty(), "{:?}", codes(&ok));
        let narrow = format!(
            "{CHILD}module Top {{\n    in  a : u16\n    out y : u8\n\n    let c = Child {{ d: a }}\n    y = c.q\n}}\n"
        );
        assert!(codes(&narrow).contains(&"E2001"), "{:?}", codes(&narrow));
    }

    #[test]
    fn instance_output_has_the_port_type() {
        let src = format!(
            "{CHILD}module Top {{\n    in  a : u8\n    out y : u16\n\n    let c = Child {{ d: a }}\n    let _q = c.q\n    y = 0\n}}\n"
        );
        assert_eq!(super::super::testutil::def_ty(&src, "_q"), "u8");
    }
}
