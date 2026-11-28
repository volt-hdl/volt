//! Tip denetiminin ifadelere verdiği genişlik ve işaret, AST'yi okuyan arka
//! uçlar için (#413, ADR-0137 §2, ADR-0141).
//!
//! Arka uçlar `volt-hir`'e bağlı değildir (ADR-0136 Aşama 3'e kadar
//! `volt-ast`'i okurlar). Tip denetiminin cevabı onlara bu tabloyla gelir:
//! `volt-hir` doldurur (`TypeckResult::checked_types`), sürücü üreticiye
//! verir, üretici okur. Tablo bir hesap yapmaz; genişlik
//! `TypeArena::bit_width`'ten, işaret `TypeArena::is_signed`'dan gelir.
//!
//! Anahtar kök AST'nin ifade indeksidir. Üretici fn açılımında (ADR-0081)
//! ve struct indirgemesinde (ADR-0077) AST'nin bir kopyası üzerinde
//! çalışır; kopya kök ifadeleri aynı indekste ve aynı konumda tutar, yeni
//! düğümleri sona ekler. Bu yüzden arama konumu da ister: kopyada aynı
//! indeksli ama başka konumlu düğüm kök ifade değildir.
//!
//! Genişlik ve işaret Trit'i i2'den ayırmaz; tipi Trit olan ifadeler ayrıca
//! işaretlenir ([`CheckedTypes::is_trit`]). Üretici Trit'i sözdiziminden
//! tanır (ADR-0062); literal ise tipini yalnız tip denetiminden alır: generic
//! `const S : Trit` parametresinin yerine geçen literal Trit'tir (#468).

use std::collections::HashMap;

use volt_span::Span;

use crate::{Expr, Idx};

/// Bir ifadenin tip denetimindeki değeri, arka ucun okuduğu biçimde.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedValue {
    /// Genişliği olan tek sinyal: tam sayı, `bits`, `bool`, `Trit`, enum,
    /// paketlenmiş dizi (ADR-0056); esnek tipte doğal genişlik (ADR-0025).
    Bits { width: u32, signed: bool },
    /// Düz struct (ADR-0077): alanları [`CheckedTypes::field`] ile okunur.
    Struct(StructKey),
    /// Genişliği olmayan tip: boyutsuz literal, demet, örnek, hata.
    NoWidth,
}

/// Tablodaki bir struct tipinin kimliği.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StructKey(pub u32);

/// Tip denetiminin cevapları ([modül belgesi](self)).
#[derive(Debug, Clone, Default)]
pub struct CheckedTypes {
    exprs: HashMap<Idx<Expr>, (Span, CheckedValue)>,
    structs: HashMap<StructKey, Vec<(String, CheckedValue)>>,
    /// Tipi Trit olan ifadeler ve konumları.
    trits: HashMap<Idx<Expr>, Span>,
}

impl CheckedTypes {
    /// Kök AST ifadesinin değeri.
    pub fn insert_expr(&mut self, expr: Idx<Expr>, span: Span, value: CheckedValue) {
        self.exprs.insert(expr, (span, value));
    }

    /// Kök AST ifadesinin tipi Trit'tir.
    pub fn insert_trit(&mut self, expr: Idx<Expr>, span: Span) {
        self.trits.insert(expr, span);
    }

    /// İfadenin tipi Trit mi: indeksi ve konumu kök AST'deki Trit tipli bir
    /// ifadeninkiyse.
    pub fn is_trit(&self, expr: Idx<Expr>, span: Span) -> bool {
        self.trits.get(&expr) == Some(&span)
    }

    /// Struct tipinin alanları, bildirim sırasıyla.
    pub fn insert_struct(&mut self, key: StructKey, fields: Vec<(String, CheckedValue)>) {
        self.structs.insert(key, fields);
    }

    /// İfadenin değeri: indeksi ve konumu kök AST'deki bir ifadeninkiyse.
    pub fn expr(&self, expr: Idx<Expr>, span: Span) -> Option<CheckedValue> {
        match self.exprs.get(&expr) {
            Some(&(s, value)) if s == span => Some(value),
            _ => None,
        }
    }

    /// Struct değerinin `path` alan zincirindeki yaprağı (`["i", "x"]`:
    /// `v.i.x`).
    pub fn field(&self, value: CheckedValue, path: &[String]) -> Option<CheckedValue> {
        let mut cur = value;
        for name in path {
            let CheckedValue::Struct(key) = cur else {
                return None;
            };
            cur = self
                .structs
                .get(&key)?
                .iter()
                .find(|(n, _)| n == name)
                .map(|&(_, v)| v)?;
        }
        Some(cur)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::Arena;
    use crate::ExprKind;
    use volt_span::FileId;

    fn exprs() -> (Arena<Expr>, Idx<Expr>, Idx<Expr>) {
        let mut a = Arena::default();
        let x = a.alloc(Expr {
            span: Span::new(FileId(0), 0, 1),
            kind: ExprKind::Error,
        });
        let y = a.alloc(Expr {
            span: Span::new(FileId(0), 2, 3),
            kind: ExprKind::Error,
        });
        (a, x, y)
    }

    #[test]
    fn an_expression_is_found_only_at_its_own_span() {
        let (a, x, y) = exprs();
        let mut t = CheckedTypes::default();
        let v = CheckedValue::Bits {
            width: 9,
            signed: true,
        };
        t.insert_expr(x, a[x].span, v);
        assert_eq!(t.expr(x, a[x].span), Some(v));
        assert_eq!(t.expr(x, a[y].span), None, "kopyada başka konumlu düğüm");
        assert_eq!(t.expr(y, a[y].span), None, "kayıtsız ifade");
    }

    #[test]
    fn a_trit_is_found_only_at_its_own_span() {
        let (a, x, y) = exprs();
        let mut t = CheckedTypes::default();
        t.insert_trit(x, a[x].span);
        assert!(t.is_trit(x, a[x].span));
        assert!(!t.is_trit(x, a[y].span), "kopyada başka konumlu düğüm");
        assert!(!t.is_trit(y, a[y].span), "kayıtsız ifade");
    }

    #[test]
    fn a_field_chain_walks_nested_structs() {
        let mut t = CheckedTypes::default();
        let leaf = CheckedValue::Bits {
            width: 5,
            signed: true,
        };
        t.insert_struct(StructKey(1), vec![("x".to_string(), leaf)]);
        t.insert_struct(
            StructKey(2),
            vec![
                ("ok".to_string(), CheckedValue::NoWidth),
                ("i".to_string(), CheckedValue::Struct(StructKey(1))),
            ],
        );
        let outer = CheckedValue::Struct(StructKey(2));
        let path = |p: &[&str]| p.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(t.field(outer, &path(&["i", "x"])), Some(leaf));
        assert_eq!(t.field(outer, &path(&[])), Some(outer));
        assert_eq!(t.field(outer, &path(&["i", "y"])), None);
        assert_eq!(
            t.field(leaf, &path(&["x"])),
            None,
            "struct olmayanın alanı yok"
        );
        assert_eq!(
            t.field(CheckedValue::Struct(StructKey(3)), &path(&["x"])),
            None,
            "tabloda olmayan struct"
        );
    }
}
