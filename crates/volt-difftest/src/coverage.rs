//! Kapsam raporu (#458, aşama 0): üretecin bir koşuda ürettiği tipler,
//! işlemler, literal yazımları ve biçimler.
//!
//! Dilin ifade düzeyindeki tip ve işlem listesi tek tablodadır:
//! [`LANGUAGE`]. Kaynağı `docs/spec/grammar-full.ebnf` §12–§14 (tipler,
//! ifadeler, literaller) ve `docs/spec/type-inference.md` §1, §3 (`Ty`,
//! sentez). Her satır üretecin onu bugün üretip üretmediğini söyler
//! (`generated`); `tests/coverage_tests.rs` sabit tohumla iki yönü de
//! sınar: üretilen her satır en az bir kez çıkar, üretilmeyen hiç çıkmaz.
//! Üretilmeyen satır raporda "never generated" ile ve sonraki adımıyla
//! görünür. Biçimler (satır içi, `let`, karşılaştırma, `ensures`, fn)
//! `harness::Variant::ALL`'dan okunur; burada ikinci bir listeleri yoktur.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::ast::{BinOp, Expr, LitFmt, PortTy, UnOp};
use crate::harness::{Case, Variant};
use crate::model::{Model, Ty};

/// Tablonun bölümü.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Area {
    Type,
    Op,
    Literal,
}

impl Area {
    pub fn name(self) -> &'static str {
        match self {
            Area::Type => "type",
            Area::Op => "operation",
            Area::Literal => "literal",
        }
    }
}

/// Dilin ifade düzeyindeki bir tipi ya da işlemi.
#[derive(Clone, Copy, Debug)]
pub struct Item {
    pub area: Area,
    /// Raporda ve sayaçta anahtar.
    pub key: &'static str,
    /// Spesifikasyondaki yeri.
    pub source: &'static str,
    /// Üreteç bugün üretiyor mu? `tests/coverage_tests.rs` iki yönü sınar.
    pub generated: bool,
    /// Üretilmiyorsa: sonraki adım ya da neden.
    pub next: &'static str,
}

const fn item(
    area: Area,
    key: &'static str,
    source: &'static str,
    generated: bool,
    next: &'static str,
) -> Item {
    Item {
        area,
        key,
        source,
        generated,
        next,
    }
}

const G: &str = "grammar-full.ebnf";
const T: &str = "type-inference.md";

/// Dilin ifade düzeyindeki tipleri, işlemleri ve literal yazımları.
pub const LANGUAGE: &[Item] = &[
    // ── Tipler (grammar §12 `PrimitiveType`, `Type`; type-inference §1 `Ty`)
    item(Area::Type, "bool", G, true, ""),
    item(Area::Type, "uN (N <= 64)", G, true, ""),
    item(Area::Type, "iN (N <= 64)", G, true, ""),
    item(
        Area::Type,
        "uN/iN (N > 64)",
        "grammar-full.ebnf §12 (ADR-0031)",
        false,
        "no SV mapping yet (E0003, ADR-0031)",
    ),
    item(Area::Type, "bits<N>", G, true, ""),
    item(Area::Type, "Trit", "type-inference.md §1, §3.3", true, ""),
    item(
        Area::Type,
        "clock",
        G,
        false,
        "not an expression value of the generator",
    ),
    item(
        Area::Type,
        "reset",
        G,
        false,
        "not an expression value of the generator",
    ),
    item(Area::Type, "[T; N] (array)", G, false, "stage 1b (#458)"),
    item(Area::Type, "tuple", G, false, "tuple literals are E0003"),
    item(Area::Type, "struct", T, false, "stage 1c (#458)"),
    item(Area::Type, "enum", T, false, "after stage 1c (#458)"),
    item(Area::Type, "integer literal (untyped)", T, true, ""),
    // ── İşlemler (grammar §13; type-inference §3.3–§3.6)
    item(Area::Op, "a + b", G, true, ""),
    item(Area::Op, "a - b", G, true, ""),
    item(Area::Op, "a * b", G, true, ""),
    item(Area::Op, "a / b", G, true, ""),
    item(Area::Op, "a % b", G, true, ""),
    item(Area::Op, "a & b", G, true, ""),
    item(Area::Op, "a | b", G, true, ""),
    item(Area::Op, "a ^ b", G, true, ""),
    item(Area::Op, "a << b", G, true, ""),
    item(Area::Op, "a >> b", G, true, ""),
    item(Area::Op, "a == b", G, true, ""),
    item(Area::Op, "a != b", G, true, ""),
    item(Area::Op, "a < b", G, true, ""),
    item(Area::Op, "a <= b", G, true, ""),
    item(Area::Op, "a > b", G, true, ""),
    item(Area::Op, "a >= b", G, true, ""),
    item(Area::Op, "a && b", G, false, "not generated yet"),
    item(Area::Op, "a || b", G, false, "not generated yet"),
    item(Area::Op, "a -> b", G, false, "not generated yet"),
    item(Area::Op, "!a", G, true, ""),
    item(Area::Op, "~a", G, true, ""),
    item(Area::Op, "-a", G, true, ""),
    item(Area::Op, "a[i]", G, true, ""),
    item(Area::Op, "a[h:l]", G, true, ""),
    item(Area::Op, "a[i +: w]", G, true, ""),
    item(Area::Op, "a[i -: w]", G, true, ""),
    item(Area::Op, "a as T", G, true, ""),
    item(Area::Op, "concat(a, b)", "ADR-0072", true, ""),
    item(Area::Op, "f(args) (call)", G, true, ""),
    item(
        Area::Op,
        "if c { a } else { b }",
        G,
        false,
        "not generated yet",
    ),
    item(Area::Op, "match", G, false, "not generated yet"),
    item(Area::Op, "a.field", G, false, "stage 1c (#458)"),
    item(Area::Op, "struct literal", G, false, "stage 1c (#458)"),
    item(Area::Op, "array literal", G, false, "stage 1b (#458)"),
    item(
        Area::Op,
        "tuple literal",
        G,
        false,
        "tuple literals are E0003",
    ),
    item(
        Area::Op,
        "prev(x)",
        "type-inference.md §3.10",
        false,
        "stage 2 (#458)",
    ),
    item(Area::Op, "todo!", G, false, "not a value"),
    item(
        Area::Op,
        "declassify(a, \"..\")",
        G,
        false,
        "not generated yet",
    ),
    // ── Literaller (grammar §14)
    item(Area::Literal, "decimal", G, true, ""),
    item(Area::Literal, "hexadecimal (0x)", G, true, ""),
    item(Area::Literal, "binary (0b)", G, true, ""),
    item(Area::Literal, "octal (0o)", G, false, "not generated yet"),
    item(
        Area::Literal,
        "suffixed (42u8)",
        G,
        false,
        "not generated yet",
    ),
    item(Area::Literal, "true / false", G, false, "not generated yet"),
];

/// Bir koşunun sayımları.
#[derive(Clone, Debug, Default)]
pub struct Coverage {
    /// Üretilen ifade sayısı.
    pub cases: usize,
    /// [`LANGUAGE`] anahtarı → düğüm (ya da tip) sayısı.
    pub items: BTreeMap<&'static str, usize>,
    /// Biçim → modüle giren ifade sayısı.
    pub forms: BTreeMap<Variant, usize>,
    /// "işlem on tip" → sayı: işlemin (ilk) işleneninin tipi.
    pub op_types: BTreeMap<String, usize>,
}

/// Bir model tipinin tablodaki anahtarı.
fn ty_key(t: Ty) -> &'static str {
    match t {
        Ty::Bool => "bool",
        Ty::Int { signed: false, .. } => "uN (N <= 64)",
        Ty::Int { signed: true, .. } => "iN (N <= 64)",
        Ty::Bits(_) => "bits<N>",
        Ty::Trit => "Trit",
        Ty::Lit(_) => "integer literal (untyped)",
    }
}

fn port_key(p: PortTy) -> &'static str {
    match p {
        PortTy::Bool => "bool",
        PortTy::UInt(_) => "uN (N <= 64)",
        PortTy::SInt(_) => "iN (N <= 64)",
        PortTy::Bits(_) => "bits<N>",
        PortTy::Trit => "Trit",
    }
}

fn bin_key(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "a + b",
        BinOp::Sub => "a - b",
        BinOp::Mul => "a * b",
        BinOp::Div => "a / b",
        BinOp::Rem => "a % b",
        BinOp::And => "a & b",
        BinOp::Or => "a | b",
        BinOp::Xor => "a ^ b",
        BinOp::Shl => "a << b",
        BinOp::Shr => "a >> b",
        BinOp::Eq => "a == b",
        BinOp::Ne => "a != b",
        BinOp::Lt => "a < b",
        BinOp::Le => "a <= b",
        BinOp::Gt => "a > b",
        BinOp::Ge => "a >= b",
    }
}

fn un_key(op: UnOp) -> &'static str {
    match op {
        UnOp::Not => "!a",
        UnOp::BitNot => "~a",
        UnOp::Neg => "-a",
    }
}

fn lit_key(f: LitFmt) -> &'static str {
    match f {
        LitFmt::Dec => "decimal",
        LitFmt::Hex => "hexadecimal (0x)",
        LitFmt::Bin => "binary (0b)",
    }
}

/// Düğümün işlem anahtarı ve (ilk) işleneni.
fn op_of(e: &Expr) -> Option<(&'static str, &Expr)> {
    match e {
        Expr::Bin(op, a, _) => Some((bin_key(*op), a)),
        Expr::Un(op, a) => Some((un_key(*op), a)),
        Expr::Index(a, _) => Some(("a[i]", a)),
        Expr::Range(a, ..) => Some(("a[h:l]", a)),
        Expr::Part(a, _, _, true) => Some(("a[i +: w]", a)),
        Expr::Part(a, _, _, false) => Some(("a[i -: w]", a)),
        Expr::Cast(a, _) => Some(("a as T", a)),
        Expr::Concat(xs) => xs.first().map(|a| ("concat(a, b)", a)),
        Expr::Port(_) | Expr::Name(..) | Expr::Lit(..) | Expr::WideLit(..) => None,
    }
}

/// Tablodaki satır (anahtarla); yoksa programlama hatasıdır.
pub fn item_of(key: &str) -> &'static Item {
    LANGUAGE
        .iter()
        .find(|i| i.key == key)
        .unwrap_or_else(|| panic!("kapsam tablosunda yok: {key}"))
}

impl Coverage {
    fn bump(&mut self, key: &'static str) {
        // Anahtar tabloda olmalı: yazım hatası sayacı sessizce ayırmasın.
        let key = item_of(key).key;
        *self.items.entry(key).or_default() += 1;
    }

    /// Bir ifadenin düğümlerini, tiplerini ve biçimlerini sayar.
    pub fn add_case(&mut self, c: &Case, ports: &[PortTy]) {
        self.cases += 1;
        for v in &c.variants {
            *self.forms.entry(*v).or_default() += 1;
        }
        if c.variants.contains(&Variant::R) {
            self.bump("f(args) (call)");
        }
        self.add_expr(&c.expr, ports);
    }

    fn add_expr(&mut self, e: &Expr, ports: &[PortTy]) {
        let model = Model { ports };
        match e {
            Expr::Port(k) => self.bump(port_key(ports[*k])),
            Expr::Lit(_, f) | Expr::WideLit(_, f) => {
                self.bump(lit_key(*f));
                self.bump("integer literal (untyped)");
            }
            _ => {}
        }
        if let Some((op, a)) = op_of(e) {
            self.bump(op);
            // Geniş literal yalnız `bits<N>`'e dönüşümün işlenenidir; tipi
            // modelde o dönüşümden gelir (A14).
            let on = match a {
                Expr::WideLit(..) => "integer literal (untyped)",
                _ => model.synth(a).map_or("ill-typed operand", |t| ty_key(t.ty)),
            };
            *self.op_types.entry(format!("{op} on {on}")).or_default() += 1;
            if let Ok(t) = model.synth(e) {
                self.bump(ty_key(t.ty));
            }
        }
        // `Case::expr`'de `Name` yoktur (C biçiminin ağacı ayrıdır).
        for ch in e.children() {
            self.add_expr(ch, ports);
        }
    }

    pub fn add_cases(&mut self, cases: &[Case], ports: &[PortTy]) {
        for c in cases {
            self.add_case(c, ports);
        }
    }

    pub fn count(&self, key: &str) -> usize {
        self.items.get(key).copied().unwrap_or(0)
    }

    /// Hiç üretilmeyen satırlar.
    pub fn missing(&self) -> Vec<&'static Item> {
        LANGUAGE.iter().filter(|i| self.count(i.key) == 0).collect()
    }

    pub fn render(&self) -> String {
        let mut s = String::new();
        let missing = self.missing();
        let _ = writeln!(
            s,
            "difftest: coverage: expressions={} language-items={} generated={} never-generated={}",
            self.cases,
            LANGUAGE.len(),
            LANGUAGE.len() - missing.len(),
            missing.len()
        );
        for area in [Area::Type, Area::Op, Area::Literal] {
            for i in LANGUAGE.iter().filter(|i| i.area == area) {
                let n = self.count(i.key);
                if n == 0 {
                    let _ = writeln!(
                        s,
                        "  {} {}: 0 never generated ({}; {})",
                        area.name(),
                        i.key,
                        i.source,
                        i.next
                    );
                } else {
                    let _ = writeln!(s, "  {} {}: {n}", area.name(), i.key);
                }
            }
        }
        for v in Variant::ALL {
            let n = self.forms.get(&v).copied().unwrap_or(0);
            let _ = writeln!(s, "  form {} ({}): {n}", v.letter(), v.what());
        }
        let list: Vec<String> = self
            .op_types
            .iter()
            .map(|(k, n)| format!("{k}={n}"))
            .collect();
        let _ = writeln!(s, "difftest: coverage by operand type: {}", list.join("; "));
        s
    }
}
