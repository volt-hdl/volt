//! Tip gösterimi ve interning arena'sı (docs/spec/type-inference.md §1).
//!
//! Aynı tip her zaman aynı `TypeId`'yi alır (interning); eşitlik ucuz bir
//! sayı karşılaştırmasıdır. `Ty::Error` kaskad hata bastırma içindir —
//! her tiple uyumlu sayılır ve yeni hata üretmez. `Ty::IntLit` soneksiz
//! tam sayı literalinin geçici tipidir; bağlamdan çözülür (§4).

use std::collections::HashMap;

/// Arena'daki bir tipin opak kimliği.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeId(u32);

/// Kullanıcı tanımlı struct — `DefId.0` taşır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StructId(pub u32);

/// Kullanıcı tanımlı enum — `DefId.0` taşır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnumId(pub u32);

/// Örneklenen modül — `DefId.0` taşır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModuleId(pub u32);

/// Sıfırlama senkronluğu (volt-ast eşleniğinin HIR yansıması;
/// `Ty` Hash gerektirdiği için burada yinelenir).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResetSync {
    Sync,
    Async,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResetPolarity {
    ActiveHigh,
    ActiveLow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResetSpec {
    pub sync: ResetSync,
    pub polarity: ResetPolarity,
}

impl From<volt_ast::ResetSpec> for ResetSpec {
    fn from(spec: volt_ast::ResetSpec) -> Self {
        ResetSpec {
            sync: match spec.sync {
                volt_ast::ResetSync::Sync => ResetSync::Sync,
                volt_ast::ResetSync::Async => ResetSync::Async,
                volt_ast::ResetSync::None => ResetSync::None,
            },
            polarity: match spec.polarity {
                volt_ast::ResetPolarity::ActiveHigh => ResetPolarity::ActiveHigh,
                volt_ast::ResetPolarity::ActiveLow => ResetPolarity::ActiveLow,
            },
        }
    }
}

/// Volt tip evreni (type-inference.md §1).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Ty {
    /// Tek bit mantıksal.
    Bool,
    /// İşaretsiz tam sayı — width bit.
    UInt { width: u16 },
    /// İşaretli tam sayı — width bit (işaret dahil).
    SInt { width: u16 },
    /// Ham bit vektörü — aritmetik YOK.
    Bits { width: u16 },
    /// Dengeli üçlü {-1, 0, +1} — depolama 2 bit.
    Trit,
    /// Saat sinyali.
    Clock,
    /// Sıfırlama sinyali; `None` anotasyonsuz `reset` demektir.
    Reset { spec: Option<ResetSpec> },
    /// Sabit uzunluklu dizi.
    Array { elem: TypeId, len: u32 },
    /// Demet.
    Tuple(Vec<TypeId>),
    /// Kullanıcı tanımlı struct.
    Struct(StructId),
    /// Kullanıcı tanımlı enum.
    Enum(EnumId),
    /// Modül örneği (port erişimi için).
    Instance(ModuleId),
    /// Boyutlandırılmamış tamsayı literali — bağlamdan belirlenir.
    IntLit,
    /// Hata kurtarma — her tiple uyumlu, kaskad bastırır.
    Error,
}

/// Interning arena'sı: aynı `Ty` → aynı `TypeId`.
#[derive(Debug, Default)]
pub struct TypeArena {
    tys: Vec<Ty>,
    interned: HashMap<Ty, TypeId>,
}

impl TypeArena {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn intern(&mut self, ty: Ty) -> TypeId {
        if let Some(&id) = self.interned.get(&ty) {
            return id;
        }
        let id = TypeId(self.tys.len() as u32);
        self.tys.push(ty.clone());
        self.interned.insert(ty, id);
        id
    }

    pub fn ty(&self, id: TypeId) -> &Ty {
        &self.tys[id.0 as usize]
    }

    pub fn error(&mut self) -> TypeId {
        self.intern(Ty::Error)
    }

    pub fn int_lit(&mut self) -> TypeId {
        self.intern(Ty::IntLit)
    }

    pub fn bool_ty(&mut self) -> TypeId {
        self.intern(Ty::Bool)
    }

    pub fn is_error(&self, id: TypeId) -> bool {
        matches!(self.ty(id), Ty::Error)
    }

    pub fn is_int_lit(&self, id: TypeId) -> bool {
        matches!(self.ty(id), Ty::IntLit)
    }

    /// Bit seçimi/aralık için taban genişlik (UInt/SInt/Bits).
    pub fn width_of(&self, id: TypeId) -> Option<u16> {
        match self.ty(id) {
            Ty::UInt { width } | Ty::SInt { width } | Ty::Bits { width } => Some(*width),
            _ => None,
        }
    }

    /// Hata mesajlarındaki kullanıcı yüzü gösterim.
    ///
    /// Struct/enum/instance isimleri arena'da tutulmaz; isimli gösterim
    /// için tip denetçisi `ResolveResult` üzerinden sarmalar.
    pub fn display(&self, id: TypeId) -> String {
        match self.ty(id) {
            Ty::Bool => "bool".to_string(),
            Ty::UInt { width } => format!("u{width}"),
            Ty::SInt { width } => format!("i{width}"),
            Ty::Bits { width } => format!("bits<{width}>"),
            Ty::Trit => "Trit".to_string(),
            Ty::Clock => "clock".to_string(),
            Ty::Reset { .. } => "reset".to_string(),
            Ty::Array { elem, len } => format!("[{}; {len}]", self.display(*elem)),
            Ty::Tuple(items) => {
                let parts: Vec<String> = items.iter().map(|&t| self.display(t)).collect();
                format!("({})", parts.join(", "))
            }
            Ty::Struct(_) => "struct".to_string(),
            Ty::Enum(_) => "enum".to_string(),
            Ty::Instance(_) => "modül örneği".to_string(),
            Ty::IntLit => "tamsayı literali".to_string(),
            Ty::Error => "<hata>".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interning_same_type_same_id() {
        let mut arena = TypeArena::new();
        let a = arena.intern(Ty::UInt { width: 8 });
        let b = arena.intern(Ty::UInt { width: 8 });
        assert_eq!(a, b);
    }

    #[test]
    fn distinct_types_distinct_ids() {
        let mut arena = TypeArena::new();
        let a = arena.intern(Ty::UInt { width: 8 });
        let b = arena.intern(Ty::UInt { width: 9 });
        let c = arena.intern(Ty::SInt { width: 8 });
        assert_ne!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn display_builtin_types() {
        let mut arena = TypeArena::new();
        let u8_ty = arena.intern(Ty::UInt { width: 8 });
        let i9 = arena.intern(Ty::SInt { width: 9 });
        let bits4 = arena.intern(Ty::Bits { width: 4 });
        let trit = arena.intern(Ty::Trit);
        assert_eq!(arena.display(u8_ty), "u8");
        assert_eq!(arena.display(i9), "i9");
        assert_eq!(arena.display(bits4), "bits<4>");
        assert_eq!(arena.display(trit), "Trit");
    }

    #[test]
    fn display_array_and_tuple() {
        let mut arena = TypeArena::new();
        let elem = arena.intern(Ty::Bool);
        let arr = arena.intern(Ty::Array { elem, len: 4 });
        let u8_ty = arena.intern(Ty::UInt { width: 8 });
        let tup = arena.intern(Ty::Tuple(vec![u8_ty, elem]));
        assert_eq!(arena.display(arr), "[bool; 4]");
        assert_eq!(arena.display(tup), "(u8, bool)");
    }

    #[test]
    fn width_of_numeric_types() {
        let mut arena = TypeArena::new();
        let u8_ty = arena.intern(Ty::UInt { width: 8 });
        let bits4 = arena.intern(Ty::Bits { width: 4 });
        let b = arena.intern(Ty::Bool);
        assert_eq!(arena.width_of(u8_ty), Some(8));
        assert_eq!(arena.width_of(bits4), Some(4));
        assert_eq!(arena.width_of(b), None);
    }

    #[test]
    fn error_and_int_lit_helpers() {
        let mut arena = TypeArena::new();
        let e = arena.error();
        let l = arena.int_lit();
        assert!(arena.is_error(e));
        assert!(arena.is_int_lit(l));
        assert!(!arena.is_error(l));
    }

    #[test]
    fn reset_spec_converts_from_ast() {
        let ast_spec = volt_ast::ResetSpec {
            sync: volt_ast::ResetSync::Async,
            polarity: volt_ast::ResetPolarity::ActiveLow,
        };
        let hir: ResetSpec = ast_spec.into();
        assert_eq!(hir.sync, ResetSync::Async);
        assert_eq!(hir.polarity, ResetPolarity::ActiveLow);
    }
}
