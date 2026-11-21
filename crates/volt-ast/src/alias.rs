//! Tip takma adı zinciri (ADR-0070) — tek yer.
//!
//! Generic olmayan `type W = u8` gibi takma adlar hedef tipine açılır.
//! SV üreticisi (`volt-sv-emit::alias`), enum ve struct düzeni
//! (`enum_layout`, `struct_layout`) ve test dilinin port genişliği
//! (`volt-hir::sim_port`, #376) bu fonksiyonu kullanır; önceden üç ayrı
//! kopyası vardı (#235).
//!
//! Arama ada göredir; özyineli zincir parser'da E4009 alır (ADR-0069),
//! derinlik sınırı yalnız savunmadır.

use crate::{Idx, ItemKind, SourceFile, TypeRef, TypeRefKind};

/// Takma ad zincirinin en çok izlenen uzunluğu.
pub const MAX_ALIAS_DEPTH: usize = 64;

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
