//! Bit, aralık, parça ve alan seçimi (type-inference.md §3.5;
//! ADR-0035 indexed part-select). Hem ifade (`synth`) hem atama hedefi
//! (`lvalue_type`) aynı sonuç kurallarını kullanır.

use volt_ast::{Expr, Idx, ItemKind, Name};
use volt_diagnostics::{lstr, ErrorCode};
use volt_span::Span;

use super::TypeChecker;
use crate::resolve::DefId;
use crate::ty::{StructId, Ty, TypeId};

impl TypeChecker<'_, '_> {
    /// §3.5 — bit/dizi indeksi. Sabit indekste sınır denetimi yapılır.
    pub(super) fn index_result(&mut self, base_ty: TypeId, index: Idx<Expr>, span: Span) -> TypeId {
        match *self.types.ty(base_ty) {
            Ty::Error => self.types.error(),
            Ty::Array { elem, len } => {
                if let Some(i) = self.try_const_eval(index) {
                    if i < 0 || i >= i128::from(len) {
                        self.index_out_of_bounds(i, u64::from(len), span);
                        return self.types.error();
                    }
                }
                elem
            }
            Ty::UInt { width }
            | Ty::SInt { width }
            | Ty::Bits { width }
            | Ty::UIntFlex { hi: width, .. }
            | Ty::SIntFlex { hi: width, .. } => {
                if let Some(i) = self.try_const_eval(index) {
                    if i < 0 || i >= i128::from(width) {
                        self.index_out_of_bounds(i, u64::from(width), span);
                        return self.types.error();
                    }
                }
                self.types.bool_ty()
            }
            _ => {
                self.err_not_selectable(
                    span,
                    &lstr!(en: "bit selection is only allowed on numeric, bits or array types"; tr: "bit seçimi yalnız sayısal, bits veya dizi tipinde yapılır"),
                );
                self.types.error()
            }
        }
    }

    /// §3.5 — aralık seçimi; sınırlar derleme zamanı sabiti olmalı.
    pub(super) fn range_result(
        &mut self,
        base_ty: TypeId,
        hi: Idx<Expr>,
        lo: Idx<Expr>,
        span: Span,
    ) -> TypeId {
        if self.types.is_error(base_ty) {
            return self.types.error();
        }
        let Some(width) = self.types.width_of(base_ty) else {
            self.err_not_selectable(
                span,
                &lstr!(en: "range selection is only allowed on numeric or bits types"; tr: "aralık seçimi yalnız sayısal veya bits tipinde yapılır"),
            );
            return self.types.error();
        };
        match (self.try_const_eval(hi), self.try_const_eval(lo)) {
            (Some(h), Some(l)) => self.const_range_result(h, l, width, span),
            _ => {
                self.error(
                    ErrorCode::E2008,
                    span,
                    lstr!(en: "range bounds must be compile-time constants"; tr: "aralık sınırları derleme zamanı sabiti olmalı"),
                    lstr!(en: "non-constant bound"; tr: "değişken sınır"),
                    lstr!(en: "use x[i +: WIDTH] for a variable index"; tr: "değişken indeks için x[i +: WIDTH] kullanın"),
                );
                self.types.error()
            }
        }
    }

    /// Sabit sınırlı `[h:l]` aralığı: ters aralık E2007, taşma E2006.
    fn const_range_result(&mut self, h: i128, l: i128, width: u16, span: Span) -> TypeId {
        if h < l {
            self.error(
                ErrorCode::E2007,
                span,
                lstr!(en: "range is reversed (hi < lo)"; tr: "aralık ters (hi < lo)"),
                lstr!(en: "the high bit must be written first"; tr: "yüksek bit önce yazılmalı"),
                lstr!(en: "write [{l}:{h}]"; tr: "[{l}:{h}] yazın"),
            );
            return self.types.error();
        }
        if l < 0 || h >= i128::from(width) {
            self.error(
                ErrorCode::E2006,
                span,
                lstr!(en: "range out of bounds (width {width})"; tr: "aralık sınır dışı (genişlik {width})"),
                lstr!(en: "range exceeds the width of the base"; tr: "aralık taban genişliği aşıyor"),
                lstr!(en: "highest valid bit: {}", width - 1; tr: "geçerli en yüksek bit: {}", width - 1),
            );
            return self.types.error();
        }
        self.types.intern(Ty::Bits {
            width: (h - l + 1) as u16,
        })
    }

    /// ADR-0035 — indexed part-select `x[i +: W]` / `x[i -: W]`.
    /// Başlangıç indeksi değişken olabilir; genişlik derleme zamanı
    /// sabiti olmalı (SV kuralı, IEEE 1800 §11.5.1). Sonuç `bits<W>`.
    pub(super) fn part_select_result(
        &mut self,
        base_ty: TypeId,
        start: Idx<Expr>,
        width: Idx<Expr>,
        ascending: bool,
        span: Span,
    ) -> TypeId {
        if self.types.is_error(base_ty) {
            return self.types.error();
        }
        let Some(base_width) = self.types.width_of(base_ty) else {
            self.err_not_selectable(
                span,
                &lstr!(en: "part-select is only allowed on numeric or bits types"; tr: "parça seçimi yalnız sayısal veya bits tipinde yapılır"),
            );
            return self.types.error();
        };
        let Some(w) = self.part_select_width(width, base_width, span) else {
            return self.types.error();
        };
        // Başlangıç sabitse tüm seçim aralığı derleme zamanında denetlenir;
        // değişkense denetim çalışma zamanına kalır (kontratla sağlanır).
        if let Some(s) = self.try_const_eval(start) {
            let (lo, hi) = if ascending {
                (s, s + w - 1)
            } else {
                (s - w + 1, s)
            };
            if !self.const_select_in_bounds(lo, hi, base_width, span) {
                return self.types.error();
            }
        }
        self.types.intern(Ty::Bits { width: w as u16 })
    }

    /// Parça genişliği: derleme zamanı sabiti (E2008) ve 1..=taban
    /// genişliği aralığında (E2006) olmalı.
    fn part_select_width(&mut self, width: Idx<Expr>, base_width: u16, span: Span) -> Option<i128> {
        let Some(w) = self.try_const_eval(width) else {
            self.error(
                ErrorCode::E2008,
                span,
                lstr!(en: "part-select width must be a compile-time constant"; tr: "parça seçimi genişliği derleme zamanı sabiti olmalı"),
                lstr!(en: "non-constant width"; tr: "değişken genişlik"),
                lstr!(en: "make WIDTH a literal or const; only the start index may vary"; tr: "WIDTH'i literal veya const yapın; yalnız başlangıç indeksi değişebilir"),
            );
            return None;
        };
        if w < 1 || w > i128::from(base_width) {
            self.error(
                ErrorCode::E2006,
                span,
                lstr!(en: "part-select width {w} out of bounds (base width {base_width})"; tr: "parça seçimi genişliği {w} sınır dışı (taban genişliği {base_width})"),
                lstr!(en: "invalid part-select width"; tr: "geçersiz parça genişliği"),
                lstr!(en: "valid width range: 1..={base_width}"; tr: "geçerli genişlik aralığı: 1..={base_width}"),
            );
            return None;
        }
        Some(w)
    }

    /// Sabit `lo..=hi` seçimi taban genişliğine sığıyor mu? Sığmıyorsa
    /// taşan uç E2006 ile bildirilir.
    fn const_select_in_bounds(&mut self, lo: i128, hi: i128, base_width: u16, span: Span) -> bool {
        if lo >= 0 && hi < i128::from(base_width) {
            return true;
        }
        let bad = if lo < 0 { lo } else { hi };
        self.index_out_of_bounds(bad, u64::from(base_width), span);
        false
    }

    /// Alan erişimi: modül örneği portu, struct alanı veya demet indeksi.
    pub(super) fn field_result(&mut self, base_ty: TypeId, field: &Name, span: Span) -> TypeId {
        match self.types.ty(base_ty).clone() {
            Ty::Error => self.types.error(),
            // Port bulunamazsa E1009'u isim çözümleme verdi — sessiz.
            Ty::Instance(m) => self
                .port_type_of(DefId(m.0), &field.text)
                .unwrap_or_else(|| self.types.error()),
            // Yerleşik primitif portu: tablo üzerinden tiplenir; geçersiz
            // alan E1009'u isim çözümlemede aldı — sessiz Error.
            Ty::Builtin { prim, data, dim } => match prim.port(&field.text) {
                Some(port) => self.builtin_port_type(port.kind, data, dim),
                None => self.types.error(),
            },
            Ty::Struct(s) => self.struct_field_type(s, field),
            Ty::Tuple(items) => match field.text.parse::<usize>() {
                Ok(i) if i < items.len() => items[i],
                _ => {
                    self.err_type_mismatch_msg(
                        span,
                        &lstr!(en: "tuple index exceeds the number of elements"; tr: "demet indeksi eleman sayısını aşıyor"),
                        &lstr!(en: "use a valid tuple index"; tr: "geçerli bir demet indeksi kullanın"),
                    );
                    self.types.error()
                }
            },
            _ => {
                let shown = self.show(base_ty);
                self.err_type_mismatch_msg(
                    span,
                    &lstr!(en: "no field access on type '{shown}'"; tr: "'{shown}' tipinde alan erişimi yok"),
                    &lstr!(en: "field access is valid on structs and module instances"; tr: "alan erişimi struct ve modül örneklerinde geçerlidir"),
                );
                self.types.error()
            }
        }
    }

    /// Struct alanının bildirilen tipi; bilinmeyen alan sessiz Error.
    fn struct_field_type(&mut self, s: StructId, field: &Name) -> TypeId {
        let ast = self.ast;
        let field_ty = self.res.item_of_def.get(&DefId(s.0)).and_then(|&item_idx| {
            match &ast.items_arena[item_idx].kind {
                ItemKind::Struct(decl) => decl
                    .fields
                    .iter()
                    .find(|f| f.name.text == field.text)
                    .map(|f| f.ty),
                _ => None,
            }
        });
        match field_ty {
            Some(t) => self.resolve_type_ref(t),
            None => self.types.error(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{codes, def_ty};

    fn module(body: &str) -> String {
        format!(
            "module M {{\n    in  a : u8\n    in  i : u8\n    in  f : bool\n    out y : u8\n\n{body}\n    y = a\n}}\n"
        )
    }

    #[test]
    fn bit_index_is_bool_and_constant_index_is_bounds_checked() {
        assert_eq!(def_ty(&module("    let _b = a[3]"), "_b"), "bool");
        assert!(codes(&module("    let _b = a[8]")).contains(&"E2006"));
    }

    #[test]
    fn range_yields_bits_of_the_selected_width() {
        assert_eq!(def_ty(&module("    let _r = a[5:2]"), "_r"), "bits<4>");
    }

    #[test]
    fn reversed_and_non_constant_ranges_are_rejected() {
        assert!(codes(&module("    let _r = a[2:5]")).contains(&"E2007"));
        assert!(codes(&module("    let _r = a[i:0]")).contains(&"E2008"));
    }

    #[test]
    fn part_select_allows_variable_start_but_checks_constant_one() {
        assert_eq!(def_ty(&module("    let _p = a[i +: 4]"), "_p"), "bits<4>");
        assert!(codes(&module("    let _p = a[6 +: 4]")).contains(&"E2006"));
    }

    #[test]
    fn selection_on_bool_is_e2003() {
        assert!(codes(&module("    let _b = f[0]")).contains(&"E2003"));
    }
}
