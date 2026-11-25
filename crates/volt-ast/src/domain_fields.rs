//! Bir `domain` bildiriminin alanları tek yerden okunur (#420, ADR-0140;
//! ADR-0137 §1).
//!
//! Bir bildirimde her alan anahtarı en çok bir kez yazılır; ikinci yazım
//! E1003'tür ve onu ayrıştırıcı, bildirimi kurduğu yerde
//! [`DomainDecl::repeated_fields`]'tan raporlar. Alanı okuyan her geçit
//! (tip denetiminin alan tablosu, RDC, kısıt modeli, SV üretici) onu
//! buradaki yöntemlerden alır. #420'den önce her geçidin kendi döngüsü
//! vardı ve yinelenen alanda ayrışıyorlardı: SV ve RDC son `reset`
//! alanını, kısıt modeli "bir alan `none` diyorsa reset'siz" kuralını,
//! saat kenarında kısıt modeli ilk alanı okuyordu.
//!
//! Okunan alan anahtarın ilk yazımıdır. Yinelenen alan zaten bir hatadır;
//! hatalı birimi okuyan geçit (LSP, hata sonrası denetimler) bu yüzden
//! E1003'ün "ilk burada" dediği alanı görür.

use crate::{
    ClockEdge, DomainDecl, DomainField, DomainKey, DomainValue, ResetPolarity, ResetSpec,
    ResetSync, TrustLevel,
};
use volt_span::Span;

impl DomainKey {
    /// Kaynaktaki yazımı; bilinmeyen anahtarda yazıldığı ad.
    pub fn as_str(&self) -> &str {
        match self {
            DomainKey::Clock => "clock",
            DomainKey::Frequency => "frequency",
            DomainKey::Reset => "reset",
            DomainKey::ResetCycles => "reset_cycles",
            DomainKey::ResetSequence => "reset_sequence",
            DomainKey::TrustLevel => "trust_level",
            DomainKey::Unknown(name) => &name.text,
        }
    }
}

impl DomainDecl {
    /// `key` anahtarlı alan: bildirimdeki ilk yazımı. Bilinmeyen anahtar
    /// (W0020) `Name`'iyle, konumu dahil karşılaştırılır: iki yazımı
    /// hiçbir zaman aynı anahtar değildir.
    pub fn field(&self, key: &DomainKey) -> Option<&DomainField> {
        self.fields.iter().find(|f| f.key == *key)
    }

    /// Aynı anahtarın ilkinden sonraki her yazımı, ilk yazımla birlikte:
    /// `(ilk, yinelenen)`, kaynak sırasıyla (E1003). Bilinmeyen anahtar
    /// bir alan değildir, yinelenmez ([`DomainDecl::field`]).
    pub fn repeated_fields(&self) -> Vec<(&DomainField, &DomainField)> {
        self.fields
            .iter()
            .filter_map(|f| {
                let first = self.field(&f.key)?;
                (!std::ptr::eq(first, f)).then_some((first, f))
            })
            .collect()
    }

    /// `clock = posedge | negedge | none`. Yazılmamışsa ya da değeri kenar
    /// değilse `None`.
    pub fn clock_edge(&self) -> Option<ClockEdge> {
        match self.field(&DomainKey::Clock)?.value {
            DomainValue::ClockEdge(edge) => Some(edge),
            _ => None,
        }
    }

    /// `reset = sync|async active_high|active_low` ya da `reset = none`,
    /// alanın konumuyla. `reset = none` `ResetSync::None` ile döner
    /// (polarite anlamsızdır, `ActiveHigh` yazılır). Yazılmamışsa ya da
    /// değeri bir reset değilse `None`: varsayılanı çağıran verir.
    pub fn reset(&self) -> Option<(ResetSpec, Span)> {
        let field = self.field(&DomainKey::Reset)?;
        let spec = match field.value {
            DomainValue::Reset(spec) => spec,
            DomainValue::ClockEdge(ClockEdge::None) => ResetSpec {
                sync: ResetSync::None,
                polarity: ResetPolarity::ActiveHigh,
            },
            _ => return None,
        };
        Some((spec, field.span))
    }

    /// `trust_level = secret | confidential | public` (ADR-0052), alanın
    /// konumuyla.
    pub fn trust_level(&self) -> Option<(TrustLevel, Span)> {
        let field = self.field(&DomainKey::TrustLevel)?;
        match field.value {
            DomainValue::Trust(level) => Some((level, field.span)),
            _ => None,
        }
    }

    /// `frequency = <literal>` alanı; değerini kısıt modeli ayrıştırır.
    pub fn frequency(&self) -> Option<&DomainField> {
        self.field(&DomainKey::Frequency)
    }
}
