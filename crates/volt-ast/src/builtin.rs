//! Yerleşik CDC primitifleri (ADR-0027): AsyncFifo, HandshakeSync,
//! PulseSync.
//!
//! Tablo BURADA (volt-ast) yaşar çünkü hem volt-hir (isim çözümleme,
//! tip/domain denetimi) hem volt-sv-emit (SV şablonu üretimi) aynı
//! port imzalarına ihtiyaç duyar; volt-ast ikisinin de ortak, bağımsız
//! bağımlılığıdır. Kullanıcı yüzeyi grammar §10 InstanceDecl biçimidir:
//! `let f = AsyncFifo<u8, 16> { wr_clk: clk, ... }`.

use crate::PortDir;

/// Derleyicide yerleşik CDC primitifi (ADR-0027 Seçenek B).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinPrim {
    /// Gray kod pointer'lı asenkron FIFO: `AsyncFifo<T, DEPTH>`.
    AsyncFifo,
    /// 4-fazlı req/ack ile tek transfer: `HandshakeSync<T>`.
    HandshakeSync,
    /// Toggle + kenar sezimi ile tek darbe geçişi: `PulseSync`.
    PulseSync,
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
}

/// Portun ait olduğu saat alanı rolü: yazma/kaynak veya okuma/hedef.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DomainRole {
    Src,
    Dst,
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

impl BuiltinPrim {
    /// İsimden primitif; bilinmeyen isimde None.
    pub fn from_name(name: &str) -> Option<BuiltinPrim> {
        match name {
            "AsyncFifo" => Some(BuiltinPrim::AsyncFifo),
            "HandshakeSync" => Some(BuiltinPrim::HandshakeSync),
            "PulseSync" => Some(BuiltinPrim::PulseSync),
            _ => None,
        }
    }

    /// Kullanıcı yüzü isim (tanı metinleri için).
    pub fn name(&self) -> &'static str {
        match self {
            BuiltinPrim::AsyncFifo => "AsyncFifo",
            BuiltinPrim::HandshakeSync => "HandshakeSync",
            BuiltinPrim::PulseSync => "PulseSync",
        }
    }

    /// Port imza tablosu.
    pub fn ports(&self) -> &'static [BuiltinPort] {
        match self {
            BuiltinPrim::AsyncFifo => ASYNC_FIFO_PORTS,
            BuiltinPrim::HandshakeSync => HANDSHAKE_SYNC_PORTS,
            BuiltinPrim::PulseSync => PULSE_SYNC_PORTS,
        }
    }

    /// Ada göre port imzası.
    pub fn port(&self, name: &str) -> Option<&'static BuiltinPort> {
        self.ports().iter().find(|p| p.name == name)
    }

    /// Beklenen tip argümanı sayısı (`T`).
    pub fn type_arg_count(&self) -> usize {
        match self {
            BuiltinPrim::AsyncFifo | BuiltinPrim::HandshakeSync => 1,
            BuiltinPrim::PulseSync => 0,
        }
    }

    /// Beklenen sabit argüman sayısı (`DEPTH`).
    pub fn const_arg_count(&self) -> usize {
        match self {
            BuiltinPrim::AsyncFifo => 1,
            BuiltinPrim::HandshakeSync | BuiltinPrim::PulseSync => 0,
        }
    }
}
