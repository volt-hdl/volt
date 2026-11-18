//! `match`'te tasarıma yazılmayan kol kuralı (ADR-0075, sv-mapping.md
//! §19.1): kuralın tek yeri, [`dead_arms`].
//!
//! İki neden: (1) ilk muhafızsız joker koldan (`_` ya da onu içeren `|`)
//! sonraki her kol hiç seçilmez — ilk eşleşen kol kazanır, joker her değeri
//! yakalar (#216); (2) muhafızsız bir kolun bütün değerleri (literal,
//! sabit adı, enum varyantı) önceki muhafızsız kollarda geçtiyse kol hiç
//! seçilmez (ADR-0074 Karar 4, ADR-0075). Böyle kol tip denetiminden geçer
//! ve W2014 alır, ama tasarımda yoktur: sv-emit onu yazmaz (deyim
//! `case`'inde de ifadenin `case`/üçlü zincirinde de), FSM kontrat
//! taraması onu görmez, donanımı inceleyen hiçbir HIR analizi (sürücü,
//! saat alanı, güven, sıfırlama alanı, el sıkışma, zamanlama) ona girmez
//! (#328, #331, #332).
//!
//! Desen yaprağının (literal, yol, tuple, hatalı desen) neyi adlandırdığını
//! katman söyler ([`Leaf`]): sabit adının değeri katmanın kendi sabit
//! değerlendiricisinden gelir, enum varyantı katmanın ad çözümünden. Kolun
//! nasıl yargılandığı — joker, muhafız, `|` alternatifleri, kapsanma —
//! yalnız buradadır.

use crate::{Expr, ExprKind, Idx, MatchArm, Pattern, PatternKind, SourceFile, UnOp};

/// Desen literalinin değeri: `(negatif, büyüklük)` ya da bool. Yazım
/// biçimi (`1`, `0x1`) değeri değiştirmez; `-0` sıfırdır.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LitKey {
    Int(bool, u128),
    Bool(bool),
}

/// Kolun neden hiç seçilmediği.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unreachable {
    /// Önceki muhafızsız bir joker kol her değeri yakaladı.
    AfterCatchAll,
    /// Bütün değerleri önceki muhafızsız kollarda geçti.
    Covered,
}

/// Bir desen yaprağının (joker ve `|` dışındaki desen) adlandırdığı değer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Leaf<K> {
    /// Tek, bilinen değer (sayı, bool ya da enum varyantı).
    Key(K),
    /// Değer bilinmiyor (değerlendirilemeyen ad, tuple): kol kapsanmaya
    /// göre yargılanmaz, değer de eklemez.
    Unknown,
    /// Tanısı verilmiş hatalı desen: kapsamada her değeri yakalıyormuş gibi
    /// sayılır — sonraki kollar kapsanmış sayılmaz, enum match'i eksik
    /// varyant almaz (tanı kaskadı yok). Jokerden farkı: sonraki kollar
    /// [`Unreachable::AfterCatchAll`] olmaz.
    Opaque,
}

/// [`dead_arms`]'ın sonucu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArmCover<K> {
    /// Kol başına: tasarıma yazılmıyorsa nedeni.
    pub dead: Vec<Option<Unreachable>>,
    /// Canlı muhafızsız kolların adlandırdığı değerler, ilk geçtiği sırayla.
    pub covered: Vec<K>,
    /// Canlı muhafızsız bir kol her değeri yakalıyor: joker ya da
    /// [`Leaf::Opaque`] yapraklı kol.
    pub catch_all: bool,
    /// Kol başına değerleri: yalnız canlı, muhafızsız ve bütün yaprakları
    /// bilinen kolda `Some`.
    pub keys: Vec<Option<Vec<K>>>,
}

impl<K> ArmCover<K> {
    /// Kol tasarıma yazılmıyor mu.
    pub fn is_dead(&self, arm: usize) -> bool {
        self.dead.get(arm).is_some_and(Option::is_some)
    }
}

/// Kolun desen özeti.
enum ArmValues<K> {
    /// Joker içeriyor.
    Wild,
    /// Hatalı yaprak var.
    Opaque,
    /// Bilinmeyen yaprak var.
    Unknown,
    Keys(Vec<K>),
}

/// Tasarıma yazılmayan kollar: ilk muhafızsız joker koldan sonrakiler ve
/// önceki muhafızsız kolların kapsadıkları. Deyim ve ifade `match`'i aynı
/// kural (ADR-0083 Karar 1); sınananın tipinden bağımsız.
///
/// `leaf` her kolun (muhafızlı ve ölü kollar dahil) her yaprağı için kol
/// sırasıyla tam bir kez çağrılır; katman desen tanısını orada verebilir.
pub fn dead_arms<K: PartialEq + Clone>(
    ast: &SourceFile,
    arms: &[MatchArm],
    leaf: &mut dyn FnMut(Idx<Pattern>) -> Leaf<K>,
) -> ArmCover<K> {
    let values: Vec<ArmValues<K>> = arms
        .iter()
        .map(|arm| arm_values(ast, arm.pattern, leaf))
        .collect();
    let first = arms
        .iter()
        .zip(&values)
        .position(|(arm, v)| arm.guard.is_none() && matches!(v, ArmValues::Wild));
    let mut out = ArmCover {
        dead: vec![None; arms.len()],
        covered: Vec::new(),
        catch_all: false,
        keys: vec![None; arms.len()],
    };
    for (i, (arm, v)) in arms.iter().zip(values).enumerate() {
        if first.is_some_and(|f| i > f) {
            out.dead[i] = Some(Unreachable::AfterCatchAll);
            continue;
        }
        if arm.guard.is_some() {
            continue; // muhafızlı kol kapsamaya sayılmaz (E0003, ADR-0032)
        }
        let keys = match v {
            ArmValues::Wild | ArmValues::Opaque => {
                out.catch_all = true;
                continue;
            }
            ArmValues::Unknown => continue,
            ArmValues::Keys(keys) => keys,
        };
        if !out.catch_all && !keys.is_empty() && keys.iter().all(|k| out.covered.contains(k)) {
            out.dead[i] = Some(Unreachable::Covered);
            continue;
        }
        for k in &keys {
            if !out.covered.contains(k) {
                out.covered.push(k.clone());
            }
        }
        out.keys[i] = Some(keys);
    }
    out
}

/// Desenin özeti; her yaprak için `leaf` bir kez çağrılır (joker görülse
/// de alternatiflerin hepsi gezilir).
fn arm_values<K>(
    ast: &SourceFile,
    pat: Idx<Pattern>,
    leaf: &mut dyn FnMut(Idx<Pattern>) -> Leaf<K>,
) -> ArmValues<K> {
    match &ast.patterns[pat].kind {
        PatternKind::Wildcard => ArmValues::Wild,
        PatternKind::Or(alts) => {
            let mut acc = ArmValues::Keys(Vec::new());
            for &a in alts {
                acc = match (acc, arm_values(ast, a, leaf)) {
                    (ArmValues::Wild, _) | (_, ArmValues::Wild) => ArmValues::Wild,
                    (ArmValues::Opaque, _) | (_, ArmValues::Opaque) => ArmValues::Opaque,
                    (ArmValues::Unknown, _) | (_, ArmValues::Unknown) => ArmValues::Unknown,
                    (ArmValues::Keys(mut a), ArmValues::Keys(b)) => {
                        a.extend(b);
                        ArmValues::Keys(a)
                    }
                };
            }
            acc
        }
        PatternKind::Literal(_)
        | PatternKind::Path { .. }
        | PatternKind::Tuple(_)
        | PatternKind::Error => match leaf(pat) {
            Leaf::Key(k) => ArmValues::Keys(vec![k]),
            Leaf::Unknown => ArmValues::Unknown,
            Leaf::Opaque => ArmValues::Opaque,
        },
    }
}

/// Sayısal ya da bool sınananda bir yaprağın değeri: literal ve sabit adı
/// (`const_value` onun değerini verir, `None` değerlendirilemez); başka
/// yaprak (yol, tuple, hatalı desen) bilinmez.
pub fn value_leaf(
    ast: &SourceFile,
    pat: Idx<Pattern>,
    const_value: &mut dyn FnMut(Idx<Expr>) -> Option<i128>,
) -> Leaf<LitKey> {
    match &ast.patterns[pat].kind {
        PatternKind::Literal(expr) => match literal_key(ast, *expr, const_value) {
            Some(k) => Leaf::Key(k),
            None => Leaf::Unknown,
        },
        PatternKind::Wildcard
        | PatternKind::Or(_)
        | PatternKind::Path { .. }
        | PatternKind::Tuple(_)
        | PatternKind::Error => Leaf::Unknown,
    }
}

fn literal_key(
    ast: &SourceFile,
    expr: Idx<Expr>,
    const_value: &mut dyn FnMut(Idx<Expr>) -> Option<i128>,
) -> Option<LitKey> {
    match &ast.exprs[expr].kind {
        ExprKind::IntLit { value, .. } => Some(LitKey::Int(false, *value)),
        ExprKind::BoolLit(b) => Some(LitKey::Bool(*b)),
        ExprKind::Path(p) if p.segments.len() == 1 => {
            let v = const_value(expr)?;
            Some(LitKey::Int(v < 0, v.unsigned_abs()))
        }
        ExprKind::Unary {
            op: UnOp::Neg,
            operand,
        } => match literal_key(ast, *operand, const_value)? {
            LitKey::Int(_, 0) => Some(LitKey::Int(false, 0)),
            LitKey::Int(neg, v) => Some(LitKey::Int(!neg, v)),
            LitKey::Bool(_) => None,
        },
        _ => None,
    }
}
