//! Enum kodlama tablosu (ADR-0074 Karar 2).
//!
//! Birim varyantlı bir enum düz bir bit vektörüne iner: varsayılan
//! kodlama bildirim sırasıyla 0'dan, genişlik `max(1, clog2(n))`; açık
//! değerli enum'da değerler yazıldığı gibi, genişlik taban tipinden ya da
//! en büyük değerin bit uzunluğundan gelir. Kural tek yerde: volt-hir
//! tanıları (E2030/E2010/E2021), sv-emit `localparam`/sinyal genişliği ve
//! parser'ın otomatik kontratları (F1 "durum geçerli") aynı tabloyu
//! kullanır. Sabit değerlendirme çağırana bırakılır (`eval`) — her katmanın
//! kendi sabit değerlendiricisi vardır.

use crate::{EnumDecl, Expr, Idx, ItemKind, SourceFile, TypeRef, TypeRefKind, VariantData};

/// Geçerli bir enum'un donanım kodlaması.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumLayout {
    /// Sinyal genişliği (bit).
    pub width: u32,
    /// Varyant başına kod, bildirim sırasıyla.
    pub values: Vec<u128>,
}

impl EnumLayout {
    /// Bütün `2^width` kodlar bir varyanta ait mi (F1 totoloji olur).
    pub fn is_dense(&self) -> bool {
        self.width < 32 && self.values.len() as u128 == 1u128 << self.width
    }
}

/// Taban tipinin (`enum E : T`) çözümü.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repr {
    /// Taban tipi yazılmamış.
    None,
    /// İşaretsiz `uN`, `uint<N>` ya da `bits<N>`.
    Width(u32),
    /// Taban tipi işaretsiz tamsayı ya da ham bit değil (işaretli, bool,
    /// kullanıcı tipi ...).
    Invalid,
    /// Genişlik sabiti değerlendirilemedi (tanısı sabit denetiminde).
    Unknown,
    /// Taban tipi bir tip döngüsünde (ADR-0069 E4009 zaten verilir).
    Cycle,
}

/// Kodlama kuralı ihlali — tanıyı çağıran katman üretir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutIssue {
    /// Varyantsız enum.
    Empty,
    /// Bazı varyantlar açık değerli, bazıları değil (`first_implicit`
    /// açık değeri olmayan ilk varyantın sırası).
    Mixed { first_implicit: usize },
    /// Açık değer sabit değil (sıra).
    NonConst(usize),
    /// Açık değer negatif (sıra, değer).
    Negative(usize, i128),
    /// Aynı kodu taşıyan iki varyant (ilk, ikinci, değer).
    Duplicate(usize, usize, u128),
    /// Taban tipi işaretsiz tamsayı/ham bit değil.
    InvalidRepr,
    /// Taban tipi değer sayısına yetmiyor (açık değersizde).
    ReprTooNarrow { needed: u32, width: u32 },
    /// Açık değer taban tipine sığmıyor (sıra, değer, genişlik).
    ValueTooWide(usize, u128, u32),
}

/// Payload taşıyan ilk varyant (`Load(u8)`, `Node { .. }`) — bu tür
/// enum'lar sinyal tipi olamaz (ADR-0074 Karar 1).
pub fn payload_variant(decl: &EnumDecl) -> Option<&crate::EnumVariant> {
    decl.variants
        .iter()
        .find(|v| !matches!(v.data, VariantData::Unit))
}

/// Ada göre enum bildirimi (birleşik birimde öğe adları tekildir).
pub fn enum_named<'a>(ast: &'a SourceFile, name: &str) -> Option<&'a EnumDecl> {
    ast.items
        .iter()
        .find_map(|&i| match &ast.items_arena[i].kind {
            ItemKind::Enum(e) if e.name.text == name => Some(e),
            _ => None,
        })
}

/// Tip `Path` ise (generic olmayan takma adlar izlenerek) adlandırdığı
/// enum bildirimi.
pub fn enum_of_type(ast: &SourceFile, ty: Idx<TypeRef>) -> Option<&EnumDecl> {
    let target = follow_aliases(ast, ty)?;
    // Generic argümanlı yol da enum'u adlandırır (`G<u8>`); generic enum
    // sinyal tipi olarak E0003'tür, karar çağıranda.
    let TypeRefKind::Path { path, .. } = &ast.types[target].kind else {
        return None;
    };
    enum_named(ast, &path.segments.last()?.text)
}

/// Takma ad zincirinin sonu; döngüde `None`.
fn follow_aliases(ast: &SourceFile, ty: Idx<TypeRef>) -> Option<Idx<TypeRef>> {
    const MAX_ALIAS_DEPTH: usize = 64;
    let mut cur = ty;
    for _ in 0..MAX_ALIAS_DEPTH {
        let TypeRefKind::Path { path, args } = &ast.types[cur].kind else {
            return Some(cur);
        };
        let [seg] = path.segments.as_slice() else {
            return Some(cur);
        };
        if !args.is_empty() {
            return Some(cur);
        }
        let target = ast
            .items
            .iter()
            .find_map(|&i| match &ast.items_arena[i].kind {
                crate::ItemKind::TypeAlias(a)
                    if a.name.text == seg.text && a.generics.is_empty() =>
                {
                    Some(a.target)
                }
                _ => None,
            });
        match target {
            Some(t) => cur = t,
            None => return Some(cur),
        }
    }
    None
}

/// `enum E : T`'nin taban tipi. Kullanıcı tipine (takma ad dışında)
/// çözülen ya da takma ad döngüsündeki taban tipi: kendisini ya da
/// başka bir enum/struct'ı adlandırıyorsa `Cycle`/`Invalid`.
pub fn repr_of(
    ast: &SourceFile,
    decl: &EnumDecl,
    eval: &mut dyn FnMut(Idx<Expr>) -> Option<i128>,
) -> Repr {
    let Some(ty) = decl.repr else {
        return Repr::None;
    };
    let Some(target) = follow_aliases(ast, ty) else {
        return Repr::Cycle;
    };
    let width = |n: Option<i128>| match n {
        Some(n) if (1..=i128::from(u16::MAX)).contains(&n) => Repr::Width(n as u32),
        Some(_) => Repr::Invalid,
        None => Repr::Unknown,
    };
    match &ast.types[target].kind {
        TypeRefKind::UInt(n) => Repr::Width(u32::from(*n)),
        TypeRefKind::Bits(e) | TypeRefKind::UIntN(e) => width(eval(*e)),
        TypeRefKind::Path { path, .. } => {
            let name = path.segments.last().map_or("", |s| s.text.as_str());
            if name == decl.name.text || reaches_enum(ast, name, &decl.name.text) {
                Repr::Cycle
            } else {
                Repr::Invalid
            }
        }
        TypeRefKind::Error => Repr::Unknown,
        _ => Repr::Invalid,
    }
}

/// `name` enum'unun taban tipi zinciri `target` enum'una varıyor mu
/// (E4009 döngüsü; ADR-0069 tanıyı verir).
fn reaches_enum(ast: &SourceFile, name: &str, target: &str) -> bool {
    let mut cur = name.to_string();
    for _ in 0..64 {
        let Some(e) = enum_named(ast, &cur) else {
            return false;
        };
        let Some(ty) = e.repr.and_then(|t| follow_aliases(ast, t)) else {
            return e.repr.is_some();
        };
        let TypeRefKind::Path { path, .. } = &ast.types[ty].kind else {
            return false;
        };
        let next = path.segments.last().map_or("", |s| s.text.as_str());
        if next == target {
            return true;
        }
        cur = next.to_string();
    }
    true
}

/// En küçük `w ≥ 1` öyle ki `v < 2^w`.
pub fn bit_len(v: u128) -> u32 {
    (128 - v.leading_zeros()).max(1)
}

/// `n` farklı kod için gereken genişlik: `max(1, clog2(n))`.
pub fn width_for_count(n: usize) -> u32 {
    if n <= 1 {
        1
    } else {
        bit_len(n as u128 - 1)
    }
}

/// Kodlama tablosu. `repr` [`repr_of`]'tan; `eval` açık değerleri
/// değerlendirir. Payload'lı ya da generic enum çağıranın işidir (bu
/// fonksiyon yalnız birim varyantların kodunu hesaplar).
pub fn layout(
    decl: &EnumDecl,
    repr: Repr,
    eval: &mut dyn FnMut(Idx<Expr>) -> Option<i128>,
) -> Result<EnumLayout, Vec<LayoutIssue>> {
    let mut issues = Vec::new();
    let n = decl.variants.len();
    if n == 0 {
        return Err(vec![LayoutIssue::Empty]);
    }
    let explicit = decl
        .variants
        .iter()
        .filter(|v| v.discriminant.is_some())
        .count();
    let values: Vec<u128> = if explicit == 0 {
        (0..n as u128).collect()
    } else if explicit < n {
        let first_implicit = decl
            .variants
            .iter()
            .position(|v| v.discriminant.is_none())
            .unwrap_or(0);
        return Err(vec![LayoutIssue::Mixed { first_implicit }]);
    } else {
        let mut out = Vec::with_capacity(n);
        for (i, v) in decl.variants.iter().enumerate() {
            match v.discriminant.and_then(&mut *eval) {
                None => issues.push(LayoutIssue::NonConst(i)),
                Some(x) if x < 0 => issues.push(LayoutIssue::Negative(i, x)),
                Some(x) => out.push(x as u128),
            }
        }
        if !issues.is_empty() {
            return Err(issues);
        }
        out
    };
    for (j, &b) in values.iter().enumerate() {
        if let Some(i) = values[..j].iter().position(|&a| a == b) {
            issues.push(LayoutIssue::Duplicate(i, j, b));
        }
    }
    let natural = if explicit == 0 {
        width_for_count(n)
    } else {
        values.iter().map(|&v| bit_len(v)).max().unwrap_or(1)
    };
    let width = match repr {
        Repr::None | Repr::Unknown | Repr::Cycle => natural,
        Repr::Invalid => {
            issues.push(LayoutIssue::InvalidRepr);
            natural
        }
        Repr::Width(w) => {
            if explicit == 0 && w < natural {
                issues.push(LayoutIssue::ReprTooNarrow {
                    needed: natural,
                    width: w,
                });
            }
            if explicit > 0 {
                for (i, &v) in values.iter().enumerate() {
                    if bit_len(v) > w {
                        issues.push(LayoutIssue::ValueTooWide(i, v, w));
                    }
                }
            }
            w
        }
    };
    if issues.is_empty() {
        Ok(EnumLayout { width, values })
    } else {
        Err(issues)
    }
}

/// Katmanların ortak kısayolu: geçerli bir birim varyantlı, generic
/// olmayan enum'un tablosu; aksi hâlde `None` (tanı HIR'dadır).
pub fn valid_layout(
    ast: &SourceFile,
    decl: &EnumDecl,
    eval: &mut dyn FnMut(Idx<Expr>) -> Option<i128>,
) -> Option<EnumLayout> {
    if !decl.generics.is_empty() || payload_variant(decl).is_some() {
        return None;
    }
    let repr = repr_of(ast, decl, eval);
    layout(decl, repr, eval).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_for_count_is_clog2_at_least_one() {
        assert_eq!(width_for_count(1), 1);
        assert_eq!(width_for_count(2), 1);
        assert_eq!(width_for_count(3), 2);
        assert_eq!(width_for_count(4), 2);
        assert_eq!(width_for_count(5), 3);
        assert_eq!(width_for_count(8), 3);
        assert_eq!(width_for_count(9), 4);
    }

    #[test]
    fn bit_len_counts_zero_as_one_bit() {
        assert_eq!(bit_len(0), 1);
        assert_eq!(bit_len(1), 1);
        assert_eq!(bit_len(8), 4);
        assert_eq!(bit_len(0b0110011), 6);
    }

    #[test]
    fn dense_means_every_code_is_a_variant() {
        let full = EnumLayout {
            width: 2,
            values: vec![0, 1, 2, 3],
        };
        let three = EnumLayout {
            width: 2,
            values: vec![0, 1, 2],
        };
        assert!(full.is_dense());
        assert!(!three.is_dense());
    }
}
