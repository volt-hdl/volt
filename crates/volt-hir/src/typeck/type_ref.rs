//! AST tip referansı → arena tipi (type-inference.md §1): yerleşik
//! tipler, `bits<N>`/`uint<N>`/`sint<N>` (ADR-0041), diziler, demetler,
//! struct/enum adları ve tip takma adları.

use volt_ast::{Expr, Idx, ItemKind, TypeRef, TypeRefKind};

use super::TypeChecker;
use crate::consteval::MAX_ARRAY_LEN;
use crate::resolve::{is_widened_int_type, DefKind};
use crate::ty::{EnumId, StructId, Ty, TypeId};

impl TypeChecker<'_, '_> {
    /// AST tip referansı → arena tipi. Genişlikler sessizce sabitlenir;
    /// geçersiz genişlik tanıları check_type_positions'ta zaten verildi.
    pub(super) fn resolve_type_ref(&mut self, ty_idx: Idx<TypeRef>) -> TypeId {
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
            // `bits<N>` / `uint<N>` / `sint<N>` (ADR-0041): genişlik sabiti
            // aynı yoldan çözülür; geçersiz genişlik tanısı consteval'de.
            kind @ (TypeRefKind::Bits(e) | TypeRefKind::UIntN(e) | TypeRefKind::SIntN(e)) => {
                self.resolve_sized(kind, *e)
            }
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
                let tys: Vec<TypeId> = items.iter().map(|&t| self.resolve_type_ref(t)).collect();
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

    /// Sabit genişlikli aile: genişlik 1..=u16::MAX aralığında bir
    /// derleme zamanı sabiti olmalı, değilse sessiz Error.
    fn resolve_sized(&mut self, kind: &TypeRefKind, width: Idx<Expr>) -> TypeId {
        match self.try_const_eval(width) {
            Some(n) if (1..=i128::from(u16::MAX)).contains(&n) => {
                let width = n as u16;
                let ty = match kind {
                    TypeRefKind::UIntN(_) => Ty::UInt { width },
                    TypeRefKind::SIntN(_) => Ty::SInt { width },
                    _ => Ty::Bits { width },
                };
                self.types.intern(ty)
            }
            _ => self.types.error(),
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
}

#[cfg(test)]
mod tests {
    use super::super::testutil::def_ty;

    fn module(port_ty: &str) -> String {
        format!("module M {{\n    in  a : {port_ty}\n    out y : bool\n\n    let _x = a\n    y = true\n}}\n")
    }

    #[test]
    fn widened_and_generic_integer_families_resolve_to_the_same_types() {
        assert_eq!(def_ty(&module("u9"), "_x"), "u9");
        assert_eq!(def_ty(&module("uint<9>"), "_x"), "u9");
        assert_eq!(def_ty(&module("sint<12>"), "_x"), "i12");
        assert_eq!(def_ty(&module("bits<5>"), "_x"), "bits<5>");
    }

    #[test]
    fn const_width_and_array_length_are_evaluated() {
        let src = "const W : u32 = 4\n\nmodule M {\n    in  a : [bits<W>; 3]\n    out y : bool\n\n    let _x = a\n    y = true\n}\n";
        assert_eq!(def_ty(src, "_x"), "[bits<4>; 3]");
    }

    #[test]
    fn type_alias_resolves_to_its_target() {
        let src = "type Byte = u8\n\nmodule M {\n    in  a : Byte\n    out y : bool\n\n    let _x = a\n    y = true\n}\n";
        assert_eq!(def_ty(src, "_x"), "u8");
    }
}
