//! Test betiğinde bütün-struct değerleri (ADR-0077 Karar 6).
//!
//! Testbench betik değerleri 64 bit sayıdır. Bütün struct karşılaştırması
//! (`assert_eq(dut.q, P { a: 1, b: true })`) iki tarafı Karar 3 düzeniyle
//! (ilk alan MSB) paketlenmiş sayıya indirir: port tarafı yaprak
//! portlarının kaydırılıp birleştirilmesi, literal tarafı sabit desen.
//! Rapor aynı düzenle sayıyı alan adlarına geri açar:
//! `P { a: 3, b: true }` ve farklı alanlar.

use volt_ast::struct_layout::{LeafKind, StructLayout};
use volt_ast::{SourceFile, TestBinOp, TestExpr, TestExprKind};
use volt_sv_emit::TbValue;

use crate::sim_lower::EnumLabels;

/// Bir yaprağın rapor biçimi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LeafLabel {
    /// Noktalı alan yolu (`i.x`).
    pub path: String,
    pub lsb: u32,
    pub width: u32,
    pub signed: bool,
    pub is_bool: bool,
    /// Enum yaprağının varyant adları (ADR-0074).
    pub variants: Option<EnumLabels>,
}

impl LeafLabel {
    fn value(&self, packed: u64) -> u64 {
        let v = packed >> self.lsb;
        if self.width >= 64 {
            v
        } else {
            v & ((1u64 << self.width) - 1)
        }
    }

    fn render(&self, packed: u64) -> String {
        let v = self.value(packed);
        if let Some(e) = &self.variants {
            return e.label(v);
        }
        if self.is_bool {
            return (v != 0).to_string();
        }
        if self.signed && self.width < 64 && v >> (self.width - 1) & 1 == 1 {
            return (v as i64 - (1i64 << self.width)).to_string();
        }
        v.to_string()
    }
}

/// Struct değerli iddianın alan adları.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StructLabels {
    pub name: String,
    pub leaves: Vec<LeafLabel>,
}

impl StructLabels {
    /// `P { a: 3, s: St::Run, i.x: 1 }`.
    pub fn render(&self, packed: u64) -> String {
        let fields: Vec<String> = self
            .leaves
            .iter()
            .map(|l| format!("{}: {}", l.path, l.render(packed)))
            .collect();
        format!("{} {{ {} }}", self.name, fields.join(", "))
    }

    /// Değeri farklı yaprakların yolları.
    pub fn differs(&self, a: u64, b: u64) -> Vec<String> {
        self.leaves
            .iter()
            .filter(|l| l.value(a) != l.value(b))
            .map(|l| l.path.clone())
            .collect()
    }
}

/// Enum adı → varyant (ad, kod) listesi.
type VariantLookup<'a> = dyn Fn(&str) -> Option<Vec<(String, u64)>> + 'a;

/// Düzenden rapor etiketleri; enum yaprakları varyant adlarıyla.
pub(crate) fn labels(
    src: &SourceFile,
    layout: &StructLayout,
    enum_variants: &VariantLookup,
) -> StructLabels {
    let leaves = layout
        .leaves
        .iter()
        .map(|l| {
            let variants = (l.kind == LeafKind::Enum)
                .then(|| volt_ast::enum_layout::enum_of_type(src, l.ty))
                .flatten()
                .and_then(|e| {
                    Some(EnumLabels {
                        enum_name: e.name.text.clone(),
                        variants: enum_variants(&e.name.text)?,
                    })
                });
            let is_bool = matches!(src.types[l.ty].kind, volt_ast::TypeRefKind::Bool);
            LeafLabel {
                path: l.dotted(),
                lsb: l.lsb,
                width: l.width,
                signed: l.signed,
                is_bool,
                variants,
            }
        })
        .collect();
    StructLabels {
        name: layout.name.clone(),
        leaves,
    }
}

/// Struct portunun paketlenmiş okuması: `((q_a << 1) | q_b)` (MSB'den).
pub(crate) fn packed_port(port: &str, layout: &StructLayout) -> TbValue {
    let mut acc: Option<TbValue> = None;
    for leaf in &layout.leaves {
        let read = TbValue::Port(format!("{port}_{}", leaf.suffix()));
        acc = Some(match acc {
            None => read,
            Some(prev) => or(shl(prev, leaf.width), read),
        });
    }
    acc.unwrap_or(TbValue::Lit(0))
}

/// Literalin paketlenmiş değeri: yaprak değerleri genişliğe maskelenip
/// MSB'den birleştirilir. `value` bir yaprağın ifadesini indirger.
pub(crate) fn packed_literal(
    lit: &TestExpr,
    layout: &StructLayout,
    value: &dyn Fn(&TestExpr) -> Option<TbValue>,
) -> Option<TbValue> {
    let mut acc: Option<TbValue> = None;
    for leaf in &layout.leaves {
        let v = value(leaf_expr(lit, &leaf.path)?)?;
        let masked = match v {
            TbValue::Lit(n) if leaf.width < 64 => TbValue::Lit(n & ((1u64 << leaf.width) - 1)),
            TbValue::Lit(n) => TbValue::Lit(n),
            other if leaf.width < 64 => TbValue::Binary {
                op: TestBinOp::And,
                lhs: Box::new(other),
                rhs: Box::new(TbValue::Lit((1u64 << leaf.width) - 1)),
            },
            other => other,
        };
        acc = Some(match acc {
            None => masked,
            Some(prev) => or(shl(prev, leaf.width), masked),
        });
    }
    acc.map(fold)
}

/// Literalde yaprağın ifadesi (iç içe literal izlenir).
fn leaf_expr<'e>(lit: &'e TestExpr, path: &[String]) -> Option<&'e TestExpr> {
    let Some((first, rest)) = path.split_first() else {
        return Some(lit);
    };
    let TestExprKind::StructLit { fields, .. } = &lit.kind else {
        return None;
    };
    let (_, v) = fields.iter().find(|(n, _)| n.text == *first)?;
    leaf_expr(v, rest)
}

fn shl(v: TbValue, n: u32) -> TbValue {
    TbValue::Binary {
        op: TestBinOp::Shl,
        lhs: Box::new(v),
        rhs: Box::new(TbValue::Lit(u64::from(n))),
    }
}

fn or(a: TbValue, b: TbValue) -> TbValue {
    TbValue::Binary {
        op: TestBinOp::Or,
        lhs: Box::new(a),
        rhs: Box::new(b),
    }
}

/// Yalnız literallerden oluşan ağaç tek literale katlanır.
fn fold(v: TbValue) -> TbValue {
    match v {
        TbValue::Binary { op, lhs, rhs } => match (fold(*lhs), fold(*rhs)) {
            (TbValue::Lit(a), TbValue::Lit(b)) => match volt_hir::fold_test_binary(op, a, b) {
                Some(n) => TbValue::Lit(n),
                None => TbValue::Binary {
                    op,
                    lhs: Box::new(TbValue::Lit(a)),
                    rhs: Box::new(TbValue::Lit(b)),
                },
            },
            (l, r) => TbValue::Binary {
                op,
                lhs: Box::new(l),
                rhs: Box::new(r),
            },
        },
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(path: &str, lsb: u32, width: u32) -> LeafLabel {
        LeafLabel {
            path: path.to_string(),
            lsb,
            width,
            signed: false,
            is_bool: false,
            variants: None,
        }
    }

    #[test]
    fn render_names_fields_and_reports_differences() {
        let mut b = leaf("b", 0, 1);
        b.is_bool = true;
        let labels = StructLabels {
            name: "P".into(),
            leaves: vec![leaf("a", 1, 4), b],
        };
        // a = 3, b = true → 0b0011_1; a = 1, b = true → 0b0001_1.
        assert_eq!(labels.render(0b00111), "P { a: 3, b: true }");
        assert_eq!(labels.render(0b00011), "P { a: 1, b: true }");
        assert_eq!(labels.differs(0b00111, 0b00011), ["a"]);
    }

    #[test]
    fn signed_leaf_renders_negative() {
        let mut y = leaf("i.y", 0, 2);
        y.signed = true;
        let labels = StructLabels {
            name: "I".into(),
            leaves: vec![y],
        };
        assert_eq!(labels.render(0b11), "I { i.y: -1 }");
    }
}
