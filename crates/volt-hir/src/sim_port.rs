//! Test bloğunda port genişliği denetimi (ADR-0059, E8512).
//!
//! Testbench bir porta C++ tamsayısı yazar; Verilator girişleri
//! maskelemez. `addr : u3` portuna 8 yazılınca depolama tipinin (8 bit)
//! üst bitleri kirli kalır ve simülasyon donanımda imkânsız bir durumu
//! yürütür. Bu modül port genişliğini AST tipinden çözer (test denetimi
//! isim çözümlemeden bağımsızdır — ADR-0033), sabit değerleri derleme
//! zamanında denetler ve sürücüye çalışma zamanı denetimi için genişliği
//! verir.

use volt_ast::{
    Expr, ExprKind, Idx, ItemKind, Name, PortDir, SourceFile, TestBinOp, TestExpr, TestExprKind,
    TypeRef, TypeRefKind,
};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};

use crate::sim::DutMap;
use crate::sim_const::TestConsts;

/// Betik değerlerinin genişliği: bundan geniş port her değeri tutar.
const SCRIPT_VALUE_BITS: u32 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarKind {
    Bool,
    UInt,
    SInt,
    /// `bits<N>` ya da paketlenmiş dizi portu (ADR-0056).
    Bits,
}

/// Bir portun (ya da bellek elemanının) bit genişliği ve işareti.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortWidth {
    pub bits: u32,
    pub kind: ScalarKind,
}

impl PortWidth {
    pub fn is_signed(self) -> bool {
        self.kind == ScalarKind::SInt
    }

    /// Porta sığan en büyük bit deseni.
    pub fn max_pattern(self) -> u64 {
        if self.bits >= SCRIPT_VALUE_BITS {
            u64::MAX
        } else {
            (1u64 << self.bits) - 1
        }
    }

    /// İşaretli portun en küçük değeri (`i8` → -128).
    pub fn min_signed(self) -> Option<i64> {
        if !self.is_signed() || self.bits == 0 || self.bits >= SCRIPT_VALUE_BITS {
            return None;
        }
        Some(-(1i64 << (self.bits - 1)))
    }

    /// OKUNAN değer: `out` portu hep bit deseni verir (`i8` -1 → 0xFF).
    pub fn fits_pattern(self, value: u64) -> bool {
        value <= self.max_pattern()
    }

    /// YAZILAN değer: bit deseni ya da — işaretli portta — 64 bitte
    /// sarmış negatif sayı (`0 - 1`); o da aralıktaysa kayıpsız temsil
    /// edilir. Testbench'teki `volt_port_fits` ile aynı kural.
    pub fn accepts(self, value: u64) -> bool {
        if self.fits_pattern(value) {
            return true;
        }
        // [63 : bits-1] bitlerinin hepsi 1: aralıktaki negatif sayı.
        self.is_signed() && (value | (self.max_pattern() >> 1)) == u64::MAX
    }

    /// Kabul edilen değerin porta yazılan bit deseni.
    pub fn to_pattern(self, value: u64) -> u64 {
        value & self.max_pattern()
    }

    /// Tanı ve test raporunda görünen tip adı (`u3`, `i8`, `bool`).
    pub fn type_name(self) -> String {
        match self.kind {
            ScalarKind::Bool => "bool".to_string(),
            ScalarKind::UInt => format!("u{}", self.bits),
            ScalarKind::SInt => format!("i{}", self.bits),
            ScalarKind::Bits => format!("bits<{}>", self.bits),
        }
    }

    /// Yazılabilir aralık: `0..7`; işaretlide `-128..255`.
    pub fn write_range(self) -> String {
        match self.min_signed() {
            Some(min) => format!("{min}..{}", self.max_pattern()),
            None => format!("0..{}", self.max_pattern()),
        }
    }
}

/// Tip başvurusunun sayı olarak genişliği.
pub(crate) enum ScalarWidth {
    Known(PortWidth),
    /// Genişlik burada çözülemedi (generic parametre, takma ad, enum):
    /// denetim testbench'te C++ depolama tipine göre yapılır.
    Unknown,
    /// Demet, struct, iç içe dizi: tek sayı olarak yazılamaz.
    NotScalar,
}

pub(crate) fn scalar_width(src: &SourceFile, ty: Idx<TypeRef>) -> ScalarWidth {
    let known = |bits, kind| ScalarWidth::Known(PortWidth { bits, kind });
    let from_expr = |expr, kind| {
        literal_or_const(src, expr)
            .and_then(|v| u32::try_from(v).ok())
            .map_or(ScalarWidth::Unknown, |bits| known(bits, kind))
    };
    match &src.types[ty].kind {
        TypeRefKind::Bool => known(1, ScalarKind::Bool),
        TypeRefKind::UInt(n) => known(u32::from(*n), ScalarKind::UInt),
        TypeRefKind::SInt(n) => known(u32::from(*n), ScalarKind::SInt),
        TypeRefKind::Bits(e) => from_expr(*e, ScalarKind::Bits),
        TypeRefKind::UIntN(e) => from_expr(*e, ScalarKind::UInt),
        TypeRefKind::SIntN(e) => from_expr(*e, ScalarKind::SInt),
        // Tip takma adı / enum: sayı olabilir, burada çözülmez.
        TypeRefKind::Path { .. } => ScalarWidth::Unknown,
        _ => ScalarWidth::NotScalar,
    }
}

/// Port tipi: skaler ya da paketlenmiş vektör olarak üretilen skaler
/// dizisi (ADR-0056) — `[u8; 4]` 32 bitlik tek değerdir.
fn port_type_width(src: &SourceFile, ty: Idx<TypeRef>) -> Option<PortWidth> {
    match scalar_width(src, ty) {
        ScalarWidth::Known(width) => Some(width),
        ScalarWidth::Unknown => None,
        ScalarWidth::NotScalar => {
            let TypeRefKind::Array { elem, len } = &src.types[ty].kind else {
                return None;
            };
            let ScalarWidth::Known(elem) = scalar_width(src, *elem) else {
                return None;
            };
            let len = u32::try_from(literal_or_const(src, *len)?).ok()?;
            Some(PortWidth {
                bits: elem.bits.checked_mul(len)?,
                kind: ScalarKind::Bits,
            })
        }
    }
}

/// Düz literal ya da düz literal değerli üst düzey `const`.
pub(crate) fn literal_or_const(src: &SourceFile, expr: Idx<Expr>) -> Option<u128> {
    literal_value(src, expr).or_else(|| {
        let ExprKind::Path(path) = &src.exprs[expr].kind else {
            return None;
        };
        let [only] = path.segments.as_slice() else {
            return None;
        };
        src.items
            .iter()
            .find_map(|idx| match &src.items_arena[*idx].kind {
                ItemKind::Const(c) if c.name.text == only.text => literal_value(src, c.value),
                _ => None,
            })
    })
}

fn literal_value(src: &SourceFile, expr: Idx<Expr>) -> Option<u128> {
    match &src.exprs[expr].kind {
        ExprKind::IntLit { value, .. } => Some(*value),
        _ => None,
    }
}

/// `module.port` genişliği; port yoksa ya da genişlik çözülemiyorsa
/// `None`. Sürücü çalışma zamanı denetimini bununla kurar.
pub fn test_port_width(sources: &[&SourceFile], module: &str, port: &str) -> Option<PortWidth> {
    let modules = crate::sim::collect_modules(sources);
    let (src, decl) = *modules.get(module)?;
    let found = decl.ports.iter().find(|p| p.name.text == port)?;
    port_type_width(src, found.ty)
}

/// Yalnız literallerden oluşan test ifadesinin değeri: boş ortamda
/// [`TestConsts::eval`]. Ad içeren ifadeler için ortamı kullanın
/// (ADR-0060).
pub fn const_test_value(expr: &TestExpr) -> Option<u64> {
    TestConsts::default().eval(expr)
}

pub(crate) fn fold_binary(op: TestBinOp, l: u64, r: u64) -> Option<u64> {
    let shift = |f: fn(u64, u32) -> u64| {
        u32::try_from(r)
            .ok()
            .filter(|n| *n < 64)
            .map_or(0, |n| f(l, n))
    };
    Some(match op {
        TestBinOp::Add => l.wrapping_add(r),
        TestBinOp::Sub => l.wrapping_sub(r),
        TestBinOp::Mul => l.wrapping_mul(r),
        TestBinOp::Div => l.checked_div(r)?,
        TestBinOp::Rem => l.checked_rem(r)?,
        TestBinOp::And => l & r,
        TestBinOp::Or => l | r,
        TestBinOp::Xor => l ^ r,
        TestBinOp::Shl => shift(|a, n| a << n),
        TestBinOp::Shr => shift(|a, n| a >> n),
        TestBinOp::Eq => u64::from(l == r),
        TestBinOp::Ne => u64::from(l != r),
        TestBinOp::Lt => u64::from(l < r),
        TestBinOp::Le => u64::from(l <= r),
        TestBinOp::Gt => u64::from(l > r),
        TestBinOp::Ge => u64::from(l >= r),
        TestBinOp::LogAnd => u64::from(l != 0 && r != 0),
        TestBinOp::LogOr => u64::from(l != 0 || r != 0),
    })
}

/// Sarmış negatif sayıyı okunur kılar: `18446744073709551416 (-200)`.
pub fn describe_value(value: u64) -> String {
    match i64::try_from(value) {
        Ok(_) => value.to_string(),
        Err(_) => format!("{value} ({})", value as i64),
    }
}

/// DUT'un `dir` yönlü portunun genişliği (dış modül/çözülemeyen → `None`).
fn dut_port_width(duts: &DutMap<'_>, dut: &Name, port: &Name, dir: PortDir) -> Option<PortWidth> {
    let (src, module) = (*duts.get(dut.text.as_str())?)?;
    let found = module
        .ports
        .iter()
        .find(|p| p.name.text == port.text && p.direction == dir)?;
    port_type_width(src, found.ty)
}

/// `dut.port = <sabit>`: değer porta sığmıyorsa E8512.
pub(crate) fn check_set_port_value(
    duts: &DutMap<'_>,
    consts: &TestConsts,
    dut: &Name,
    port: &Name,
    value: &TestExpr,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(width) = dut_port_width(duts, dut, port, PortDir::In) else {
        return;
    };
    let Some(constant) = consts.eval(value) else {
        return; // çalışma zamanı değeri: testbench koşuda denetler
    };
    if width.accepts(constant) {
        return;
    }
    let ty = width.type_name();
    let range = width.write_range();
    let help = if width.is_signed() {
        lstr!(en: "use a value in range {range}; write a negative number as 0 - n";
              tr: "{range} aralığında bir değer kullanın; negatif sayıyı 0 - n biçiminde yazın")
    } else {
        lstr!(en: "use a value in range {range}"; tr: "{range} aralığında bir değer kullanın")
    };
    diags.push(does_not_fit(consts, value, constant, help).with_secondary(
        port.span,
        lstr!(en: "port '{}' is {ty} (max {})", port.text, width.max_pattern();
              tr: "'{}' portu {ty} (en çok {})", port.text, width.max_pattern()),
    ));
}

/// `assert_eq(dut.out, <sabit>)` / `assert_ne`: sabit, portun
/// okunabilecek hiçbir değerine eşit olamıyorsa E8512 — `assert_eq` hep
/// düşer, `assert_ne` SESSİZCE hep geçerdi.
pub(crate) fn check_assert_compare(
    duts: &DutMap<'_>,
    consts: &TestConsts,
    args: &[TestExpr],
    diags: &mut Vec<Diagnostic>,
) {
    let [a, b] = args else {
        return;
    };
    for (read, other) in [(a, b), (b, a)] {
        let TestExprKind::PortRead { dut, port } = &read.kind else {
            continue;
        };
        let Some(width) = dut_port_width(duts, dut, port, PortDir::Out) else {
            continue;
        };
        let Some(constant) = consts.eval(other) else {
            continue;
        };
        if width.fits_pattern(constant) {
            continue;
        }
        let ty = width.type_name();
        let max = width.max_pattern();
        let help = if width.is_signed() {
            lstr!(en: "a port reads as a bit pattern in 0..{max}; compare a negative number by its pattern (-1 is {max:#x})";
                  tr: "port 0..{max} aralığında bit deseni olarak okunur; negatif sayıyı deseniyle karşılaştırın (-1 = {max:#x})")
        } else {
            lstr!(en: "the port reads values in 0..{max}; this comparison can never match";
                  tr: "port 0..{max} aralığında değer okur; bu karşılaştırma hiç eşleşemez")
        };
        diags.push(does_not_fit(consts, other, constant, help).with_secondary(
            port.span,
            lstr!(en: "port '{}' is {ty} (max {max})", port.text;
                  tr: "'{}' portu {ty} (en çok {max})", port.text),
        ));
    }
}

/// E8512. Değer `let`/`const` bağlamalarından geliyorsa (ADR-0060) not
/// satırı hangi adın hangi değeri taşıdığını söyler.
fn does_not_fit(consts: &TestConsts, value: &TestExpr, constant: u64, help: String) -> Diagnostic {
    let shown = describe_value(constant);
    let diag = Diagnostic::error(
        ErrorCode::E8512,
        lstr!(en: "value does not fit in port width"; tr: "değer port genişliğine sığmıyor"),
        LabeledSpan::primary(value.span, lstr!(en: "value {shown}"; tr: "değer {shown}")),
        help,
    );
    let bindings = consts.bindings_in(value);
    if bindings.is_empty() {
        return diag;
    }
    let listed = bindings
        .iter()
        .map(|(name, v)| format!("{name} = {}", describe_value(*v)))
        .collect::<Vec<_>>()
        .join(", ");
    diag.with_note(
        NoteKind::Note,
        lstr!(en: "known at compile time: {listed}"; tr: "derleme zamanında bilinen: {listed}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn width(bits: u32, kind: ScalarKind) -> PortWidth {
        PortWidth { bits, kind }
    }

    #[test]
    fn unsigned_port_accepts_exactly_its_patterns() {
        let u3 = width(3, ScalarKind::UInt);
        assert!(u3.accepts(0));
        assert!(u3.accepts(7));
        assert!(!u3.accepts(8));
        assert!(!u3.accepts(u64::MAX), "0 - 1 işaretsiz porta sığmaz");
        assert_eq!(u3.write_range(), "0..7");
        assert_eq!(u3.type_name(), "u3");
    }

    #[test]
    fn bool_port_accepts_zero_and_one() {
        let b = width(1, ScalarKind::Bool);
        assert!(b.accepts(0) && b.accepts(1));
        assert!(!b.accepts(2));
        assert_eq!(b.type_name(), "bool");
    }

    #[test]
    fn signed_port_accepts_patterns_and_in_range_negatives() {
        let i8w = width(8, ScalarKind::SInt);
        assert!(i8w.accepts(127));
        assert!(i8w.accepts(255), "bit deseni: okunan değerle simetrik");
        assert!(!i8w.accepts(256));
        assert!(i8w.accepts(0u64.wrapping_sub(1)));
        assert!(i8w.accepts(0u64.wrapping_sub(128)));
        assert!(!i8w.accepts(0u64.wrapping_sub(129)));
        assert_eq!(i8w.to_pattern(0u64.wrapping_sub(1)), 0xFF);
        assert_eq!(i8w.to_pattern(0u64.wrapping_sub(128)), 0x80);
        assert_eq!(i8w.write_range(), "-128..255");
    }

    #[test]
    fn one_bit_signed_port_holds_minus_one() {
        let i1 = width(1, ScalarKind::SInt);
        assert!(i1.accepts(u64::MAX));
        assert!(!i1.accepts(u64::MAX - 1));
    }

    #[test]
    fn sixty_four_bit_port_accepts_everything() {
        for kind in [ScalarKind::UInt, ScalarKind::SInt] {
            let w = width(64, kind);
            assert!(w.accepts(u64::MAX));
            assert_eq!(w.to_pattern(u64::MAX), u64::MAX);
        }
    }

    #[test]
    fn reads_never_see_negative_numbers() {
        let i8w = width(8, ScalarKind::SInt);
        assert!(i8w.fits_pattern(0xFF));
        assert!(!i8w.fits_pattern(u64::MAX));
    }

    #[test]
    fn describe_value_shows_wrapped_negatives() {
        assert_eq!(describe_value(8), "8");
        assert_eq!(
            describe_value(0u64.wrapping_sub(200)),
            "18446744073709551416 (-200)"
        );
    }
}
