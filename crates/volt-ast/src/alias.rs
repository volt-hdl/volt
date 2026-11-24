//! Tip takma adı zinciri (ADR-0070) — tek yer.
//!
//! Generic olmayan `type W = u8` gibi takma adlar hedef tipine açılır.
//! SV üreticisi (`volt-sv-emit::alias`), enum ve struct düzeni
//! (`enum_layout`, `struct_layout`) ve test dilinin port genişliği
//! (`volt-hir::sim_port`, #376) bu fonksiyonu kullanır; önceden üç ayrı
//! kopyası vardı (#235). `signal_class` saat/reset sınıflamasının tip
//! denetimi olmadan koşan yoludur (#234).
//!
//! Arama ada göredir; özyineli zincir parser'da E4009 alır (ADR-0069),
//! derinlik sınırı yalnız savunmadır.

use crate::{Idx, ItemKind, ResetSpec, SourceFile, TypeRef, TypeRefKind};

/// Takma ad zincirinin en çok izlenen uzunluğu.
pub const MAX_ALIAS_DEPTH: usize = 64;

/// Bir sinyal tipinin saat/reset sınıfı (#234): `type Clk = clock` ile
/// yazılan port, `clock` yazılanla aynı sınıftadır (ADR-0070 §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalClass {
    /// `clock`.
    Clock,
    /// `reset` ya da `reset(sync|async, polarite)`; `None` türsüz `reset`.
    Reset(Option<ResetSpec>),
    /// Geri kalan her tip.
    Data,
}

/// Tip denetiminin sonucuna ulaşamayan geçitlerin sınıflaması: takma ad
/// zinciri ada göre açılır (`alias_chain_end`). Tip denetiminden sonra
/// koşan geçitler `volt_hir::port_class`'ı kullanır; bu yol ayrıştırıcının
/// geçitleri (pipeline, `@mmio`, otomatik kontrat, E0019 önerisi), tip
/// denetimi olmadan koşan denetimler (test blokları, ham reset portunun
/// W1001 süzgeci) ve LSP'nin yarım kod yolu içindir. Ayrıştırma sırasında
/// koşan geçit yalnız kendisinden önce bildirilen takma adı görür.
pub fn signal_class(ast: &SourceFile, ty: Idx<TypeRef>) -> SignalClass {
    let end = alias_chain_end(ast, ty).unwrap_or_else(|last| last);
    match ast.types[end].kind {
        TypeRefKind::Clock => SignalClass::Clock,
        TypeRefKind::Reset(spec) => SignalClass::Reset(spec),
        _ => SignalClass::Data,
    }
}

/// `ty` generic olmayan bir takma adı gösteriyorsa zincirin sonundaki
/// tip: `Ok(hedef)`. Zincir `MAX_ALIAS_DEPTH` adımda bitmediyse
/// `Err(son)`: o ana kadar varılan tip (döngü ya da aşırı uzun zincir).
/// Takma ad değilse `Ok(ty)`.
pub fn alias_chain_end(ast: &SourceFile, ty: Idx<TypeRef>) -> Result<Idx<TypeRef>, Idx<TypeRef>> {
    let mut cur = ty;
    for _ in 0..MAX_ALIAS_DEPTH {
        let TypeRefKind::Path { path, args } = &ast.types[cur].kind else {
            return Ok(cur);
        };
        let [seg] = path.segments.as_slice() else {
            return Ok(cur);
        };
        if !args.is_empty() {
            return Ok(cur);
        }
        let target = ast
            .items
            .iter()
            .find_map(|&i| match &ast.items_arena[i].kind {
                ItemKind::TypeAlias(a) if a.name.text == seg.text && a.generics.is_empty() => {
                    Some(a.target)
                }
                _ => None,
            });
        match target {
            Some(t) => cur = t,
            None => return Ok(cur),
        }
    }
    Err(cur)
}
