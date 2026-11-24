//! Port saat mı, reset mi, veri mi — tek kural (#234, ADR-0136).
//!
//! Sınıf, tip denetiminin tanıma verdiği tipten okunur (`Ty::Clock`,
//! `Ty::Reset`): `type Clk = clock` ile yazılan port tip denetimi için
//! zaten saattir (ADR-0070 §6), sözdizimi yeniden çözülmez. Saat alanı
//! çıkarımı, modül özeti, güven denetimi, reset alanı denetimi, zamanlama
//! kısıtları, L1 zamanlama ve tip denetiminin reset bağlama kuralı bu
//! modülü çağırır. Tip denetimine ulaşamayan geçitler (ayrıştırıcı, test
//! blokları, LSP'nin yarım kod yolu) aynı sınıfı
//! `volt_ast::alias::signal_class` ile takma ad zincirinden okur.

use std::collections::HashMap;

use volt_ast::{Port, PortDir};

use crate::resolve::{DefId, ResolveResult};
use crate::ty::{Ty, TypeArena, TypeId};
use crate::typeck::TypeckResult;

pub use volt_ast::alias::SignalClass;

/// Tip denetiminin tanım tipleri: bitmiş denetimin sonucu
/// ([`TypeckResult`]) ya da sürmekte olan denetimin tabloları.
#[derive(Clone, Copy)]
pub struct DefTypes<'a> {
    types: &'a TypeArena,
    def_types: &'a HashMap<DefId, TypeId>,
}

impl<'a> DefTypes<'a> {
    pub fn of(tyck: &'a TypeckResult) -> Self {
        Self::new(&tyck.types, &tyck.def_types)
    }

    pub(crate) fn new(types: &'a TypeArena, def_types: &'a HashMap<DefId, TypeId>) -> Self {
        DefTypes { types, def_types }
    }

    /// Tanımın sınıfı; tipi kaydedilmemiş tanım veridir.
    pub fn class(self, def: DefId) -> SignalClass {
        let Some(ty) = self.def_types.get(&def).map(|&t| self.types.ty(t)) else {
            return SignalClass::Data;
        };
        if let Ty::Reset { spec } = ty {
            return SignalClass::Reset(spec.map(Into::into));
        }
        if matches!(ty, Ty::Clock) {
            SignalClass::Clock
        } else {
            SignalClass::Data
        }
    }

    pub fn is_clock(self, def: DefId) -> bool {
        self.class(def) == SignalClass::Clock
    }

    /// Portun sınıfı (modül, extern ya da örnekleme hedefi); bildirimi
    /// çözülmemiş port veridir.
    pub fn port(self, res: &ResolveResult, port: &Port) -> SignalClass {
        res.decl_spans
            .get(&port.name.span)
            .map_or(SignalClass::Data, |&def| self.class(def))
    }

    pub fn is_clock_port(self, res: &ResolveResult, port: &Port) -> bool {
        self.port(res, port) == SignalClass::Clock
    }

    /// Ham reset portu (ADR-0065 §1): reset tipli giriş portu. Çıkış ve
    /// çift yönlü reset portu sv-emit'te E0003 kalır.
    pub fn is_raw_reset(self, res: &ResolveResult, port: &Port) -> bool {
        port.direction == PortDir::In && matches!(self.port(res, port), SignalClass::Reset(_))
    }
}
