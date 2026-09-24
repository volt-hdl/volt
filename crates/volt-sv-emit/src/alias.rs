//! Kullanıcı tipleri ve SV eşlemesi (ADR-0070).
//!
//! * Takma ad çözümü: generic olmayan `type W = u8` gibi takma adlar SV
//!   eşlemesinden ÖNCE hedef tipine açılır — eşleme noktaları
//!   (`sig_of_typeref`, saat/dizi/reset/Trit tespiti) çözülmüş tipe
//!   bakar. Böylece desteklenen bir tipe çözülen takma ad, o tipin
//!   kendisi gibi çalışır (hata düzeltmesi: önce hep E0003'tü).
//! * Desteklenmeyen kullanıcı tipinin ve bulunamayan örnek hedefinin
//!   ADINI ve TÜRÜNÜ söyleyen E0003 metni (struct, enum, generic takma
//!   ad, extern modül, struct literali).
//!
//! Arama ada göredir (emitter modül örnek hedeflerini de böyle bulur);
//! özyineli takma ad zinciri parser'da E4009 alır (ADR-0069), derinlik
//! sınırı yalnız savunmadır.

use volt_ast::{Idx, ItemKind, SourceFile, TypeRef, TypeRefKind};
use volt_diagnostics::lstr;

/// Takma ad zincirinin en çok izlenen uzunluğu (savunma; döngü E4009).
const MAX_ALIAS_DEPTH: usize = 64;

/// `ty` generic olmayan bir takma adı gösteriyorsa (zincir boyunca)
/// hedef tipi; değilse `ty`'nin kendisi.
pub(crate) fn resolve(ast: &SourceFile, ty: Idx<TypeRef>) -> Idx<TypeRef> {
    let mut cur = ty;
    for _ in 0..MAX_ALIAS_DEPTH {
        let TypeRefKind::Path { path, args } = &ast.types[cur].kind else {
            return cur;
        };
        let [seg] = path.segments.as_slice() else {
            return cur;
        };
        if !args.is_empty() {
            return cur;
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
            None => return cur,
        }
    }
    cur
}

fn item_named<'a>(ast: &'a SourceFile, name: &str) -> Option<&'a ItemKind> {
    ast.items
        .iter()
        .map(|&i| &ast.items_arena[i].kind)
        .find(|k| match k {
            ItemKind::Struct(s) => s.name.text == name,
            ItemKind::Enum(e) => e.name.text == name,
            ItemKind::TypeAlias(a) => a.name.text == name,
            ItemKind::Module(m) => m.name.text == name,
            ItemKind::Extern(e) => e.name.text == name,
            _ => false,
        })
}

/// Çözülmüş (takma ad olmayan) bir `Path` tipi için E0003 metni: ne
/// desteklenmiyor.
pub(crate) fn describe_user_type(ast: &SourceFile, ty: Idx<TypeRef>) -> String {
    let TypeRefKind::Path { path, .. } = &ast.types[ty].kind else {
        return lstr!(en: "this type as a signal type"; tr: "sinyal tipi olarak bu tip");
    };
    let name = path.segments.last().map_or("", |s| s.text.as_str());
    match item_named(ast, name) {
        // ADR-0077 Karar 1: kalıcı kural — bundle bir değer değildir.
        Some(ItemKind::Struct(s)) if s.is_port => lstr!(
            en: "'struct port' bundle '{name}' outside a module port: a 'struct port' groups directed port fields and is not a value; use a plain 'struct' for data";
            tr: "modül portu dışında 'struct port' bundle '{name}': 'struct port' yönlü port alanlarını gruplar, bir değer değildir; veri için düz 'struct' kullanın"
        ),
        Some(ItemKind::Struct(s)) if !s.generics.is_empty() => lstr!(
            en: "generic struct type '{name}' as a signal type (ADR-0069)";
            tr: "sinyal tipi olarak generic struct tipi '{name}' (ADR-0069)"
        ),
        Some(ItemKind::Struct(s)) => describe_struct_layout(ast, s),
        Some(ItemKind::Enum(_)) => lstr!(
            en: "enum type '{name}' as a signal type (ports, reg, wire, let)";
            tr: "sinyal tipi olarak enum tipi '{name}' (port, reg, wire, let)"
        ),
        Some(ItemKind::TypeAlias(_)) => lstr!(
            en: "generic type alias '{name}' as a signal type";
            tr: "sinyal tipi olarak generic takma ad '{name}'"
        ),
        _ if name == "Handshake" => lstr!(
            en: "'Handshake' outside a module port";
            tr: "modül portu dışında 'Handshake'"
        ),
        _ => lstr!(
            en: "user-defined type '{name}' as a signal type";
            tr: "sinyal tipi olarak kullanıcı tipi '{name}'"
        ),
    }
}

/// Düzeni kurulamayan düz struct (ADR-0077): hangi alan neden.
fn describe_struct_layout(ast: &SourceFile, s: &volt_ast::StructDecl) -> String {
    use volt_ast::struct_layout::{layout, LayoutError};
    let name = &s.name.text;
    match layout(ast, s, &mut |e| crate::structs::const_int(ast, e)) {
        Err(LayoutError::ArrayOfStruct(f)) => lstr!(
            en: "arrays of structs (field '{f}' of struct '{name}')";
            tr: "struct dizileri ('{name}' struct'ının '{f}' alanı)"
        ),
        Err(LayoutError::Unsupported(f)) => lstr!(
            en: "struct '{name}' as a signal type: field '{f}' has a type with no hardware mapping (enum arrays, payload enums, tuples, clock/reset)";
            tr: "sinyal tipi olarak '{name}' struct'ı: '{f}' alanının tipinin donanım eşlemesi yok (enum dizisi, payload'lı enum, tuple, clock/reset)"
        ),
        _ => lstr!(
            en: "struct type '{name}' as a signal type (ports, reg, wire, let)";
            tr: "sinyal tipi olarak struct tipi '{name}' (port, reg, wire, let)"
        ),
    }
}

/// Emitter'ın hedefini bulamadığı örnekleme için E0003 metni.
pub(crate) fn describe_missing_instance_target(
    ast: &SourceFile,
    inst: &str,
    target: &str,
) -> String {
    match item_named(ast, target) {
        Some(ItemKind::Extern(_)) => lstr!(
            en: "instances of generic extern module '{target}' (instance '{inst}'; extern generics are not monomorphized)";
            tr: "generic extern modül '{target}' örnekleri ('{inst}' örneği; extern generic'leri monomorfize edilmez)"
        ),
        Some(ItemKind::Struct(_)) => lstr!(
            en: "struct literals ('{inst} = {target} {{ ... }}')";
            tr: "struct literalleri ('{inst} = {target} {{ ... }}')"
        ),
        _ => lstr!(
            en: "instance '{inst}' of '{target}' (the target is not a module of this design)";
            tr: "'{target}' hedefli '{inst}' örneği (hedef bu tasarımın bir modülü değil)"
        ),
    }
}
