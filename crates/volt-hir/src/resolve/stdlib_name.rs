//! Standart kütüphane adı taşıyan kullanıcı modülleri (ADR-0102 §1, #92).
//!
//! Yerleşik primitifler (ADR-0027) emit'te ve birkaç denetimde ada göre
//! tanınır (`BuiltinPrim::from_name`); aynı adlı bir kullanıcı modülü
//! sessizce yerleşikle değişiyordu. Kural:
//! - `extern module <yerleşik ad>` bildirildiği yerde E1016 — extern'ün
//!   tek işi örneklenmektir;
//! - Volt modülü yalnız başka bir modülde ÖRNEKLENDİĞİ yerde E1016 — bu
//!   adla tek başına üst modül (`volt new` şablonundaki `Counter`)
//!   geçerli kalır.

use volt_ast::{ExternDecl, Name};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan};

use super::def::{DefId, DefKind};
use super::Resolver;
use crate::builtin::BuiltinPrim;

impl Resolver<'_> {
    /// `extern module` bildirimi: yerleşik adda E1016 (W1003 yerine), ad
    /// yine bildirilir ki örneklemeler E1001 zinciri üretmesin.
    pub(super) fn declare_extern(&mut self, x: &ExternDecl, is_public: bool) -> DefId {
        let name = x.name.clone();
        if BuiltinPrim::from_name(&name.text).is_none() {
            return self.declare_checked(&name, DefKind::ExternModule, self.root, is_public);
        }
        if self.report_duplicate(&name, self.root) {
            return self.error_def;
        }
        let shown = self.ast.generate.source_name(name.span, &name.text);
        self.diagnostics.push(Diagnostic::error(
            ErrorCode::E1016,
            lstr!(en: "extern module '{}' has the name of a standard library module", shown;
                  tr: "extern module '{}' bir standart kütüphane modülünün adını taşıyor", shown),
            LabeledSpan::primary(
                name.span,
                lstr!(en: "the built-in '{}' would be used in its place", shown;
                      tr: "yerine yerleşik '{}' kullanılırdı", shown),
            ),
            lstr!(en: "declare the SystemVerilog module under another name; if it must stay '{}', place it in a SystemVerilog wrapper module with another name and declare the wrapper", shown;
                  tr: "SystemVerilog modülünü başka bir adla bildirin; adı '{}' kalmalıysa onu başka adlı bir SystemVerilog sarmalayıcı modüle yerleştirip sarmalayıcıyı bildirin", shown),
        ));
        self.declare(&name, DefKind::ExternModule, self.root, is_public)
    }

    /// Örnekleme: hedef yerleşik adlı bir Volt modülüyse E1016. Extern
    /// hedef bildiriminde zaten raporlandı.
    pub(super) fn check_instance_stdlib_name(&mut self, target: DefId, path_name: &Name) {
        if !matches!(self.def(target).kind, DefKind::Module)
            || BuiltinPrim::from_name(&path_name.text).is_none()
        {
            return;
        }
        let module_span = self.def(target).span;
        let shown = self
            .ast
            .generate
            .source_name(path_name.span, &path_name.text);
        self.diagnostics.push(
            Diagnostic::error(
                ErrorCode::E1016,
                lstr!(en: "module '{}' has the name of a standard library module", shown;
                      tr: "'{}' modülü bir standart kütüphane modülünün adını taşıyor", shown),
                LabeledSpan::primary(
                    path_name.span,
                    lstr!(en: "the built-in '{}' would be placed here", shown;
                          tr: "buraya yerleşik '{}' yerleştirilirdi", shown),
                ),
                lstr!(en: "rename the module and this instantiation, e.g. 'My{}'", shown;
                      tr: "modülü ve bu örneklemeyi yeniden adlandırın, ör. 'My{}'", shown),
            )
            .with_secondary(
                module_span,
                lstr!(en: "module declared here"; tr: "modül burada bildirildi"),
            ),
        );
    }
}

/// Örnekleme yolunun tek segmenti (yerleşikler tek segmentlidir).
pub(super) fn single_segment(path: &volt_ast::Path) -> Option<&Name> {
    match path.segments.as_slice() {
        [seg] => Some(seg),
        _ => None,
    }
}
