//! Struct bit düzeni (ADR-0077 Karar 3, normatif).
//!
//! Düz bir `struct` değeri, alanlarının bildirim sırasıyla derinlik
//! öncelikli yaprak listesidir; **ilk yaprak en anlamlı bitlerdedir**
//! (SV `typedef struct packed` ile aynı). `W` yaprak genişliklerinin
//! toplamıdır. Yaprak içi: sayısal alan kendi ikili gösterimi, `bool` 1
//! bit, `Trit` 2 bit (ADR-0003), enum ADR-0074 kodu, dizi alanı ADR-0056
//! paketlenmiş vektörü.
//!
//! Kural tek yerde: volt-hir (bildirim denetimi, sürücü analizi, `as`
//! kuralları, sinyal genişliği), sv-emit (yaprak sinyalleri, `p as uN`,
//! `raw as P`), simülasyon raporu ve LSP hover aynı tabloyu kullanır.
//! Sabit değerlendirme çağırana bırakılır (`eval`).

use crate::enum_layout;
use crate::{Expr, Idx, ItemKind, SourceFile, StructDecl, TypeRef, TypeRefKind};

/// Düzleştirme bütçesi (ADR-0067 sınırlarıyla aynı): yaprak sayısı.
/// Döngüsüz ama elmas biçimli iç içelik (`A { x: B, y: B }`) yaprakları
/// üstel çoğaltır; aşım E4010'dur.
pub const MAX_LEAVES: usize = 4096;

/// İç içe struct derinlik sınırı (ADR-0067): aşım E4010. Kendine
/// başvuran tanım (E4009) buraya gelmeden elenir; sınır yığını korur.
pub const MAX_NESTING: usize = 8;

/// Yaprağın donanım gösterimi türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeafKind {
    /// `uN`, `iN`, `bits<N>`, `bool`, dizi.
    Plain,
    /// Birim varyantlı enum (ADR-0074) — her kod geçerli değildir.
    Enum,
    /// `Trit` (ADR-0003) — `10` kodu geçersizdir.
    Trit,
}

/// Düzenin bir yaprağı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Leaf {
    /// Alan yolu, dıştan içe (`["i", "x"]`).
    pub path: Vec<String>,
    /// Yaprak alanının bildirilen tipi (takma ad çözülmeden).
    pub ty: Idx<TypeRef>,
    /// Bit genişliği.
    pub width: u32,
    /// En düşük bitin konumu: yaprak `[lsb + width - 1 : lsb]`.
    pub lsb: u32,
    pub kind: LeafKind,
    /// İşaretli sayısal alan (`iN`, `int<N>`) ya da `Trit`.
    pub signed: bool,
}

impl Leaf {
    /// Noktalı yol (`i.x`).
    pub fn dotted(&self) -> String {
        self.path.join(".")
    }

    /// SV yaprak sinyalinin soneki (`i_x`) — ADR-0039 §14 kuralı.
    pub fn suffix(&self) -> String {
        self.path.join("_")
    }

    /// En yüksek bit.
    pub fn msb(&self) -> u32 {
        self.lsb + self.width - 1
    }

    /// `[11:8]` ya da tek bitte `[5]`.
    pub fn bits(&self) -> String {
        if self.width == 1 {
            format!("[{}]", self.lsb)
        } else {
            format!("[{}:{}]", self.msb(), self.lsb)
        }
    }
}

/// Geçerli bir struct'ın bit düzeni.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructLayout {
    /// Struct'ın adı.
    pub name: String,
    /// Toplam genişlik `W`.
    pub width: u32,
    /// Yapraklar, bildirim sırasıyla (ilki MSB).
    pub leaves: Vec<Leaf>,
}

impl StructLayout {
    /// Yapraklardan biri enum ya da `Trit` mi (`bits → struct` yasağı,
    /// ADR-0077 Karar 4).
    pub fn has_enum_or_trit(&self) -> bool {
        self.leaves.iter().any(|l| l.kind != LeafKind::Plain)
    }

    /// `prefix` alan yolunun altındaki yapraklar (boş önek: hepsi).
    pub fn leaves_under<'a>(&'a self, prefix: &'a [String]) -> impl Iterator<Item = &'a Leaf> {
        self.leaves
            .iter()
            .filter(move |l| l.path.len() >= prefix.len() && l.path[..prefix.len()] == *prefix)
    }

    /// Alan yolunun kapladığı bitler `(lsb, genişlik)`; bir alanın
    /// yaprakları bitişiktir. Yol yoksa `None`.
    pub fn field_bits(&self, prefix: &[String]) -> Option<(u32, u32)> {
        let mut it = self.leaves_under(prefix);
        let first = it.next()?;
        let lsb = it.fold(first.lsb, |_, l| l.lsb);
        Some((lsb, first.msb() + 1 - lsb))
    }

    /// Belge satırı: `a[11:8] s[7:6] b[5] i.x[4:2] i.y[1:0]`.
    pub fn describe(&self) -> String {
        self.leaves
            .iter()
            .map(|l| format!("{}{}", l.dotted(), l.bits()))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Düzenin kurulamama nedeni — tanıyı çağıran katman üretir (çoğu
/// bildirim denetiminde E2xxx/E4010, sinyal tipinde E0003).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutError {
    /// Generic struct (ADR-0069: sinyal tipi olarak E0003).
    Generic,
    /// Alansız struct.
    Empty,
    /// Alan tipi bir `struct port` bundle'ı (alan adı).
    BundleField(String),
    /// Alan tipi bir struct dizisi (alan adı) — ertelendi (Karar 2).
    ArrayOfStruct(String),
    /// Alan tipi desteklenmeyen bir tip (alan adı): enum dizisi,
    /// payload'lı/generic enum, tuple, clock/reset, bilinmeyen ad.
    Unsupported(String),
    /// Genişlik sabiti değerlendirilemedi ya da geçersiz enum kodlaması
    /// (tanısı kendi denetiminde).
    UnknownWidth,
    /// İç içe struct `MAX_NESTING`'i aşıyor ya da döngü (E4009).
    TooDeep,
    /// Yaprak sayısı `MAX_LEAVES`'i aşıyor.
    TooManyLeaves,
}

/// Ada göre düz (`struct port` olmayan) struct bildirimi (birleşik
/// birimde öğe adları tekildir).
pub fn struct_named<'a>(ast: &'a SourceFile, name: &str) -> Option<&'a StructDecl> {
    ast.items
        .iter()
        .find_map(|&i| match &ast.items_arena[i].kind {
            ItemKind::Struct(s) if s.name.text == name && !s.is_port => Some(s),
            _ => None,
        })
}

/// Tip `Path` ise (generic olmayan takma adlar izlenerek) adlandırdığı
/// düz struct bildirimi. Generic argümanlı yol da struct'ı adlandırır —
/// generic struct sinyal tipi kararı çağıranda (E0003).
pub fn struct_of_type(ast: &SourceFile, ty: Idx<TypeRef>) -> Option<&StructDecl> {
    let target = enum_layout::follow_aliases(ast, ty)?;
    let TypeRefKind::Path { path, .. } = &ast.types[target].kind else {
        return None;
    };
    struct_named(ast, &path.segments.last()?.text)
}

/// Bit düzeni. `eval` genişlik sabitlerini değerlendirir.
pub fn layout(
    ast: &SourceFile,
    decl: &StructDecl,
    eval: &mut dyn FnMut(Idx<Expr>) -> Option<i128>,
) -> Result<StructLayout, LayoutError> {
    let mut leaves = Vec::new();
    collect(ast, decl, &mut Vec::new(), 1, eval, &mut leaves)?;
    // Genişlikler toplandı; LSB'ler sondan başa (ilk yaprak MSB).
    let mut lsb = 0u32;
    for leaf in leaves.iter_mut().rev() {
        leaf.lsb = lsb;
        lsb = lsb
            .checked_add(leaf.width)
            .ok_or(LayoutError::TooManyLeaves)?;
    }
    Ok(StructLayout {
        name: decl.name.text.clone(),
        width: lsb,
        leaves,
    })
}

fn collect(
    ast: &SourceFile,
    decl: &StructDecl,
    prefix: &mut Vec<String>,
    depth: usize,
    eval: &mut dyn FnMut(Idx<Expr>) -> Option<i128>,
    out: &mut Vec<Leaf>,
) -> Result<(), LayoutError> {
    if depth > MAX_NESTING {
        return Err(LayoutError::TooDeep);
    }
    if !decl.generics.is_empty() {
        return Err(LayoutError::Generic);
    }
    if decl.fields.is_empty() {
        return Err(LayoutError::Empty);
    }
    for field in &decl.fields {
        prefix.push(field.name.text.clone());
        let result = leaf_or_nested(ast, field.ty, prefix, depth, eval, out);
        prefix.pop();
        result?;
        if out.len() > MAX_LEAVES {
            return Err(LayoutError::TooManyLeaves);
        }
    }
    Ok(())
}

fn leaf_or_nested(
    ast: &SourceFile,
    ty: Idx<TypeRef>,
    prefix: &mut Vec<String>,
    depth: usize,
    eval: &mut dyn FnMut(Idx<Expr>) -> Option<i128>,
    out: &mut Vec<Leaf>,
) -> Result<(), LayoutError> {
    let field = prefix.join(".");
    let target = enum_layout::follow_aliases(ast, ty).ok_or(LayoutError::TooDeep)?;
    let width = |n: Option<i128>| match n {
        Some(n) if (1..=i128::from(u16::MAX)).contains(&n) => Ok(n as u32),
        _ => Err(LayoutError::UnknownWidth),
    };
    let (w, kind, signed) = match &ast.types[target].kind {
        TypeRefKind::Bool => (1, LeafKind::Plain, false),
        TypeRefKind::UInt(n) => (u32::from(*n), LeafKind::Plain, false),
        TypeRefKind::SInt(n) => (u32::from(*n), LeafKind::Plain, true),
        TypeRefKind::Bits(e) | TypeRefKind::UIntN(e) => (width(eval(*e))?, LeafKind::Plain, false),
        TypeRefKind::SIntN(e) => (width(eval(*e))?, LeafKind::Plain, true),
        TypeRefKind::Trit => (2, LeafKind::Trit, true),
        TypeRefKind::Array { elem, len } => {
            let elem_w = array_elem_width(ast, *elem, &field, eval)?;
            let n = width(eval(*len))?;
            (
                elem_w.checked_mul(n).ok_or(LayoutError::UnknownWidth)?,
                LeafKind::Plain,
                false,
            )
        }
        TypeRefKind::Path { path, .. } => {
            let name = path.segments.last().map_or("", |s| s.text.as_str());
            if let Some(inner) = struct_named(ast, name) {
                return collect(ast, inner, prefix, depth + 1, eval, out);
            }
            if is_bundle(ast, name) {
                return Err(LayoutError::BundleField(field));
            }
            match enum_layout::enum_named(ast, name) {
                Some(e) => match enum_layout::valid_layout(ast, e, eval) {
                    Some(l) => (l.width, LeafKind::Enum, false),
                    None if !e.generics.is_empty() || enum_layout::payload_variant(e).is_some() => {
                        return Err(LayoutError::Unsupported(field))
                    }
                    None => return Err(LayoutError::UnknownWidth),
                },
                None => return Err(LayoutError::Unsupported(field)),
            }
        }
        TypeRefKind::Error => return Err(LayoutError::UnknownWidth),
        TypeRefKind::Clock | TypeRefKind::Reset(_) | TypeRefKind::Tuple(_) => {
            return Err(LayoutError::Unsupported(field))
        }
    };
    out.push(Leaf {
        path: prefix.clone(),
        ty,
        width: w,
        lsb: 0,
        kind,
        signed,
    });
    Ok(())
}

/// Dizi alanının eleman genişliği: skaler sayısal/bool eleman (ADR-0056
/// paketlenmiş vektör kuralı).
fn array_elem_width(
    ast: &SourceFile,
    elem: Idx<TypeRef>,
    field: &str,
    eval: &mut dyn FnMut(Idx<Expr>) -> Option<i128>,
) -> Result<u32, LayoutError> {
    let target = enum_layout::follow_aliases(ast, elem).ok_or(LayoutError::TooDeep)?;
    let width = |n: Option<i128>| match n {
        Some(n) if (1..=i128::from(u16::MAX)).contains(&n) => Ok(n as u32),
        _ => Err(LayoutError::UnknownWidth),
    };
    match &ast.types[target].kind {
        TypeRefKind::Bool => Ok(1),
        TypeRefKind::UInt(n) | TypeRefKind::SInt(n) => Ok(u32::from(*n)),
        TypeRefKind::Bits(e) | TypeRefKind::UIntN(e) | TypeRefKind::SIntN(e) => width(eval(*e)),
        TypeRefKind::Path { path, .. }
            if path
                .segments
                .last()
                .is_some_and(|s| struct_named(ast, &s.text).is_some()) =>
        {
            Err(LayoutError::ArrayOfStruct(field.to_string()))
        }
        TypeRefKind::Error => Err(LayoutError::UnknownWidth),
        _ => Err(LayoutError::Unsupported(field.to_string())),
    }
}

/// Ad bir `struct port` bundle'ı mı (düz struct alanı olamaz).
pub fn is_bundle(ast: &SourceFile, name: &str) -> bool {
    ast.items.iter().any(|&i| {
        matches!(&ast.items_arena[i].kind, ItemKind::Struct(s) if s.is_port && s.name.text == name)
    })
}

/// Ada göre düz struct'ın geçerli düzeni (katmanların ortak kısayolu).
pub fn layout_of_name(
    ast: &SourceFile,
    name: &str,
    eval: &mut dyn FnMut(Idx<Expr>) -> Option<i128>,
) -> Option<StructLayout> {
    layout(ast, struct_named(ast, name)?, eval).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(path: &[&str], width: u32) -> Leaf {
        let mut types = crate::Arena::<TypeRef>::default();
        let ty = types.alloc(TypeRef {
            span: volt_span::Span::new(volt_span::FileId(0), 0, 0),
            kind: TypeRefKind::Bool,
        });
        Leaf {
            path: path.iter().map(|s| s.to_string()).collect(),
            ty,
            width,
            lsb: 0,
            kind: LeafKind::Plain,
            signed: false,
        }
    }

    fn adr_example() -> StructLayout {
        // ADR-0077 Karar 3 tablosu: a:u4, s:St(2), b:bool, i.x:u3, i.y:i2.
        let mut leaves = vec![
            leaf(&["a"], 4),
            leaf(&["s"], 2),
            leaf(&["b"], 1),
            leaf(&["i", "x"], 3),
            leaf(&["i", "y"], 2),
        ];
        let mut lsb = 0;
        for l in leaves.iter_mut().rev() {
            l.lsb = lsb;
            lsb += l.width;
        }
        StructLayout {
            name: "P".into(),
            width: lsb,
            leaves,
        }
    }

    #[test]
    fn first_leaf_is_most_significant() {
        let l = adr_example();
        assert_eq!(l.width, 12);
        assert_eq!(l.describe(), "a[11:8] s[7:6] b[5] i.x[4:2] i.y[1:0]");
    }

    #[test]
    fn nested_field_bits_are_contiguous() {
        let l = adr_example();
        assert_eq!(l.field_bits(&["i".to_string()]), Some((0, 5)));
        assert_eq!(l.field_bits(&["a".to_string()]), Some((8, 4)));
        assert_eq!(l.field_bits(&["nope".to_string()]), None);
        assert_eq!(l.leaves_under(&[]).count(), 5);
    }
}
