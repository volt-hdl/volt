//! Yerleşik CDC primitif imzaları (ADR-0027).
//!
//! Çekirdek tablo volt-ast'ta yaşar (volt-sv-emit ile paylaşılır —
//! her iki crate de SV üretimi/denetim için aynı port imzalarına
//! bakar); HIR yüzeyi buradan yeniden dışa aktarılır.

pub use volt_ast::builtin::{BuiltinPort, BuiltinPrim, DomainRole, PortKind};
