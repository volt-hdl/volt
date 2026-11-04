//! Referans model: Volt ifade anlamı, doğrudan spesifikasyondan.
//!
//! Kaynaklar: `docs/spec/type-inference.md` §3 (sentez), §4 (literal),
//! §5 (atanabilirlik), `docs/spec/operator-precedence.md`,
//! `docs/spec/sv-mapping.md` §6, ADR-0025 (esnek genişlik aralığı),
//! ADR-0035 (değişken indeks, `+:`/`-:`), ADR-0036 (işaretli kaydırma,
//! dönüşüm), ADR-0041 (check kipinde aynı işaretli genişleme ve itme),
//! ADR-0106 (karşılaştırma genişliği, bileşik değerden seçim).
//!
//! Derleyicinin hiçbir parçası kullanılmaz (sabit katlayıcı, tip
//! denetleyicisi, emitter): bu crate'in bağımlılığı yoktur.
//!
//! **Anlam.** Her alt ifadenin bağlamdan bağımsız bir değeri vardır: doğal
//! genişliğinde (aralığın üst ucu) bit örüntüsü. Volt'ta genişliği
//! işlenenler belirler, içinde bulunulan ifade değil. İkili işlem
//! işlenenlerini sonucun doğal genişliğine dönüştürür (geniş olan kesilir,
//! dar olan kendi işaretiyle genişler) ve sonucu o genişlikte hesaplar.
//! Check kipinde (çıkış ataması, tipli `let`) ADR-0041'in itmesi yalnız en
//! üstteki aritmetik işlemi hedef genişlikte yapar.
//!
//! Spesifikasyonun sessiz ya da belirsiz kaldığı her yer bir varsayım
//! etiketi taşır (`ASSUMPTIONS`); fark raporu farkı etiketleriyle birlikte
//! basar. Spesifikasyonun hiç tanımlamadığı biçimler `Unspecified`
//! döner ve karşılaştırılmaz.

use crate::ast::{BinOp, Expr, PortTy, UnOp};
use crate::value::{conv, fits, mask, to_i128};

/// Modelin tip gösterimi (type-inference.md §1, ADR-0025 esnek aralık).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    Bool,
    /// `lo == hi` somut tip; `lo < hi` aritmetik sonucun esnek aralığı.
    Int {
        signed: bool,
        lo: u32,
        hi: u32,
    },
    Bits(u32),
    /// Soneksiz literal, değeriyle (§3.1, §4).
    Lit(i128),
}

impl Ty {
    pub fn int(signed: bool, w: u32) -> Ty {
        Ty::Int {
            signed,
            lo: w,
            hi: w,
        }
    }

    /// Doğal tip: kullanıcıya gösterilen, tipsiz `let`'in taşıdığı.
    pub fn natural(self) -> Option<PortTy> {
        match self {
            Ty::Bool => Some(PortTy::Bool),
            Ty::Int { signed, hi, .. } => Some(PortTy::int(signed, hi)),
            Ty::Bits(w) => Some(PortTy::Bits(w)),
            Ty::Lit(_) => None,
        }
    }

    /// Değer örüntüsünün genişliği (literal için 128).
    pub fn width(self) -> u32 {
        match self {
            Ty::Bool => 1,
            Ty::Int { hi, .. } => hi,
            Ty::Bits(w) => w,
            Ty::Lit(_) => 128,
        }
    }

    fn of_port(p: PortTy) -> Ty {
        match p {
            PortTy::Bool => Ty::Bool,
            PortTy::UInt(w) => Ty::int(false, w),
            PortTy::SInt(w) => Ty::int(true, w),
            PortTy::Bits(w) => Ty::Bits(w),
        }
    }
}

/// Modelin bir ifade için kararı (geçerli değilse).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reject {
    /// Spesifikasyon hata der; tanı kodu ya da kod adlandırılmamışsa `"?"`.
    Invalid(&'static str),
    /// Belgelenmiş "henüz desteklenmiyor" sınırı (E0003).
    Unsupported(&'static str),
    /// Spesifikasyon tanımlamıyor: değer karşılaştırılmaz.
    Unspecified(&'static str),
}

/// En geniş tam sayı (ADR-0031: `uN`/`iN` 64'e kadar; üstü E0003).
pub const MAX_INT: u32 = 64;
/// Sınanan en geniş `bits<N>` (görev: 1–70 bit).
pub const MAX_BITS: u32 = 70;

/// Varsayım etiketleri: spesifikasyonun açık bıraktığı yerde modelin
/// seçtiği okuma. Kimlikler ADR-0108'deki tabloyla aynıdır.
pub mod tag {
    /// A1: aralıklı (esnek) işlenen sonucun doğal genişliğine dönüştürülür;
    /// sonucun işlem genişliğine (kesişimin üst ucu) kesilmez.
    pub const FLEX_OPERAND: u32 = 1 << 0;
    /// A2: check kipinde itme işlenenlerin sentezlenmiş değerini genişletir;
    /// iç içe aritmetiğe hedef genişlik özyineli itilmez.
    pub const PUSH_NESTED: u32 = 1 << 1;
    /// A3: işaretsiz çıkarma doğal genişlikte (N+1 bit) modüler sarar.
    pub const UNSIGNED_SUB: u32 = 1 << 2;
    /// A4: esnek sol işlenenin kaydırması doğal genişliğinde yapılır.
    pub const SHIFT_FLEX: u32 = 1 << 3;
    /// A5: esnek işlenenin `~`'i doğal genişliğinde yapılır.
    pub const NOT_FLEX: u32 = 1 << 4;
    /// A6: işaret değiştiren genişletme kaynağın işaretiyle genişletir.
    pub const CAST_SIGN_EXT: u32 = 1 << 5;
    /// A7: `iN as iM` (M < N) geçerli ve keser (§3.6 yalnız işaretsiz
    /// daraltmayı sayar).
    pub const SIGNED_NARROW: u32 = 1 << 6;
    /// A8: `iN as bits<N>` geçerli (§3.6 yalnız `uN` ↔ `bits<N>` sayar).
    pub const SINT_TO_BITS: u32 = 1 << 7;
    /// A9: esnek işlenenin yanındaki literal doğal genişliğe sığmalı.
    pub const LIT_FLEX: u32 = 1 << 8;
    /// A10: esnek işaretli işlenenin `-`'si `i(doğal+1)`.
    pub const NEG_FLEX: u32 = 1 << 9;
    /// A11: `~b` (`b : bool`) `bool` verir (§3.4 `BitNot => ot`).
    pub const BOOL_BITNOT: u32 = 1 << 10;
    /// A12: işaretli `>>` genişlik kadar ya da fazla kaydırınca işaretle
    /// dolar (W2013 "sonuç hep 0" der).
    pub const SHR_OVERSHIFT: u32 = 1 << 11;
}

/// `(etiket, kimlik, açıklama)` — rapor ve ADR için.
pub const ASSUMPTIONS: &[(u32, &str, &str)] = &[
    (
        tag::FLEX_OPERAND,
        "A1",
        "flex operand converted to the result's natural width, not truncated to the common (intersection) width",
    ),
    (
        tag::PUSH_NESTED,
        "A2",
        "check-mode push widens the operands' synthesized values; it does not recurse into nested arithmetic",
    ),
    (
        tag::UNSIGNED_SUB,
        "A3",
        "unsigned subtraction wraps modulo 2^(N+1) at its natural width",
    ),
    (
        tag::SHIFT_FLEX,
        "A4",
        "shift of a flex left operand happens at its natural width",
    ),
    (
        tag::NOT_FLEX,
        "A5",
        "~ of a flex operand happens at its natural width",
    ),
    (
        tag::CAST_SIGN_EXT,
        "A6",
        "a sign-changing widening cast extends by the source's sign",
    ),
    (
        tag::SIGNED_NARROW,
        "A7",
        "iN as iM (M < N) is legal and truncates",
    ),
    (
        tag::SINT_TO_BITS,
        "A8",
        "iN as bits<N> is legal",
    ),
    (
        tag::LIT_FLEX,
        "A9",
        "a literal next to a flex operand has to fit the natural width",
    ),
    (
        tag::NEG_FLEX,
        "A10",
        "-x of a flex signed x is i(natural + 1)",
    ),
    (
        tag::BOOL_BITNOT,
        "A11",
        "~b of a bool is a bool",
    ),
    (
        tag::SHR_OVERSHIFT,
        "A12",
        "signed >> by the width or more fills with the sign bit",
    ),
];

pub fn tag_ids(tags: u32) -> String {
    let ids: Vec<&str> = ASSUMPTIONS
        .iter()
        .filter(|(t, ..)| tags & t != 0)
        .map(|(_, id, _)| *id)
        .collect();
    if ids.is_empty() {
        "-".into()
    } else {
        ids.join(",")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Typed {
    pub ty: Ty,
    pub tags: u32,
}

impl Typed {
    fn new(ty: Ty, tags: u32) -> Typed {
        Typed { ty, tags }
    }
}

type R<T> = Result<T, Reject>;

pub struct Model<'a> {
    pub ports: &'a [PortTy],
}

fn meet(a: (u32, u32), b: (u32, u32)) -> Option<(u32, u32)> {
    let lo = a.0.max(b.0);
    let hi = a.1.min(b.1);
    (lo <= hi).then_some((lo, hi))
}

impl Model<'_> {
    // ── Sentez (type-inference.md §3) ───────────────────────────────

    pub fn synth(&self, e: &Expr) -> R<Typed> {
        match e {
            Expr::Port(k) => Ok(Typed::new(Ty::of_port(self.ports[*k]), 0)),
            Expr::Name(_, b) => self.synth(b),
            Expr::Lit(v, _) => Ok(Typed::new(Ty::Lit(*v), 0)),
            Expr::Un(op, x) => {
                let t = self.synth(x)?;
                let (ty, tags) = self.unary(*op, t.ty)?;
                Ok(Typed::new(ty, t.tags | tags))
            }
            Expr::Bin(op, x, y) => {
                let a = self.synth(x)?;
                let b = self.synth(y)?;
                let (ty, tags) = self.binary(*op, a.ty, b.ty)?;
                Ok(Typed::new(ty, a.tags | b.tags | tags))
            }
            Expr::Index(b, i) => {
                let tb = self.synth(b)?;
                let w = Self::select_width(tb.ty)?;
                let ti = self.synth(i)?;
                match ti.ty {
                    Ty::Lit(v) if v < 0 || v >= i128::from(w) => Err(Reject::Invalid("E2006")),
                    Ty::Lit(_) | Ty::Int { signed: false, .. } => {
                        Ok(Typed::new(Ty::Bool, tb.tags | ti.tags))
                    }
                    Ty::Int { signed: true, .. } => Err(Reject::Unspecified("signed index")),
                    Ty::Bits(_) => Err(Reject::Unspecified("bits<N> index")),
                    Ty::Bool => Err(Reject::Invalid("?")),
                }
            }
            Expr::Range(b, h, l) => {
                let tb = self.synth(b)?;
                let w = Self::select_width(tb.ty)?;
                if h < l {
                    return Err(Reject::Invalid("E2007"));
                }
                if *h >= w {
                    return Err(Reject::Invalid("E2006"));
                }
                Ok(Typed::new(Ty::Bits(h - l + 1), tb.tags))
            }
            Expr::Part(b, s, pw, up) => {
                let tb = self.synth(b)?;
                let w = Self::select_width(tb.ty)?;
                if *pw == 0 || *pw > w {
                    return Err(Reject::Invalid("E2006"));
                }
                let ts = self.synth(s)?;
                match ts.ty {
                    Ty::Lit(v) => {
                        let (lo, hi) = if *up {
                            (v, v + i128::from(*pw) - 1)
                        } else {
                            (v - i128::from(*pw) + 1, v)
                        };
                        if lo < 0 || hi >= i128::from(w) {
                            return Err(Reject::Invalid("E2006"));
                        }
                    }
                    Ty::Int { signed: false, .. } => {}
                    Ty::Int { signed: true, .. } => {
                        return Err(Reject::Unspecified("signed part-select start"))
                    }
                    Ty::Bits(_) => return Err(Reject::Unspecified("bits<N> part-select start")),
                    Ty::Bool => return Err(Reject::Invalid("?")),
                }
                Ok(Typed::new(Ty::Bits(*pw), tb.tags | ts.tags))
            }
            Expr::Cast(x, t) => {
                let tx = self.synth(x)?;
                let (ty, tags) = Self::cast(tx.ty, *t)?;
                Ok(Typed::new(ty, tx.tags | tags))
            }
            Expr::Concat(_) => Err(Reject::Unsupported(
                "concat(): no SystemVerilog mapping yet (E0003, ADR-0072)",
            )),
        }
    }

    /// Seçimin gördüğü genişlik: bileşik değerin doğal genişliği
    /// (ADR-0106 §4), `bits<N>`'in N'i.
    fn select_width(t: Ty) -> R<u32> {
        match t {
            Ty::Int { hi, .. } => Ok(hi),
            Ty::Bits(w) => Ok(w),
            Ty::Bool => Err(Reject::Invalid("?")),
            Ty::Lit(_) => Err(Reject::Unspecified("select of an untyped literal")),
        }
    }

    fn unary(&self, op: UnOp, t: Ty) -> R<(Ty, u32)> {
        match (op, t) {
            (UnOp::Not, Ty::Bool) => Ok((Ty::Bool, 0)),
            (UnOp::Not, _) => Err(Reject::Invalid("?")),
            // §3.4: `~x` genişlik korunur.
            (UnOp::BitNot, Ty::Bool) => Ok((Ty::Bool, tag::BOOL_BITNOT)),
            (UnOp::BitNot, Ty::Int { lo, hi, .. }) => {
                Ok((t, if lo != hi { tag::NOT_FLEX } else { 0 }))
            }
            (UnOp::BitNot, Ty::Bits(_)) => Ok((t, 0)),
            (UnOp::BitNot, Ty::Lit(_)) => Err(Reject::Unspecified("~ of an untyped literal")),
            // §3.4: `-x` işaretli olmalı, 1 bit genişler; ADR-0025 eki: somut i(N+1).
            (
                UnOp::Neg,
                Ty::Int {
                    signed: true,
                    lo,
                    hi,
                },
            ) => {
                if hi + 1 > MAX_INT {
                    return Err(Reject::Unspecified("natural width over 64"));
                }
                Ok((
                    Ty::int(true, hi + 1),
                    if lo != hi { tag::NEG_FLEX } else { 0 },
                ))
            }
            (UnOp::Neg, Ty::Int { signed: false, .. }) => Err(Reject::Invalid("E2002")),
            (UnOp::Neg, Ty::Lit(v)) => Ok((Ty::Lit(-v), 0)),
            (UnOp::Neg, _) => Err(Reject::Invalid("?")),
        }
    }

    fn binary(&self, op: BinOp, a: Ty, b: Ty) -> R<(Ty, u32)> {
        if op.is_arith() {
            return match (a, b) {
                (Ty::Lit(_), Ty::Lit(_)) => Err(Reject::Unspecified("literal-literal arithmetic")),
                // §3.3: literal somut tipe uyarlanır.
                (Ty::Lit(v), t @ Ty::Int { .. }) | (t @ Ty::Int { .. }, Ty::Lit(v)) => {
                    let lt = Self::lit_in(v, t)?;
                    let (ty, tags) = Self::int_arith(op, t, t)?;
                    Ok((ty, tags | lt))
                }
                (Ty::Int { .. }, Ty::Int { .. }) => Self::int_arith(op, a, b),
                (Ty::Bits(_), _) | (_, Ty::Bits(_)) => Err(Reject::Invalid("E2004")),
                _ => Err(Reject::Invalid("?")),
            };
        }
        if op.is_bitwise() {
            // §3.3 "Bit Düzeyi: Genişlemez"; ADR-0025: kesişim aralığı korunur.
            return match (a, b) {
                (Ty::Bool, Ty::Bool) => Ok((Ty::Bool, 0)),
                (Ty::Bits(x), Ty::Bits(y)) if x == y => Ok((Ty::Bits(x), 0)),
                (Ty::Bits(_), Ty::Bits(_)) => Err(Reject::Invalid("?")),
                (Ty::Lit(v), t @ Ty::Int { .. }) | (t @ Ty::Int { .. }, Ty::Lit(v)) => {
                    let lt = Self::lit_in(v, t)?;
                    Ok((t, lt))
                }
                (
                    Ty::Int {
                        signed: s1,
                        lo: l1,
                        hi: h1,
                    },
                    Ty::Int {
                        signed: s2,
                        lo: l2,
                        hi: h2,
                    },
                ) => {
                    if s1 != s2 {
                        return Err(Reject::Invalid("?"));
                    }
                    let (lo, hi) = meet((l1, h1), (l2, h2)).ok_or(Reject::Invalid("E2001"))?;
                    Ok((Ty::Int { signed: s1, lo, hi }, 0))
                }
                (Ty::Lit(_), Ty::Lit(_)) => Err(Reject::Unspecified("literal-literal bitwise")),
                (Ty::Lit(_), Ty::Bits(_)) | (Ty::Bits(_), Ty::Lit(_)) => Err(Reject::Unspecified(
                    "literal in a bits<N> bitwise operation",
                )),
                _ => Err(Reject::Invalid("?")),
            };
        }
        if op.is_shift() {
            // §3.3 "Kaydırma: Sol Operandın Tipi"; ADR-0036 işaretli `>>`.
            let mut tags = 0;
            let w = match a {
                Ty::Int { lo, hi, .. } => {
                    if lo != hi {
                        tags |= tag::SHIFT_FLEX;
                    }
                    hi
                }
                Ty::Bits(w) => w,
                Ty::Lit(_) => return Err(Reject::Unspecified("shift of an untyped literal")),
                Ty::Bool => return Err(Reject::Invalid("?")),
            };
            let max_amount: u128 = match b {
                Ty::Lit(v) if v >= 0 => v as u128,
                Ty::Lit(_) => return Err(Reject::Unspecified("negative shift amount")),
                Ty::Int {
                    signed: false, hi, ..
                } => mask(hi),
                Ty::Int { signed: true, .. } => {
                    return Err(Reject::Unspecified("signed shift amount"))
                }
                Ty::Bits(_) => return Err(Reject::Unspecified("bits<N> shift amount")),
                Ty::Bool => return Err(Reject::Invalid("?")),
            };
            if op == BinOp::Shr
                && matches!(a, Ty::Int { signed: true, .. })
                && max_amount >= u128::from(w)
            {
                tags |= tag::SHR_OVERSHIFT;
            }
            return Ok((a, tags));
        }
        // Karşılaştırma (§3.3, ADR-0106 §1).
        let ordering = !matches!(op, BinOp::Eq | BinOp::Ne);
        match (a, b) {
            (Ty::Bool, Ty::Bool) | (Ty::Bits(_), Ty::Bits(_)) if ordering => Err(
                Reject::Unspecified("ordering comparison of bool or bits<N>"),
            ),
            (Ty::Bool, Ty::Bool) => Ok((Ty::Bool, 0)),
            (Ty::Bits(x), Ty::Bits(y)) if x == y => Ok((Ty::Bool, 0)),
            (Ty::Bits(_), Ty::Bits(_)) => Err(Reject::Invalid("?")),
            (
                Ty::Int {
                    signed: s1,
                    lo: l1,
                    hi: h1,
                },
                Ty::Int {
                    signed: s2,
                    lo: l2,
                    hi: h2,
                },
            ) => {
                // Karşılaştırmada işaret farkının kodunu spesifikasyon
                // adlandırmaz (§3.3 `unify_for_comparison`: "aynı tip olmalı").
                if s1 != s2 {
                    return Err(Reject::Invalid("?"));
                }
                if meet((l1, h1), (l2, h2)).is_none() {
                    return Err(Reject::Invalid("E2003"));
                }
                if h1 != h2 {
                    return Err(Reject::Invalid("E2031"));
                }
                Ok((Ty::Bool, 0))
            }
            // ADR-0106 §2: literal geniş yanın doğal genişliğinde.
            (Ty::Lit(v), Ty::Int { signed, hi, .. }) | (Ty::Int { signed, hi, .. }, Ty::Lit(v)) => {
                if fits(v, hi, signed) {
                    Ok((Ty::Bool, 0))
                } else {
                    Err(Reject::Invalid("E2010"))
                }
            }
            (Ty::Lit(_), Ty::Lit(_)) => Err(Reject::Unspecified("literal-literal comparison")),
            _ => Err(Reject::Invalid("?")),
        }
    }

    /// Somut ya da esnek tam sayı yanındaki literal (§3.3, §4).
    fn lit_in(v: i128, t: Ty) -> R<u32> {
        let Ty::Int { signed, lo, hi } = t else {
            unreachable!()
        };
        if !fits(v, hi, signed) {
            return Err(Reject::Invalid("E2010"));
        }
        Ok(if fits(v, lo, signed) {
            0
        } else {
            tag::LIT_FLEX
        })
    }

    /// §3.3 + ADR-0025: işaret aynı, aralıklar kesişmeli; `+ -` 1 bit,
    /// `*` iki kat.
    fn int_arith(op: BinOp, a: Ty, b: Ty) -> R<(Ty, u32)> {
        let (
            Ty::Int {
                signed: s1,
                lo: l1,
                hi: h1,
            },
            Ty::Int {
                signed: s2,
                lo: l2,
                hi: h2,
            },
        ) = (a, b)
        else {
            unreachable!()
        };
        if s1 != s2 {
            return Err(Reject::Invalid("E2002"));
        }
        let (lo, c) = meet((l1, h1), (l2, h2)).ok_or(Reject::Invalid("E2001"))?;
        let n = match op {
            BinOp::Add | BinOp::Sub => c + 1,
            BinOp::Mul => 2 * c,
            _ => unreachable!(),
        };
        if n > MAX_INT {
            return Err(Reject::Unspecified("natural width over 64"));
        }
        let mut tags = 0;
        if h1.max(h2) > c {
            tags |= tag::FLEX_OPERAND;
        }
        if op == BinOp::Sub && !s1 {
            tags |= tag::UNSIGNED_SUB;
        }
        Ok((
            Ty::Int {
                signed: s1,
                lo,
                hi: n,
            },
            tags,
        ))
    }

    /// §3.6 `check_cast_legal`; esnek kaynak doğal genişliğine sabitlenir
    /// (ADR-0025).
    fn cast(src: Ty, t: PortTy) -> R<(Ty, u32)> {
        let e2009 = Err(Reject::Invalid("E2009"));
        match (src, t) {
            (Ty::Int { signed, hi, .. }, PortTy::UInt(b) | PortTy::SInt(b)) => {
                let ts = t.signed();
                let mut tags = 0;
                if signed == ts && b < hi && signed {
                    tags |= tag::SIGNED_NARROW;
                }
                if signed != ts && b > hi && signed {
                    tags |= tag::CAST_SIGN_EXT;
                }
                Ok((Ty::int(ts, b), tags))
            }
            (Ty::Int { signed, hi, .. }, PortTy::Bits(b)) => {
                if hi != b {
                    return e2009;
                }
                Ok((Ty::Bits(b), if signed { tag::SINT_TO_BITS } else { 0 }))
            }
            (
                Ty::Int {
                    signed: false,
                    hi: 1,
                    ..
                },
                PortTy::Bool,
            ) => Ok((Ty::Bool, 0)),
            (Ty::Int { .. }, PortTy::Bool) => e2009,
            (Ty::Bits(a), PortTy::UInt(b) | PortTy::SInt(b)) if a == b => {
                Ok((Ty::int(t.signed(), b), 0))
            }
            (Ty::Bits(_), _) => e2009,
            (Ty::Bool, PortTy::UInt(1)) => Ok((Ty::int(false, 1), 0)),
            (Ty::Bool, _) => e2009,
            (Ty::Lit(_), _) => Err(Reject::Unspecified("cast of an untyped literal")),
        }
    }

    // ── Değer ───────────────────────────────────────────────────────

    /// Doğal genişliğinde değer örüntüsü; `None` = SV'de x (sınır dışı
    /// değişken seçim, ADR-0035 §1) ya da tanımsız.
    pub fn eval(&self, e: &Expr, inp: &[u128]) -> Option<u128> {
        match e {
            Expr::Port(k) => Some(inp[*k] & mask(self.ports[*k].width())),
            Expr::Name(_, b) => self.eval(b, inp),
            Expr::Lit(v, _) => Some(*v as u128),
            Expr::Un(op, x) => {
                let t = self.synth(x).ok()?.ty;
                let v = self.eval(x, inp)?;
                match (op, t) {
                    (UnOp::Not, _) => Some(v ^ 1),
                    (UnOp::BitNot, _) => Some(!v & mask(t.width())),
                    (UnOp::Neg, Ty::Int { hi, .. }) => {
                        let n = to_i128(v, hi, true);
                        Some((n.wrapping_neg() as u128) & mask(hi + 1))
                    }
                    (UnOp::Neg, Ty::Lit(n)) => Some(n.wrapping_neg() as u128),
                    _ => None,
                }
            }
            Expr::Bin(op, x, y) => self.eval_bin(*op, x, y, inp),
            Expr::Index(b, i) => {
                let w = Self::select_width(self.synth(b).ok()?.ty).ok()?;
                let v = self.eval(b, inp)?;
                let idx = self.eval_unsigned(i, inp)?;
                (idx < u128::from(w)).then(|| (v >> idx) & 1)
            }
            Expr::Range(b, h, l) => {
                if h < l {
                    return None;
                }
                let v = self.eval(b, inp)?;
                Some((v >> l) & mask(h - l + 1))
            }
            Expr::Part(b, s, pw, up) => {
                let w = u128::from(Self::select_width(self.synth(b).ok()?.ty).ok()?);
                let v = self.eval(b, inp)?;
                let st = self.eval_unsigned(s, inp)?;
                let pw128 = u128::from(*pw);
                let lo = if *up {
                    if st + pw128 > w {
                        return None;
                    }
                    st
                } else {
                    if st >= w || st + 1 < pw128 {
                        return None;
                    }
                    st + 1 - pw128
                };
                Some((v >> lo) & mask(*pw))
            }
            Expr::Cast(x, t) => {
                let src = self.synth(x).ok()?.ty;
                let v = self.eval(x, inp)?;
                match (src, t) {
                    (Ty::Int { signed, hi, .. }, PortTy::UInt(b) | PortTy::SInt(b)) => {
                        Some(conv(v, hi, signed, *b))
                    }
                    (Ty::Int { hi, .. }, PortTy::Bits(_)) => Some(v & mask(hi)),
                    (Ty::Int { .. }, PortTy::Bool) => Some(v & 1),
                    (Ty::Bits(w), _) => Some(v & mask(w)),
                    (Ty::Bool, _) => Some(v & 1),
                    (Ty::Lit(_), _) => None,
                }
            }
            Expr::Concat(_) => None,
        }
    }

    /// İndeks/başlangıç/kaydırma miktarı: işaretsiz değer.
    fn eval_unsigned(&self, e: &Expr, inp: &[u128]) -> Option<u128> {
        match self.synth(e).ok()?.ty {
            Ty::Lit(v) => (v >= 0).then_some(v as u128),
            Ty::Int { signed: false, .. } => self.eval(e, inp),
            _ => None,
        }
    }

    /// İşleneni `n` bite dönüştürür: literal ikiye tümleyenle, tam sayı
    /// kendi işaretiyle.
    fn operand(&self, e: &Expr, n: u32, inp: &[u128]) -> Option<u128> {
        let t = self.synth(e).ok()?.ty;
        let v = self.eval(e, inp)?;
        Some(match t {
            Ty::Lit(_) => v & mask(n),
            Ty::Int { signed, hi, .. } => conv(v, hi, signed, n),
            Ty::Bits(w) => conv(v, w, false, n),
            Ty::Bool => v & 1,
        })
    }

    fn eval_bin(&self, op: BinOp, x: &Expr, y: &Expr, inp: &[u128]) -> Option<u128> {
        let tx = self.synth(x).ok()?.ty;
        let ty = self.synth(y).ok()?.ty;
        if op.is_arith() || op.is_bitwise() {
            let rt = self.binary(op, tx, ty).ok()?.0;
            let n = rt.width();
            let a = self.operand(x, n, inp)?;
            let b = self.operand(y, n, inp)?;
            let r = match op {
                BinOp::Add => a.wrapping_add(b),
                BinOp::Sub => a.wrapping_sub(b),
                BinOp::Mul => a.wrapping_mul(b),
                BinOp::And => a & b,
                BinOp::Or => a | b,
                BinOp::Xor => a ^ b,
                _ => unreachable!(),
            };
            return Some(r & mask(n));
        }
        if op.is_shift() {
            let w = tx.width();
            let v = self.eval(x, inp)? & mask(w);
            let amt = self.eval_unsigned(y, inp)?;
            let signed = matches!(tx, Ty::Int { signed: true, .. });
            let big = amt >= u128::from(w);
            return Some(match op {
                BinOp::Shl if big => 0,
                BinOp::Shl => (v << amt) & mask(w),
                BinOp::Shr if !signed && big => 0,
                BinOp::Shr if !signed => v >> amt,
                _ => {
                    let s = to_i128(v, w, true);
                    let sh = if big { 127 } else { amt as u32 };
                    ((s >> sh) as u128) & mask(w)
                }
            });
        }
        // Karşılaştırma: iki yan aynı doğal genişlikte, işaretliyse işaretli.
        let num = |t: Ty, v: u128| -> i128 {
            match t {
                Ty::Lit(n) => n,
                Ty::Int { signed, hi, .. } => to_i128(v, hi, signed),
                _ => v as i128,
            }
        };
        let a = num(tx, self.eval(x, inp)?);
        let b = num(ty, self.eval(y, inp)?);
        let r = match op {
            BinOp::Eq => a == b,
            BinOp::Ne => a != b,
            BinOp::Lt => a < b,
            BinOp::Le => a <= b,
            BinOp::Gt => a > b,
            BinOp::Ge => a >= b,
            _ => unreachable!(),
        };
        Some(u128::from(r))
    }

    // ── Check kipi (§4, §5, ADR-0041) ───────────────────────────────

    /// ADR-0041 itmesi uygulanıyor mu? `e` aritmetik ikili işlemse ve
    /// literal olmayan her işlenen int tipli, hedefle aynı işaretli ve
    /// doğal genişliği hedefe sığıyorsa işlenen tipleri.
    fn push(&self, e: &Expr, signed: bool, w: u32) -> R<Option<u32>> {
        let Expr::Bin(op, x, y) = e else {
            return Ok(None);
        };
        if !op.is_arith() {
            return Ok(None);
        }
        let mut tags = 0;
        let mut lits = 0;
        for side in [x, y] {
            let t = self.synth(side)?;
            tags |= t.tags;
            match t.ty {
                Ty::Lit(v) => {
                    lits += 1;
                    if !fits(v, w, signed) {
                        return Err(Reject::Invalid("E2010"));
                    }
                }
                Ty::Int { signed: s, hi, .. } if s == signed && hi <= w => {
                    if let Expr::Bin(inner, ..) = &**side {
                        if inner.is_arith() && hi < w {
                            tags |= tag::PUSH_NESTED;
                        }
                    }
                }
                _ => return Ok(None),
            }
        }
        if lits == 2 {
            return Ok(None);
        }
        if *op == BinOp::Sub && !signed {
            tags |= tag::UNSIGNED_SUB;
        }
        Ok(Some(tags))
    }

    /// `target` tipine atanan `e` geçerli mi (§5, ADR-0041); varsayım
    /// etiketleri.
    pub fn check(&self, e: &Expr, target: PortTy) -> R<u32> {
        match target {
            PortTy::Bool => match self.synth(e)? {
                Typed { ty: Ty::Bool, tags } => Ok(tags),
                _ => Err(Reject::Invalid("E2003")),
            },
            PortTy::Bits(w) => match self.synth(e)? {
                Typed {
                    ty: Ty::Bits(x),
                    tags,
                } if x == w => Ok(tags),
                Typed {
                    ty: Ty::Bits(_), ..
                } => Err(Reject::Invalid("E2001")),
                _ => Err(Reject::Invalid("E2003")),
            },
            PortTy::UInt(w) | PortTy::SInt(w) => {
                let signed = target.signed();
                if let Some(tags) = self.push(e, signed, w)? {
                    return Ok(tags);
                }
                let t = self.synth(e)?;
                match t.ty {
                    Ty::Int { signed: s, .. } if s != signed => Err(Reject::Invalid("E2002")),
                    Ty::Int { lo, .. } if w < lo => Err(Reject::Invalid("E2001")),
                    Ty::Int { .. } => Ok(t.tags),
                    Ty::Lit(v) if fits(v, w, signed) => Ok(t.tags),
                    Ty::Lit(_) => Err(Reject::Invalid("E2010")),
                    _ => Err(Reject::Invalid("E2003")),
                }
            }
        }
    }

    /// `target`'e atanmış `e`'nin değeri (`check` geçerli dediyse).
    pub fn eval_check(&self, e: &Expr, target: PortTy, inp: &[u128]) -> Option<u128> {
        match target {
            PortTy::Bool | PortTy::Bits(_) => self.eval(e, inp),
            PortTy::UInt(w) | PortTy::SInt(w) => {
                let signed = target.signed();
                if let Ok(Some(_)) = self.push(e, signed, w) {
                    let Expr::Bin(op, x, y) = e else {
                        unreachable!()
                    };
                    let a = self.operand(x, w, inp)?;
                    let b = self.operand(y, w, inp)?;
                    let r = match op {
                        BinOp::Add => a.wrapping_add(b),
                        BinOp::Sub => a.wrapping_sub(b),
                        BinOp::Mul => a.wrapping_mul(b),
                        _ => unreachable!(),
                    };
                    return Some(r & mask(w));
                }
                let t = self.synth(e).ok()?.ty;
                let v = self.eval(e, inp)?;
                Some(match t {
                    Ty::Int { signed: s, hi, .. } => conv(v, hi, s, w),
                    _ => v & mask(w),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Expr as E, LitFmt};

    fn p(k: usize) -> E {
        E::Port(k)
    }

    fn lit(v: i128) -> E {
        E::Lit(v, LitFmt::Dec)
    }

    const PORTS: &[PortTy] = &[
        PortTy::UInt(8),  // 0: a
        PortTy::UInt(8),  // 1: b
        PortTy::UInt(16), // 2
        PortTy::SInt(8),  // 3
        PortTy::Bits(8),  // 4
        PortTy::Bool,     // 5
        PortTy::UInt(3),  // 6
        PortTy::UInt(4),  // 7
    ];

    fn m() -> Model<'static> {
        Model { ports: PORTS }
    }

    fn ty(e: &E) -> Result<Ty, Reject> {
        m().synth(e).map(|t| t.ty)
    }

    /// type-inference.md §10 test vektörleri.
    #[test]
    fn spec_section_10_vectors() {
        assert_eq!(ty(&lit(42)), Ok(Ty::Lit(42)));
        let add = E::bin(BinOp::Add, p(0), p(1));
        assert_eq!(ty(&add).unwrap().natural(), Some(PortTy::UInt(9)));
        let mul = E::bin(BinOp::Mul, p(0), p(1));
        assert_eq!(ty(&mul).unwrap().natural(), Some(PortTy::UInt(16)));
        let and = E::bin(BinOp::And, p(0), p(1));
        assert_eq!(ty(&and), Ok(Ty::int(false, 8)));
        assert_eq!(ty(&E::bin(BinOp::Eq, p(0), p(1))), Ok(Ty::Bool));
        assert_eq!(ty(&E::un(UnOp::Not, p(5))), Ok(Ty::Bool));
        assert_eq!(ty(&E::un(UnOp::BitNot, p(0))), Ok(Ty::int(false, 8)));
        assert_eq!(ty(&E::un(UnOp::Neg, p(3))), Ok(Ty::int(true, 9)));
        assert_eq!(
            ty(&E::Index(Box::new(p(0)), Box::new(lit(3)))),
            Ok(Ty::Bool)
        );
        assert_eq!(ty(&E::Range(Box::new(p(0)), 7, 4)), Ok(Ty::Bits(4)));
        assert_eq!(ty(&E::cast(p(0), PortTy::UInt(16))), Ok(Ty::int(false, 16)));
        // Hatalar
        assert_eq!(
            ty(&E::bin(BinOp::Add, p(0), p(2))),
            Err(Reject::Invalid("E2001"))
        );
        assert_eq!(
            ty(&E::bin(BinOp::Add, p(0), p(3))),
            Err(Reject::Invalid("E2002"))
        );
        assert_eq!(
            ty(&E::bin(BinOp::Add, p(4), p(4))),
            Err(Reject::Invalid("E2004"))
        );
        assert_eq!(
            ty(&E::Index(Box::new(p(0)), Box::new(lit(9)))),
            Err(Reject::Invalid("E2006"))
        );
    }

    /// ADR-0025: u16 + (u16 * u16 → [16, 32]) 16'da buluşur.
    #[test]
    fn flex_ranges_meet_by_intersection() {
        let mul = E::bin(BinOp::Mul, p(2), p(2));
        assert_eq!(
            ty(&mul),
            Ok(Ty::Int {
                signed: false,
                lo: 16,
                hi: 32
            })
        );
        let sum = E::bin(BinOp::Add, p(2), mul);
        assert_eq!(
            ty(&sum),
            Ok(Ty::Int {
                signed: false,
                lo: 16,
                hi: 17
            })
        );
    }

    /// ADR-0106: farklı doğal genişlikte karşılaştırma E2031; literal
    /// geniş yanın doğal genişliğinde (c + d < 300 doğru okunur).
    #[test]
    fn comparison_width_rules() {
        let b1 = E::bin(BinOp::Add, p(6), lit(1));
        assert_eq!(
            ty(&E::bin(BinOp::Eq, p(6), b1)),
            Err(Reject::Invalid("E2031"))
        );
        let cd = E::bin(BinOp::Add, p(0), p(1));
        let lt = E::bin(BinOp::Lt, cd, lit(300));
        assert_eq!(ty(&lt), Ok(Ty::Bool));
        // 250 + 11 = 261 < 300: doğru
        let mut inp = vec![0u128; PORTS.len()];
        inp[0] = 250;
        inp[1] = 11;
        assert_eq!(m().eval(&lt, &inp), Some(1));
    }

    /// ADR-0106 §6: `a << 4` u8'dir (a = 255 → 240), `~a` u8'dir (0 → 255),
    /// daha geniş hedefe atanınca da.
    #[test]
    fn self_sized_operands_in_a_wider_target() {
        let mut inp = vec![0u128; PORTS.len()];
        inp[0] = 255;
        let sh = E::bin(BinOp::Shl, p(0), lit(4));
        assert_eq!(m().eval_check(&sh, PortTy::UInt(12), &inp), Some(240));
        inp[0] = 0;
        let not = E::bin(BinOp::Add, E::un(UnOp::BitNot, p(0)), lit(0));
        assert_eq!(m().check(&not, PortTy::UInt(12)), Ok(0));
        assert_eq!(m().eval_check(&not, PortTy::UInt(12), &inp), Some(255));
    }

    /// ADR-0041: `let c : u10 = a + b` hedefte toplar; aynı ifadenin
    /// doğal değeri (u9) genişletilince de aynı. İşaretsiz çıkarmada
    /// itme sonucu değiştirir (A3).
    #[test]
    fn check_mode_push() {
        let mut inp = vec![0u128; PORTS.len()];
        inp[0] = 0;
        inp[1] = 1;
        let sub = E::bin(BinOp::Sub, p(0), p(1));
        assert_eq!(m().eval(&sub, &inp), Some(511));
        assert_eq!(m().eval_check(&sub, PortTy::UInt(9), &inp), Some(511));
        assert_eq!(m().eval_check(&sub, PortTy::UInt(10), &inp), Some(1023));
        let t = m().check(&sub, PortTy::UInt(10)).unwrap();
        assert_ne!(t & tag::UNSIGNED_SUB, 0);
        // a8 + u16 synth'te E2001, u16 hedefte itme geçerli kılar.
        let mixed = E::bin(BinOp::Add, p(0), p(2));
        assert_eq!(m().check(&mixed, PortTy::UInt(16)), Ok(0));
    }

    /// ADR-0106 §4: bileşik değerden seçim doğal genişlikte.
    #[test]
    fn select_of_a_sum_reads_the_carry() {
        let mut inp = vec![0u128; PORTS.len()];
        inp[0] = 200;
        inp[1] = 100;
        let sum = E::bin(BinOp::Add, p(0), p(1));
        let carry = E::Index(Box::new(sum.clone()), Box::new(lit(8)));
        assert_eq!(m().eval(&carry, &inp), Some(1));
        let hi = E::Range(Box::new(sum), 8, 5);
        assert_eq!(m().eval(&hi, &inp), Some(300 >> 5));
    }

    #[test]
    fn signed_shift_and_cast() {
        let mut inp = vec![0u128; PORTS.len()];
        inp[3] = 0x80; // -128
        let sh = E::bin(BinOp::Shr, p(3), lit(2));
        assert_eq!(m().eval(&sh, &inp), Some(0xE0));
        let wide = E::cast(p(3), PortTy::SInt(12));
        assert_eq!(m().eval(&wide, &inp), Some(0xF80));
        let u = E::cast(p(3), PortTy::UInt(8));
        assert_eq!(m().eval(&u, &inp), Some(0x80));
    }

    #[test]
    fn variable_part_select_out_of_range_is_x() {
        let mut inp = vec![0u128; PORTS.len()];
        let part = E::Part(Box::new(p(0)), Box::new(p(7)), 4, true);
        inp[7] = 4;
        inp[0] = 0xA5;
        assert_eq!(m().eval(&part, &inp), Some(0xA));
        inp[7] = 5;
        assert_eq!(m().eval(&part, &inp), None);
    }
}
