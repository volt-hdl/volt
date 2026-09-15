//! `@mmio` register haritasının dil bağımsız özeti (ADR-0053).
//!
//! Parser `@reg` bildirimlerini tüketip RTL'e açar (ADR-0044); alt
//! geçitler haritayı hiç görmez. Yazılım tarafı (Rust/C sürücü,
//! `regmap.json`, Markdown belge) aynı bilgiden üretilir, bu yüzden
//! parser haritayı bu yan tabloya kopyalar ve `ParseResult.regmaps`
//! ile sürücüye döner. Yapı saf veridir: span, AST indeksi ya da
//! sentetik isim taşımaz; `volt-sw-emit` yalnız bunu okur.

/// Bir `@mmio` modülünün register haritası.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegMap {
    /// Modül adı (`Gpio`).
    pub module: String,
    /// `@mmio(base = ...)` — 32 bitlik taban adres.
    pub base: u64,
    /// `@mmio(bus = ...)` — şimdilik yalnız `AXI4Lite`.
    pub bus: String,
    /// Modülün `///` doc yorumu (satırlar `\n` ile ayrılmış).
    pub doc: Option<String>,
    /// Bildirim sırasına göre register'lar (offset sırası değil).
    pub registers: Vec<RegDesc>,
}

/// `@reg(access = ...)` erişim türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegAccess {
    ReadWrite,
    ReadOnly,
    WriteOnly,
}

impl RegAccess {
    /// Yazılım okuyabilir mi? (`WriteOnly` 0 okur — sürücü getter üretmez.)
    pub fn readable(self) -> bool {
        self != RegAccess::WriteOnly
    }

    /// Yazılım yazabilir mi? (`ReadOnly` yazması yok sayılır — setter yok.)
    pub fn writable(self) -> bool {
        self != RegAccess::ReadOnly
    }

    /// `regmap.json` kısa adı: `rw` | `ro` | `wo`.
    pub fn short(self) -> &'static str {
        match self {
            RegAccess::ReadWrite => "rw",
            RegAccess::ReadOnly => "ro",
            RegAccess::WriteOnly => "wo",
        }
    }
}

/// Tek register (32 bitlik sözcük).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegDesc {
    pub name: String,
    /// `@reg(offset = ...)` — taban adrese göre bayt offset'i.
    pub offset: u64,
    pub access: RegAccess,
    /// `volatile`: donanım günceller; yalnız `ReadOnly` ile geçerli.
    pub volatile: bool,
    pub doc: Option<String>,
    /// En düşük bitten başlayarak bildirim sırası; `@reserved` dahil.
    pub fields: Vec<FieldDesc>,
}

impl RegDesc {
    /// `base + offset` — sürücünün eriştiği mutlak adres.
    pub fn address(&self, base: u64) -> u64 {
        base + self.offset
    }

    /// Adlandırılmış (rezerve olmayan) alanlar.
    pub fn named(&self) -> impl Iterator<Item = &FieldDesc> {
        self.fields.iter().filter(|f| !f.reserved)
    }

    /// Adlandırılmış alan bitlerinin maskesi — rezerve bitler dışarıda.
    pub fn read_mask(&self) -> u32 {
        self.named().fold(0, |m, f| m | f.mask())
    }
}

/// Alan tipi ailesi (genişlik ayrı tutulur).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    /// `bool` (1 bit).
    Bool,
    /// `bits<N>`.
    Bits,
    /// `uN`.
    UInt,
}

impl FieldKind {
    /// `regmap.json` `type` değeri.
    pub fn json_name(self) -> &'static str {
        match self {
            FieldKind::Bool => "bool",
            FieldKind::Bits => "bits",
            FieldKind::UInt => "uint",
        }
    }
}

/// Register alanı; `@reserved` dolgusu da bir alandır (`reserved = true`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDesc {
    /// `@reserved` alanında `_reserved`.
    pub name: String,
    /// En düşük bit.
    pub lsb: u32,
    pub width: u32,
    pub kind: FieldKind,
    pub reserved: bool,
    /// `@self_clearing`: 1 yazılınca bir döngü 1 kalır (tetikleyici).
    pub self_clearing: bool,
    /// `@w1c`: donanım kurar, yazılım 1 yazarak temizler.
    pub w1c: bool,
    pub doc: Option<String>,
}

impl FieldDesc {
    /// En yüksek bit (dahil).
    pub fn msb(&self) -> u32 {
        self.lsb + self.width - 1
    }

    /// Alanın sözcük içindeki bit maskesi.
    pub fn mask(&self) -> u32 {
        let bits = if self.width >= 32 {
            u32::MAX
        } else {
            (1u32 << self.width) - 1
        };
        bits << self.lsb
    }
}
