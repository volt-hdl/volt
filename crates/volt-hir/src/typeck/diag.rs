//! Tanı üretimi yardımcıları (type-inference.md §7, §8).
//!
//! Tip denetleyicinin birden çok modülünden çağrılan ortak tanılar
//! burada toplanır; yalnız tek bir kuralın ürettiği tanılar o kuralın
//! yanında kalır. `error`/`warning` beş parçalı tanı şablonunu (kod,
//! konum, açıklama, etiket, öneri) tek çağrıya indirir.

use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use super::TypeChecker;
use crate::ty::{Ty, TypeId};

impl TypeChecker<'_, '_> {
    /// Tanılardaki tip gösterimi — enum'lar adıyla (ADR-0074).
    pub(super) fn show(&self, ty: TypeId) -> String {
        self.types.display_enum_named(ty, &self.res.defs)
    }

    /// Tek birincil konumlu hata tanısı ekler.
    pub(super) fn error(
        &mut self,
        code: ErrorCode,
        span: Span,
        message: String,
        label: String,
        help: String,
    ) {
        self.diagnostics.push(Diagnostic::error(
            code,
            message,
            LabeledSpan::primary(span, label),
            help,
        ));
    }

    /// Tek birincil konumlu uyarı tanısı ekler.
    pub(super) fn warning(
        &mut self,
        code: ErrorCode,
        span: Span,
        message: String,
        label: String,
        help: String,
    ) {
        self.diagnostics.push(Diagnostic::warning(
            code,
            message,
            LabeledSpan::primary(span, label),
            help,
        ));
    }

    /// E2003 — beklenen/bulunan tip çifti.
    pub(super) fn err_type_mismatch(&mut self, expected: TypeId, actual: TypeId, span: Span) {
        let exp = self.show(expected);
        let act = self.show(actual);
        self.err_type_mismatch_msg(
            span,
            &lstr!(en: "type mismatch: expected '{exp}', found '{act}'"; tr: "tip uyumsuzluğu: '{exp}' bekleniyor, '{act}' bulundu"),
            &lstr!(en: "adapt the value to the target type; use an explicit cast with 'as' if needed"; tr: "değeri hedef tipe uyarlayın; gerekiyorsa 'as' ile açık dönüşüm yapın"),
        );
    }

    /// E2003 — özel açıklama ve öneriyle.
    pub(super) fn err_type_mismatch_msg(&mut self, span: Span, msg: &str, help: &str) {
        self.error(
            ErrorCode::E2003,
            span,
            msg.to_string(),
            lstr!(en: "mismatched types"; tr: "uyumsuz tip"),
            help.to_string(),
        );
    }

    /// E2003 — seçim (bit/aralık/parça) uygun olmayan bir tipe uygulandı.
    pub(super) fn err_not_selectable(&mut self, span: Span, msg: &str) {
        self.err_type_mismatch_msg(
            span,
            msg,
            &lstr!(en: "convert the value to a suitable type first"; tr: "önce değeri uygun bir tipe dönüştürün"),
        );
    }

    /// E2002 — ikili operatörde işaretli/işaretsiz karışımı.
    pub(super) fn err_sign_mismatch(&mut self, span: Span) {
        self.error(
            ErrorCode::E2002,
            span,
            lstr!(en: "cannot mix signed and unsigned"; tr: "işaretli ve işaretsiz karıştırılamaz"),
            lstr!(en: "signs differ"; tr: "işaretler farklı"),
            lstr!(en: "use an explicit cast with as"; tr: "as ile açık dönüşüm yapın"),
        );
    }

    /// E2001 — operand genişlikleri örtük birleştirilemez (§3.3, §8).
    pub(super) fn operand_width_mismatch(&mut self, a: u16, b: u16, prefix: &str, span: Span) {
        self.diagnostics.push(
            Diagnostic::error(
                ErrorCode::E2001,
                lstr!(en: "bit width mismatch: {prefix}{a} and {prefix}{b}"; tr: "bit genişliği uyumsuzluğu: {prefix}{a} ve {prefix}{b}"),
                LabeledSpan::primary(span, lstr!(en: "operand widths differ"; tr: "operand genişlikleri farklı")),
                lstr!(en: "widen the narrow operand: (expr) as {prefix}{}", a.max(b); tr: "dar operandı genişletin: (ifade) as {prefix}{}", a.max(b)),
            )
            .with_note(
                NoteKind::Reason,
                lstr!(en: "different widths cannot be combined implicitly; widening requires extra wires and logic in hardware"; tr: "farklı genişlikler örtük birleştirilemez; genişletme donanımda ek tel ve mantık gerektirir"),
            ),
        );
    }

    /// E2001 — örtük daraltma: `a` bitlik değer `b` bitlik hedefe
    /// sığmaz; kesme açık dönüşümle görünür kılınmalı (§5; ADR-0041:
    /// genişleme hedef tip yazılmışsa örtük, daraltma asla).
    pub(super) fn width_mismatch(&mut self, a: u16, b: u16, prefix: &str, span: Span) {
        self.diagnostics.push(
            Diagnostic::error(
                ErrorCode::E2001,
                lstr!(en: "a {a}-bit value does not fit in a {b}-bit target"; tr: "{a} bit değer {b} bit hedefe sığmaz"),
                LabeledSpan::primary(
                    span,
                    lstr!(en: "implicit narrowing is not allowed"; tr: "örtük daraltma yasak"),
                ),
                lstr!(en: "explicit cast: (expr) as {prefix}{b}"; tr: "açık dönüşüm: (ifade) as {prefix}{b}"),
            )
            .with_note(
                NoteKind::Reason,
                lstr!(en: "narrowing drops the upper bits; in hardware that truncation must be visible"; tr: "daraltma üst bitleri düşürür; donanımda bu kesme görünür olmalı"),
            ),
        );
    }

    /// E2006 — sabit indeks taban genişliğinin dışında.
    pub(super) fn index_out_of_bounds(&mut self, index: i128, width: u64, span: Span) {
        self.error(
            ErrorCode::E2006,
            span,
            lstr!(en: "index {index} out of bounds (width {width})"; tr: "indeks {index} sınır dışı (genişlik {width})"),
            lstr!(en: "invalid bit index"; tr: "geçersiz bit indeksi"),
            lstr!(en: "valid range: 0..{}", width.saturating_sub(1); tr: "geçerli aralık: 0..{}", width.saturating_sub(1)),
        );
    }

    /// E2010 — literal hedef tipin aralığına sığmıyor.
    pub(super) fn literal_overflow(&mut self, value: u128, ty: TypeId, span: Span) {
        let shown = self.show(ty);
        let max = match *self.types.ty(ty) {
            Ty::UInt { width } | Ty::UIntFlex { hi: width, .. } if width < 128 => {
                (1u128 << width) - 1
            }
            Ty::SInt { width } | Ty::SIntFlex { hi: width, .. } if width > 0 && width <= 128 => {
                (1u128 << (width - 1)) - 1
            }
            _ => u128::MAX,
        };
        self.error(
            ErrorCode::E2010,
            span,
            lstr!(en: "literal {value} does not fit in type {shown} (maximum {max})"; tr: "literal {value}, {shown} tipine sığmıyor (maksimum {max})"),
            lstr!(en: "value is outside the type's range"; tr: "değer tip aralığının dışında"),
            lstr!(en: "use a wider type or reduce the value"; tr: "daha geniş bir tip kullanın veya değeri küçültün"),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{codes, diagnostics};

    #[test]
    fn type_mismatch_reports_both_types_and_five_parts() {
        let diags = diagnostics("module M {\n    in  a : bool\n    out y : u8\n\n    y = a\n}\n");
        let d = diags
            .iter()
            .find(|d| d.code.as_str() == "E2003")
            .expect("E2003 beklenir");
        assert!(
            d.message.contains("u8") && d.message.contains("bool"),
            "{}",
            d.message
        );
        assert!(d.help.as_deref().is_some_and(|h| !h.is_empty()));
        assert!(d.primary_span().is_some_and(|s| !s.label.is_empty()));
    }

    #[test]
    fn narrowing_carries_reason_note() {
        let diags = diagnostics("module M {\n    in  a : u16\n    out y : u8\n\n    y = a\n}\n");
        let d = diags
            .iter()
            .find(|d| d.code.as_str() == "E2001")
            .expect("E2001 beklenir");
        assert!(!d.notes.is_empty(), "gerekçe notu beklenir");
    }

    #[test]
    fn literal_overflow_is_e2010() {
        let c = codes(
            "module M {\n    in  a : u8\n    out y : u8\n\n    let _x = 300u8\n\n    y = a\n}\n",
        );
        assert!(c.contains(&"E2010"), "{c:?}");
    }
}
