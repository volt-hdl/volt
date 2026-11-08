//! Hiç örneklenmemiş generic modül (ADR-0111, #143).
//!
//! Monomorfizasyon (ADR-0041) örneklenen her generic modülü somut bir
//! modüle çevirir ve şablonu öğe listesinden çıkarır. Öğe listesinde
//! generic parametreli kalan modül hiç örneklenmemiş şablondur:
//! parametrelerinin değeri yoktur, SV'si yazılmaz. Ana dosyadaki şablon
//! W1006 alır; `use` ile yüklenen kütüphane dosyasının modülleri zaten
//! çıktı kümesine girmez (ADR-0042 eki, sürücünün `retain_reachable`'ı),
//! onun şablonu uyarı almaz.

use volt_ast::ModuleDecl;
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan};
use volt_span::FileId;

/// #171: parametreli SV önerisi (v0.2.0); W1006'nın metni onu gösterir.
pub(crate) const PARAMETERIZED_SV_ISSUE: u32 = 171;

/// `module` bir şablonsa `Some` (üretici onu atlar): içinde W1006, şablon
/// ana dosyadaysa. `main` kaynak listesinin ilk dosyası; liste boşsa
/// (kitaplık API'si) her dosya ana dosya sayılır. Şablon değilse `None`.
pub(crate) fn skip(
    module: &ModuleDecl,
    file: FileId,
    main: Option<FileId>,
) -> Option<Option<Diagnostic>> {
    if module.generics.is_empty() {
        return None;
    }
    Some(
        main.is_none_or(|m| m == file)
            .then(|| uninstantiated_generic(module)),
    )
}

/// W1006: şablonun adında; metin parametreli çıktı önerisini gösterir.
fn uninstantiated_generic(module: &ModuleDecl) -> Diagnostic {
    let name = &module.name.text;
    let issue = PARAMETERIZED_SV_ISSUE;
    Diagnostic::warning(
        ErrorCode::W1006,
        lstr!(en: "module '{name}' is generic and is never instantiated; no SystemVerilog is generated for it (parameterized output is planned: #{issue})";
              tr: "'{name}' modülü generic ve hiç örneklenmiyor; onun için SystemVerilog üretilmez (parametreli çıktı planlanıyor: #{issue})"),
        LabeledSpan::primary(
            module.name.span,
            lstr!(en: "a template: its parameters have no values in this file";
                  tr: "bir şablon: parametrelerinin bu dosyada değeri yok"),
        ),
        lstr!(en: "instantiate it with concrete arguments where it is used (for example 'let u = {name}<8> {{ ... }}' generates '{name}_8'), or remove it";
              tr: "kullanıldığı yerde somut argümanlarla örnekleyin ('let u = {name}<8> {{ ... }}' '{name}_8'i üretir) ya da silin"),
    )
}
