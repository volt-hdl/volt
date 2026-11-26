//! Tip denetiminin cevabı, AST'yi okuyan arka uçlar için (#413, ADR-0137
//! §2, ADR-0141): `volt_ast::checked::CheckedTypes`'ı dolduran tek yer.
//!
//! Genişlik `TypeArena::bit_width`'ten, işaret `TypeArena::is_signed`'dan
//! gelir; burada başka bir kural yoktur. Struct tipli ifade alanlarıyla
//! birlikte yazılır, böylece üretici struct indirgemesinin (ADR-0077)
//! yeni yaprak düğümlerinin tipini kök ifadenin tipinden okuyabilir.

use std::collections::HashMap;

use volt_ast::checked::{CheckedTypes, CheckedValue, StructKey};
use volt_ast::SourceFile;

use super::TypeckResult;
use crate::ty::{StructId, Ty, TypeArena, TypeId};

impl TypeckResult {
    /// Kök AST'nin tip denetiminden geçen her ifadesinin değeri.
    pub fn checked_types(&self, ast: &SourceFile) -> CheckedTypes {
        let mut out = CheckedTypes::default();
        let mut memo: HashMap<TypeId, CheckedValue> = HashMap::new();
        for (&expr, &ty) in &self.expr_types {
            let value = checked_value(&self.types, ty, &mut out, &mut memo);
            out.insert_expr(expr, ast.exprs[expr].span, value);
        }
        out
    }
}

/// Bir tipin tablodaki değeri; struct tipinin alanlarını da tabloya yazar.
/// `Delayed<T, N>` `T`'ninki (`bit_width` ve `is_signed` içe iner).
fn checked_value(
    types: &TypeArena,
    ty: TypeId,
    out: &mut CheckedTypes,
    memo: &mut HashMap<TypeId, CheckedValue>,
) -> CheckedValue {
    if let Some(&v) = memo.get(&ty) {
        return v;
    }
    let value = match types.ty(ty) {
        Ty::Struct(s) => struct_value(types, *s, out, memo),
        Ty::Bool
        | Ty::UInt { .. }
        | Ty::SInt { .. }
        | Ty::Bits { .. }
        | Ty::Trit
        | Ty::Clock
        | Ty::Reset { .. }
        | Ty::Array { .. }
        | Ty::Tuple(_)
        | Ty::Enum(_)
        | Ty::Instance(_)
        | Ty::Builtin { .. }
        | Ty::Delayed { .. }
        | Ty::IntLit
        | Ty::UIntFlex { .. }
        | Ty::SIntFlex { .. }
        | Ty::Error => match types.bit_width(ty) {
            Some(width) => CheckedValue::Bits {
                width,
                signed: types.is_signed(ty),
            },
            None => CheckedValue::NoWidth,
        },
    };
    memo.insert(ty, value);
    value
}

/// Düzeni geçerli struct: alanlarıyla; değilse genişliksiz. Her struct
/// tipi bir kez yazılır (`checked_value`'nun `TypeId` önbelleği); düzeni
/// geçerli struct kendini, doğrudan ya da dolaylı, içeremez.
fn struct_value(
    types: &TypeArena,
    s: StructId,
    out: &mut CheckedTypes,
    memo: &mut HashMap<TypeId, CheckedValue>,
) -> CheckedValue {
    let Some(fields) = types.struct_fields(s) else {
        return CheckedValue::NoWidth;
    };
    let key = StructKey(s.0);
    let fields: Vec<(String, CheckedValue)> = fields
        .iter()
        .map(|(name, t)| (name.clone(), checked_value(types, *t, out, memo)))
        .collect();
    out.insert_struct(key, fields);
    CheckedValue::Struct(key)
}

#[cfg(test)]
mod tests {
    use volt_ast::checked::CheckedValue;
    use volt_ast::{ExprKind, ItemKind};

    use crate::typeck::testutil::check;

    /// Kontrattaki `prev(..)` çağrılarının argümanlarının değerleri,
    /// kaynak sırasıyla.
    fn prev_args(src: &str) -> Vec<CheckedValue> {
        let (ast, result) = check(src);
        let table = result.checked_types(&ast);
        let mut out = Vec::new();
        for &item in &ast.items {
            let ItemKind::Module(m) = &ast.items_arena[item].kind else {
                continue;
            };
            for c in &m.contracts {
                let mut stack = vec![c.expr];
                while let Some(e) = stack.pop() {
                    let kind = &ast.exprs[e].kind;
                    if let ExprKind::Call { args, .. } = kind {
                        let x = args[0];
                        out.push(table.expr(x, ast.exprs[x].span).expect("argüman tabloda"));
                    }
                    stack.extend(volt_ast::visit::expr_children(kind));
                }
            }
        }
        out
    }

    fn bits(width: u32, signed: bool) -> CheckedValue {
        CheckedValue::Bits { width, signed }
    }

    #[test]
    fn widths_and_signs_come_from_the_type_checker() {
        let src = "
type Word = u12
struct P {
    x : u7
    y : i5
}
module M {
    in  clk : clock
    in  w   : Word
    in  p   : P
    in  t   : Trit
    in  sa  : i8
    in  a   : u8
    in  b   : u8
    out y   : u8
    y = a
    invariant: prev(w) != 0
    invariant: prev(p.y) < 0
    invariant: prev(t) == t
    invariant: prev(sa) < 0
    invariant: prev(a + b) > 255
}
";
        // Elle: Word u12; p.y i5; Trit 2 bit işaretli (ADR-0062); i8;
        // u8 + u8 esnek [8, 9], doğal genişlik 9 (ADR-0025).
        assert_eq!(
            prev_args(src),
            vec![
                bits(12, false),
                bits(5, true),
                bits(2, true),
                bits(8, true),
                bits(9, false),
            ]
        );
    }

    #[test]
    fn a_struct_value_carries_its_fields() {
        let src = "
struct In {
    x : i3
}
struct P {
    ok : bool
    i  : In
}
module M {
    in  clk : clock
    in  p   : P
    out y   : bool
    y = p.ok
    invariant: prev(p) == p
}
";
        let (ast, result) = check(src);
        let table = result.checked_types(&ast);
        let [v] = prev_args(src)[..] else {
            panic!("bir prev bekleniyor");
        };
        assert!(matches!(v, CheckedValue::Struct(_)), "{v:?}");
        let path = |p: &[&str]| p.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(table.field(v, &path(&["ok"])), Some(bits(1, false)));
        assert_eq!(table.field(v, &path(&["i", "x"])), Some(bits(3, true)));
        assert_eq!(table.field(v, &path(&["i", "y"])), None);
        let _ = ast;
    }

    #[test]
    fn an_unsized_literal_has_no_width() {
        let src = "
module M {
    in  clk : clock
    in  a   : u8
    out y   : u8
    y = a
    invariant: prev(5) == a
}
";
        assert_eq!(prev_args(src), vec![CheckedValue::NoWidth]);
    }
}
