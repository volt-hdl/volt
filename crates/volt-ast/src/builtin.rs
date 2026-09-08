//! Yerleşik stdlib primitifleri (ADR-0027, ADR-0029): CDC primitifleri
//! (AsyncFifo, HandshakeSync, PulseSync) ve tek saatli günlük yapı
//! taşları (SyncFifo, Ram, DualPortRam, Counter, ShiftRegister,
//! RoundRobinArbiter, PriorityArbiter, EdgeDetect).
//!
//! Tablo BURADA (volt-ast) yaşar çünkü hem volt-hir (isim çözümleme,
//! tip/domain denetimi) hem volt-sv-emit (SV şablonu üretimi) aynı
//! port imzalarına ihtiyaç duyar; volt-ast ikisinin de ortak, bağımsız
//! bağımlılığıdır. Kullanıcı yüzeyi grammar §10 InstanceDecl biçimidir:
//! `let f = AsyncFifo<u8, 16> { wr_clk: clk, ... }`.

use crate::PortDir;

/// Derleyicide yerleşik primitif (ADR-0027 Seçenek B).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinPrim {
    /// Gray kod pointer'lı asenkron FIFO: `AsyncFifo<T, DEPTH>`.
    AsyncFifo,
    /// 4-fazlı req/ack ile tek transfer: `HandshakeSync<T>`.
    HandshakeSync,
    /// Toggle + kenar sezimi ile tek darbe geçişi: `PulseSync`.
    PulseSync,
    /// Tek saat alanında FIFO: `SyncFifo<T, DEPTH>` (ADR-0029).
    SyncFifo,
    /// Tek portlu senkron RAM: `Ram<T, DEPTH>` (ADR-0029).
    Ram,
    /// Aynı saatte iki bağımsız portlu RAM: `DualPortRam<T, DEPTH>`.
    DualPortRam,
    /// Enable/clear'lı sayaç: `Counter<WIDTH>` (ADR-0029).
    Counter,
    /// Seri-paralel kaydırma: `ShiftRegister<T, LEN>` (ADR-0029).
    ShiftRegister,
    /// Dönen öncelikli arbiter: `RoundRobinArbiter<N>` (ADR-0029).
    RoundRobinArbiter,
    /// Sabit öncelikli arbiter (req[0] en yüksek): `PriorityArbiter<N>`.
    PriorityArbiter,
    /// Tek saatli kenar algılama: `EdgeDetect` (ADR-0029).
    EdgeDetect,
}

/// Primitif portunun taşıdığı değerin türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PortKind {
    /// Saat girişi.
    Clock,
    /// Generic `T` tipindeki veri.
    Data,
    /// Tek bitlik kontrol/durum.
    Bool,
    /// Adres: `uK` (K = clog2(DEPTH)) — DEPTH iki kuvveti olduğundan
    /// her adres değeri geçerli aralıktadır.
    Addr,
    /// Sabit generic argüman genişliğinde ham vektör: `bits<DIM>`
    /// (Counter.count, arbiter req/grant).
    Dim,
    /// Tüm aşamaların paralel görünümü: `bits<LEN * width(T)>`
    /// (ShiftRegister.taps).
    Taps,
}

/// Portun ait olduğu saat alanı rolü: yazma/kaynak veya okuma/hedef.
/// Tek saatli primitiflerde tüm portlar `Src`'dir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DomainRole {
    Src,
    Dst,
}

/// Sabit generic argümanın (DEPTH/WIDTH/LEN/N) geçerlilik kuralı.
/// Hem volt-hir (E2025) hem volt-sv-emit aynı kuralı uygular.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstRule {
    /// İki kuvveti olmalı, [min, max] aralığında (FIFO/RAM derinlikleri).
    PowerOfTwo { min: u128, max: u128 },
    /// [min, max] aralığında herhangi bir değer (genişlik/uzunluk/port sayısı).
    Range { min: u128, max: u128 },
}

impl ConstRule {
    /// Değer kurala uyuyor mu?
    pub fn allows(&self, value: u128) -> bool {
        match *self {
            ConstRule::PowerOfTwo { min, max } => {
                (min..=max).contains(&value) && value.is_power_of_two()
            }
            ConstRule::Range { min, max } => (min..=max).contains(&value),
        }
    }
}

/// Bir primitif portunun imzası.
#[derive(Debug, Clone, Copy)]
pub struct BuiltinPort {
    pub name: &'static str,
    pub dir: PortDir,
    pub kind: PortKind,
    pub role: DomainRole,
}

const fn port(name: &'static str, dir: PortDir, kind: PortKind, role: DomainRole) -> BuiltinPort {
    BuiltinPort {
        name,
        dir,
        kind,
        role,
    }
}

/// Tek saatli primitif portu kısayolu (rol her zaman Src).
const fn sport(name: &'static str, dir: PortDir, kind: PortKind) -> BuiltinPort {
    port(name, dir, kind, DomainRole::Src)
}

/// AsyncFifo<T, DEPTH> port tablosu (yazma tarafı Src, okuma Dst).
const ASYNC_FIFO_PORTS: &[BuiltinPort] = &[
    port("wr_clk", PortDir::In, PortKind::Clock, DomainRole::Src),
    port("wr_data", PortDir::In, PortKind::Data, DomainRole::Src),
    port("wr_en", PortDir::In, PortKind::Bool, DomainRole::Src),
    port("wr_full", PortDir::Out, PortKind::Bool, DomainRole::Src),
    port("rd_clk", PortDir::In, PortKind::Clock, DomainRole::Dst),
    port("rd_data", PortDir::Out, PortKind::Data, DomainRole::Dst),
    port("rd_en", PortDir::In, PortKind::Bool, DomainRole::Dst),
    port("rd_empty", PortDir::Out, PortKind::Bool, DomainRole::Dst),
];

/// HandshakeSync<T> port tablosu.
const HANDSHAKE_SYNC_PORTS: &[BuiltinPort] = &[
    port("src_clk", PortDir::In, PortKind::Clock, DomainRole::Src),
    port("data_in", PortDir::In, PortKind::Data, DomainRole::Src),
    port("send", PortDir::In, PortKind::Bool, DomainRole::Src),
    port("busy", PortDir::Out, PortKind::Bool, DomainRole::Src),
    port("dst_clk", PortDir::In, PortKind::Clock, DomainRole::Dst),
    port("data_out", PortDir::Out, PortKind::Data, DomainRole::Dst),
    port("valid", PortDir::Out, PortKind::Bool, DomainRole::Dst),
];

/// PulseSync port tablosu (generic argümansız).
const PULSE_SYNC_PORTS: &[BuiltinPort] = &[
    port("src_clk", PortDir::In, PortKind::Clock, DomainRole::Src),
    port("pulse_in", PortDir::In, PortKind::Bool, DomainRole::Src),
    port("dst_clk", PortDir::In, PortKind::Clock, DomainRole::Dst),
    port("pulse_out", PortDir::Out, PortKind::Bool, DomainRole::Dst),
];

/// SyncFifo<T, DEPTH> port tablosu — tek saat, tek alan.
const SYNC_FIFO_PORTS: &[BuiltinPort] = &[
    sport("clk", PortDir::In, PortKind::Clock),
    sport("wr_data", PortDir::In, PortKind::Data),
    sport("wr_en", PortDir::In, PortKind::Bool),
    sport("full", PortDir::Out, PortKind::Bool),
    sport("rd_data", PortDir::Out, PortKind::Data),
    sport("rd_en", PortDir::In, PortKind::Bool),
    sport("empty", PortDir::Out, PortKind::Bool),
];

/// Ram<T, DEPTH> port tablosu — tek portlu senkron RAM.
const RAM_PORTS: &[BuiltinPort] = &[
    sport("clk", PortDir::In, PortKind::Clock),
    sport("addr", PortDir::In, PortKind::Addr),
    sport("wr_data", PortDir::In, PortKind::Data),
    sport("wr_en", PortDir::In, PortKind::Bool),
    sport("rd_data", PortDir::Out, PortKind::Data),
];

/// DualPortRam<T, DEPTH> port tablosu — aynı saatte iki bağımsız port.
const DUAL_PORT_RAM_PORTS: &[BuiltinPort] = &[
    sport("clk", PortDir::In, PortKind::Clock),
    sport("a_addr", PortDir::In, PortKind::Addr),
    sport("a_wr_data", PortDir::In, PortKind::Data),
    sport("a_wr_en", PortDir::In, PortKind::Bool),
    sport("a_rd_data", PortDir::Out, PortKind::Data),
    sport("b_addr", PortDir::In, PortKind::Addr),
    sport("b_wr_data", PortDir::In, PortKind::Data),
    sport("b_wr_en", PortDir::In, PortKind::Bool),
    sport("b_rd_data", PortDir::Out, PortKind::Data),
];

/// Counter<WIDTH> port tablosu.
const COUNTER_PORTS: &[BuiltinPort] = &[
    sport("clk", PortDir::In, PortKind::Clock),
    sport("enable", PortDir::In, PortKind::Bool),
    sport("clear", PortDir::In, PortKind::Bool),
    sport("count", PortDir::Out, PortKind::Dim),
    sport("overflow", PortDir::Out, PortKind::Bool),
];

/// ShiftRegister<T, LEN> port tablosu.
const SHIFT_REGISTER_PORTS: &[BuiltinPort] = &[
    sport("clk", PortDir::In, PortKind::Clock),
    sport("data_in", PortDir::In, PortKind::Data),
    sport("shift_en", PortDir::In, PortKind::Bool),
    sport("data_out", PortDir::Out, PortKind::Data),
    sport("taps", PortDir::Out, PortKind::Taps),
];

/// RoundRobinArbiter<N> / PriorityArbiter<N> port tablosu.
const ARBITER_PORTS: &[BuiltinPort] = &[
    sport("clk", PortDir::In, PortKind::Clock),
    sport("req", PortDir::In, PortKind::Dim),
    sport("grant", PortDir::Out, PortKind::Dim),
];

/// EdgeDetect port tablosu (generic argümansız).
const EDGE_DETECT_PORTS: &[BuiltinPort] = &[
    sport("clk", PortDir::In, PortKind::Clock),
    sport("signal", PortDir::In, PortKind::Bool),
    sport("rising", PortDir::Out, PortKind::Bool),
    sport("falling", PortDir::Out, PortKind::Bool),
    sport("both", PortDir::Out, PortKind::Bool),
];

impl BuiltinPrim {
    /// İsimden primitif; bilinmeyen isimde None.
    pub fn from_name(name: &str) -> Option<BuiltinPrim> {
        match name {
            "AsyncFifo" => Some(BuiltinPrim::AsyncFifo),
            "HandshakeSync" => Some(BuiltinPrim::HandshakeSync),
            "PulseSync" => Some(BuiltinPrim::PulseSync),
            "SyncFifo" => Some(BuiltinPrim::SyncFifo),
            "Ram" => Some(BuiltinPrim::Ram),
            "DualPortRam" => Some(BuiltinPrim::DualPortRam),
            "Counter" => Some(BuiltinPrim::Counter),
            "ShiftRegister" => Some(BuiltinPrim::ShiftRegister),
            "RoundRobinArbiter" => Some(BuiltinPrim::RoundRobinArbiter),
            "PriorityArbiter" => Some(BuiltinPrim::PriorityArbiter),
            "EdgeDetect" => Some(BuiltinPrim::EdgeDetect),
            _ => None,
        }
    }

    /// Kullanıcı yüzü isim (tanı metinleri için).
    pub fn name(&self) -> &'static str {
        match self {
            BuiltinPrim::AsyncFifo => "AsyncFifo",
            BuiltinPrim::HandshakeSync => "HandshakeSync",
            BuiltinPrim::PulseSync => "PulseSync",
            BuiltinPrim::SyncFifo => "SyncFifo",
            BuiltinPrim::Ram => "Ram",
            BuiltinPrim::DualPortRam => "DualPortRam",
            BuiltinPrim::Counter => "Counter",
            BuiltinPrim::ShiftRegister => "ShiftRegister",
            BuiltinPrim::RoundRobinArbiter => "RoundRobinArbiter",
            BuiltinPrim::PriorityArbiter => "PriorityArbiter",
            BuiltinPrim::EdgeDetect => "EdgeDetect",
        }
    }

    /// Port imza tablosu.
    pub fn ports(&self) -> &'static [BuiltinPort] {
        match self {
            BuiltinPrim::AsyncFifo => ASYNC_FIFO_PORTS,
            BuiltinPrim::HandshakeSync => HANDSHAKE_SYNC_PORTS,
            BuiltinPrim::PulseSync => PULSE_SYNC_PORTS,
            BuiltinPrim::SyncFifo => SYNC_FIFO_PORTS,
            BuiltinPrim::Ram => RAM_PORTS,
            BuiltinPrim::DualPortRam => DUAL_PORT_RAM_PORTS,
            BuiltinPrim::Counter => COUNTER_PORTS,
            BuiltinPrim::ShiftRegister => SHIFT_REGISTER_PORTS,
            BuiltinPrim::RoundRobinArbiter | BuiltinPrim::PriorityArbiter => ARBITER_PORTS,
            BuiltinPrim::EdgeDetect => EDGE_DETECT_PORTS,
        }
    }

    /// Ada göre port imzası.
    pub fn port(&self, name: &str) -> Option<&'static BuiltinPort> {
        self.ports().iter().find(|p| p.name == name)
    }

    /// Beklenen tip argümanı sayısı (`T`).
    pub fn type_arg_count(&self) -> usize {
        match self {
            BuiltinPrim::AsyncFifo
            | BuiltinPrim::HandshakeSync
            | BuiltinPrim::SyncFifo
            | BuiltinPrim::Ram
            | BuiltinPrim::DualPortRam
            | BuiltinPrim::ShiftRegister => 1,
            BuiltinPrim::PulseSync
            | BuiltinPrim::Counter
            | BuiltinPrim::RoundRobinArbiter
            | BuiltinPrim::PriorityArbiter
            | BuiltinPrim::EdgeDetect => 0,
        }
    }

    /// Beklenen sabit argüman sayısı (`DEPTH`/`WIDTH`/`LEN`/`N`).
    pub fn const_arg_count(&self) -> usize {
        match self {
            BuiltinPrim::AsyncFifo
            | BuiltinPrim::SyncFifo
            | BuiltinPrim::Ram
            | BuiltinPrim::DualPortRam
            | BuiltinPrim::Counter
            | BuiltinPrim::ShiftRegister
            | BuiltinPrim::RoundRobinArbiter
            | BuiltinPrim::PriorityArbiter => 1,
            BuiltinPrim::HandshakeSync | BuiltinPrim::PulseSync | BuiltinPrim::EdgeDetect => 0,
        }
    }

    /// Sabit generic parametrenin kullanıcı yüzü adı (tanılar için).
    pub fn const_param_name(&self) -> &'static str {
        match self {
            BuiltinPrim::Counter => "WIDTH",
            BuiltinPrim::ShiftRegister => "LEN",
            BuiltinPrim::RoundRobinArbiter | BuiltinPrim::PriorityArbiter => "N",
            _ => "DEPTH",
        }
    }

    /// Sabit generic argümanın geçerlilik kuralı; sabit argümansız
    /// primitiflerde None.
    pub fn const_rule(&self) -> Option<ConstRule> {
        match self {
            BuiltinPrim::AsyncFifo
            | BuiltinPrim::SyncFifo
            | BuiltinPrim::Ram
            | BuiltinPrim::DualPortRam => Some(ConstRule::PowerOfTwo {
                min: 2,
                max: 65_536,
            }),
            BuiltinPrim::Counter => Some(ConstRule::Range { min: 1, max: 64 }),
            BuiltinPrim::ShiftRegister
            | BuiltinPrim::RoundRobinArbiter
            | BuiltinPrim::PriorityArbiter => Some(ConstRule::Range { min: 2, max: 64 }),
            BuiltinPrim::HandshakeSync | BuiltinPrim::PulseSync | BuiltinPrim::EdgeDetect => None,
        }
    }

    /// Sabit argüman kural ihlalinde fix-it önerisi metni (EN/TR aynı
    /// biçim — sayı listesi dil bağımsız).
    pub fn const_rule_hint_en(&self) -> &'static str {
        match self.const_rule() {
            Some(ConstRule::PowerOfTwo { .. }) => "use a power of two: 2, 4, 8, 16, ...",
            Some(ConstRule::Range { min: 1, .. }) => "use a value between 1 and 64",
            Some(ConstRule::Range { .. }) => "use a value between 2 and 64",
            None => "",
        }
    }

    /// `const_rule_hint_en`'in Türkçe eşleniği.
    pub fn const_rule_hint_tr(&self) -> &'static str {
        match self.const_rule() {
            Some(ConstRule::PowerOfTwo { .. }) => "iki kuvveti kullanın: 2, 4, 8, 16, ...",
            Some(ConstRule::Range { min: 1, .. }) => "1 ile 64 arasında bir değer kullanın",
            Some(ConstRule::Range { .. }) => "2 ile 64 arasında bir değer kullanın",
            None => "",
        }
    }

    /// Generic imza kalıbı (E2003 yardım metni için).
    pub fn generic_shape(&self) -> &'static str {
        match self {
            BuiltinPrim::AsyncFifo => "AsyncFifo<T, DEPTH>",
            BuiltinPrim::HandshakeSync => "HandshakeSync<T>",
            BuiltinPrim::PulseSync => "PulseSync",
            BuiltinPrim::SyncFifo => "SyncFifo<T, DEPTH>",
            BuiltinPrim::Ram => "Ram<T, DEPTH>",
            BuiltinPrim::DualPortRam => "DualPortRam<T, DEPTH>",
            BuiltinPrim::Counter => "Counter<WIDTH>",
            BuiltinPrim::ShiftRegister => "ShiftRegister<T, LEN>",
            BuiltinPrim::RoundRobinArbiter => "RoundRobinArbiter<N>",
            BuiltinPrim::PriorityArbiter => "PriorityArbiter<N>",
            BuiltinPrim::EdgeDetect => "EdgeDetect",
        }
    }

    /// Ayrı bir hedef (okuma) saat alanı var mı? CDC primitiflerinde
    /// evet; tek saatli yapı taşlarında hayır (tüm portlar Src).
    pub fn has_dst_clock(&self) -> bool {
        matches!(
            self,
            BuiltinPrim::AsyncFifo | BuiltinPrim::HandshakeSync | BuiltinPrim::PulseSync
        )
    }
}
