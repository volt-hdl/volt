//! Testbench port genişliği (ADR-0095 §5).
//!
//! Verilator 64 bitten geniş portu `VlWide<N>` (32 bitlik sözcük dizisi)
//! olarak üretir: `dut.p = 1` ve `(unsigned long long)dut.p` derlenmez.
//! `volt run` testbench'i genişliği kanıtlanamayan portları şablon
//! yardımcılarıyla sürer ve basar; skaler portta yardımcı eski ifadenin
//! aynısını üretir, bu yüzden dar portlu tasarımın testbench'i bayt bayt
//! değişmez.

use volt_ast::{Idx, SourceFile, TypeRef, TypeRefKind};

use crate::SimPort;

/// İç içe tip (dizi, takma ad) için özyineleme sınırı.
const MAX_TYPE_DEPTH: u32 = 16;

/// Verilator'un skaler C++ tipine (`QData`) sığan en geniş port.
pub(crate) const MAX_SCALAR_BITS: u32 = 64;

/// Tipin bit genişliği; derleme zamanında çözülemezse `None`.
pub(crate) fn port_bits(src: &SourceFile, ty: Idx<TypeRef>) -> Option<u32> {
    bits_at(src, ty, 0)
}

fn bits_at(src: &SourceFile, ty: Idx<TypeRef>, depth: u32) -> Option<u32> {
    if depth > MAX_TYPE_DEPTH {
        return None;
    }
    let ty = crate::alias::resolve(src, ty);
    let from_expr = |e| crate::structs::const_int(src, e).and_then(|v| u32::try_from(v).ok());
    match &src.types[ty].kind {
        TypeRefKind::Bool | TypeRefKind::Clock | TypeRefKind::Reset(_) => Some(1),
        TypeRefKind::UInt(n) | TypeRefKind::SInt(n) => Some(u32::from(*n)),
        TypeRefKind::Trit => Some(2),
        TypeRefKind::Bits(e) | TypeRefKind::UIntN(e) | TypeRefKind::SIntN(e) => from_expr(*e),
        TypeRefKind::Array { elem, len } => {
            bits_at(src, *elem, depth + 1)?.checked_mul(from_expr(*len)?)
        }
        TypeRefKind::Path { .. } => {
            let decl = volt_ast::enum_layout::enum_of_type(src, ty)?;
            let eval = &mut |e| crate::structs::const_int(src, e);
            volt_ast::enum_layout::valid_layout(src, decl, eval).map(|l| l.width)
        }
        TypeRefKind::Tuple(_) | TypeRefKind::Error => None,
    }
}

/// Port Verilator modelinde `VlWide` olabilir mi? Genişlik bilinmiyorsa
/// evet (yardımcı her iki tipte de derlenir).
pub(crate) fn may_be_wide(bits: Option<u32>) -> bool {
    bits.is_none_or(|b| b > MAX_SCALAR_BITS)
}

/// Geniş değerin tablo sütunu: `0x` + 32 bitlik sözcük başına 8 onaltılık
/// basamak. Genişlik bilinmiyorsa 0 (sütun adın genişliğinde kalır).
pub(crate) fn show_width(bits: Option<u32>) -> usize {
    bits.map_or(0, |b| 2 + 8 * b.div_ceil(32) as usize)
}

/// `volt run` girişine sabit yazan satır; geniş port yardımcıyla (§5).
pub(crate) fn drive(p: &SimPort, value: u8) -> String {
    let port = crate::sim::cpp_port(&p.name);
    if may_be_wide(p.bits) {
        format!("    volt_drive(dut.{port}, {value});\n")
    } else {
        format!("    dut.{port} = {value};\n")
    }
}

/// `volt_drive` / `volt_show`: skaler tipte düz atama ve ondalık yazım,
/// `VlWide<N>`'de sözcük sözcük atama ve onaltılık yazım.
pub(crate) const WIDE_PRELUDE: &str = "\
// Ports wider than 64 bits are VlWide words in the Verilator model.
#include <string>
template <typename T>
static void volt_drive(T& port, unsigned long long v) { port = static_cast<T>(v); }
template <std::size_t N>
static void volt_drive(VlWide<N>& port, unsigned long long v) {
    for (std::size_t i = 0; i < N; ++i) port[i] = 0;
    port[0] = static_cast<EData>(v);
    if (N > 1) port[1] = static_cast<EData>(v >> 32);
}
template <typename T>
static std::string volt_show(const T& v) { return std::to_string(static_cast<unsigned long long>(v)); }
template <std::size_t N>
static std::string volt_show(const VlWide<N>& v) {
    std::string out = \"0x\";
    char word[9];
    for (std::size_t i = N; i-- > 0;) {
        std::snprintf(word, sizeof word, \"%08x\", static_cast<unsigned>(v[i]));
        out += word;
    }
    return out;
}
";

#[cfg(test)]
mod tests {
    use super::*;
    use volt_ast::{ItemKind, ModuleDecl};
    use volt_span::FileId;

    fn widths(src: &str) -> Vec<(String, Option<u32>)> {
        let parsed = volt_syntax::parse(FileId(0), src);
        let ast = parsed.ast;
        let module: &ModuleDecl = ast
            .items
            .iter()
            .find_map(|i| match &ast.items_arena[*i].kind {
                ItemKind::Module(m) => Some(m),
                _ => None,
            })
            .expect("modül");
        module
            .ports
            .iter()
            .map(|p| (p.name.text.clone(), port_bits(&ast, p.ty)))
            .collect()
    }

    #[test]
    fn port_bits_covers_scalars_arrays_trits_and_consts() {
        // Arrange
        let src = "const N : u8 = 9\n\
                   module W {\n    in clk : clock\n    in a : u64\n    in b : [u8; 9]\n\
                   \x20   in c : [Trit; N * N]\n    in d : [u8; 4]\n    in e : bits<N>\n    out q : bool\n    q = true\n}\n";

        // Act
        let got = widths(src);

        // Assert
        let get = |n: &str| got.iter().find(|(p, _)| p == n).and_then(|(_, b)| *b);
        assert_eq!(get("clk"), Some(1));
        assert_eq!(get("a"), Some(64));
        assert_eq!(get("b"), Some(72));
        assert_eq!(get("c"), Some(162));
        assert_eq!(get("d"), Some(32));
        assert_eq!(get("e"), Some(9));
    }

    #[test]
    fn unknown_or_over_64_bits_is_wide() {
        assert!(!may_be_wide(Some(64)));
        assert!(may_be_wide(Some(65)));
        assert!(may_be_wide(None));
        assert_eq!(show_width(Some(65)), 2 + 24);
        assert_eq!(show_width(Some(128)), 2 + 32);
        assert_eq!(show_width(None), 0);
    }
}
