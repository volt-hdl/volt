//! Bit, aralık, parça ve alan seçimi (type-inference.md §3.5;
//! ADR-0035 indexed part-select). Hem ifade (`synth`) hem atama hedefi
//! (`lvalue_type`) aynı sonuç kurallarını kullanır; indeksin tipi
//! kuralı (`reject_index_type`; Trit #465, işaretli tamsayı #475) her
//! seçim yolunun ortak yeridir.

use volt_ast::{Expr, Idx, ItemKind, Name};
use volt_diagnostics::{
    lstr, Applicability, Diagnostic, ErrorCode, LabeledSpan, NoteKind, Suggestion,
};
use volt_span::Span;

use super::TypeChecker;
use crate::resolve::DefId;
use crate::ty::{StructId, Ty, TypeId};

/// İndeks tipi kuralını çağıran seçim biçimi: adres genişliği ve yardımın
/// kodu ona bağlıdır.
#[derive(Clone, Copy)]
enum IndexForm {
    /// Bit ya da dizi indeksi `x[i]`, `arr[i]`.
    Index,
    /// Aralık seçiminin üst (`true`) ya da alt sınırı `x[h : l]`.
    Range(bool),
    /// Parça seçiminin başlangıcı `x[i +: W]` (`true`) ya da `x[i -: W]`.
    Part(bool),
}

impl TypeChecker<'_, '_> {
    /// §3.5 — indeksin tipi; kuralın tek yeri (ADR-0137). Bit/dizi
    /// indeksi, aralık sınırları ve parça seçiminin başlangıcı, ifadede ve
    /// atama hedefinde (`synth_select`, `lvalue_type`); indeksin tipini
    /// çağıranlar sentezler. İki ret, ikisi de E2003 ve tipe bakar, değere
    /// değil:
    /// - tipi Trit tutan indeks (Trit ya da Trit tutan dizi, ADR-0142'nin
    ///   `holds_trit`'i; #465, ADR-0146);
    /// - tipi işaretli tamsayı olan indeks (`iN` ve esnek `iN`; #475,
    ///   ADR-0147). SV indeksi işaretsiz okur: i : i3 = -1 için `x[i]`
    ///   `x[7]`'dir. Yardımın genişliği tabanın adresidir; adresi olmayan
    ///   tabanda (hata ya da seçilemeyen tip) ret yoktur, o tabanın kendi
    ///   tanısı çıkar.
    ///
    /// Öteki indeks tipleri (uN, literal, bits<N>, bool, enum, Trit
    /// tutmayan dizi) değişmez. İndeks reddedildiyse `true`.
    fn reject_index_type(&mut self, base_ty: TypeId, index: Idx<Expr>, form: IndexForm) -> bool {
        let Some(&ty) = self.expr_types.get(&index) else {
            return false;
        };
        if self.holds_trit(ty) {
            self.trit_index_error(ty, index);
            return true;
        }
        if !self.types.is_signed(ty) {
            return false;
        }
        let Some(width) = self.address_width(base_ty, form) else {
            return false;
        };
        self.signed_index_error(ty, base_ty, index, form, width);
        true
    }

    /// Tabanın adres genişliği: konum sayısını (dizide eleman, vektörde
    /// bit) işaretsiz tutan en dar genişlik, en az 1 (`u8` → 3, `[u8; 4]`
    /// → 2, `u5` → 3, `u1` → 1). Dizi yalnız bit/dizi indeksinde
    /// seçilebilir; aralık ve parça seçimi vektör ister.
    fn address_width(&self, base_ty: TypeId, form: IndexForm) -> Option<u32> {
        let positions = match (self.types.ty(base_ty), form) {
            (Ty::Array { len, .. }, IndexForm::Index) => u64::from(*len),
            _ => u64::from(self.types.width_of(base_ty)?),
        };
        Some((u64::BITS - positions.saturating_sub(1).leading_zeros()).max(1))
    }

    /// İşaretli indeksin E2003'ü (#475, ADR-0147). Yardım indeksi tabanın
    /// adres genişliğinde işaretsize çevirir; daha geniş bir dönüşüm
    /// Verilator'da WIDTHTRUNC'a düşerdi (#474).
    fn signed_index_error(
        &mut self,
        ty: TypeId,
        base_ty: TypeId,
        index: Idx<Expr>,
        form: IndexForm,
        width: u32,
    ) {
        let shown = self.show(ty);
        let base = self.show(base_ty);
        let name = if matches!(self.types.ty(base_ty), Ty::Array { .. }) {
            "arr"
        } else {
            "x"
        };
        let conv = format!("as u{width}");
        let code = match form {
            IndexForm::Index => format!("{name}[i {conv}]"),
            IndexForm::Range(true) => format!("{name}[h {conv} : l]"),
            IndexForm::Range(false) => format!("{name}[h : l {conv}]"),
            IndexForm::Part(true) => format!("{name}[i {conv} +: W]"),
            IndexForm::Part(false) => format!("{name}[i {conv} -: W]"),
        };
        let max = (1u64 << width) - 1;
        self.diagnostics.push(
            Diagnostic::error(
                ErrorCode::E2003,
                lstr!(en: "an index must be unsigned, found '{shown}'"; tr: "indeks işaretsiz olmalı, bulunan '{shown}'"),
                LabeledSpan::primary(
                    self.ast.exprs[index].span,
                    lstr!(en: "a negative value of this index would select a high bit or element"; tr: "bu indeksin negatif bir değeri yüksek bir biti ya da elemanı seçerdi"),
                ),
                lstr!(en: "convert the index explicitly to an unsigned number of the address width: {code} for {name} : {base}; a negative value then becomes a large index, -1 {conv} is {max}";
                      tr: "indeksi açıkça adres genişliğinde işaretsiz bir sayıya çevirin: {name} : {base} için {code}; negatif bir değer o zaman büyük bir indeks olur, -1 {conv} = {max}"),
            )
            .with_note(
                NoteKind::Reason,
                lstr!(en: "SystemVerilog reads an index as an unsigned number, so a negative value selects a high bit or element without a diagnostic";
                      tr: "SystemVerilog indeksi işaretsiz bir sayı olarak okur; bu yüzden negatif bir değer tanısız olarak yüksek bir biti ya da elemanı seçer"),
            ),
        );
    }

    /// Trit tutan indeksin E2003'ü (#465, ADR-0146). Trit'in değer kümesi
    /// {-1, 0, +1}'dir (ADR-0003) ve -1 bir konum değildir; indeks ise
    /// Trit'i kodlayan bitleri işaretsiz bir sayı olarak okurdu.
    fn trit_index_error(&mut self, ty: TypeId, index: Idx<Expr>) {
        let shown = self.show(ty);
        self.diagnostics.push(
            Diagnostic::error(
                ErrorCode::E2003,
                lstr!(en: "an index must not be a Trit, found '{shown}'"; tr: "indeks Trit olamaz, bulunan '{shown}'"),
                LabeledSpan::primary(
                    self.ast.exprs[index].span,
                    lstr!(en: "the index would read the bits that encode the Trit, not its value"; tr: "indeks Trit'in değerini değil onu kodlayan bitleri okurdu"),
                ),
                lstr!(en: "convert the Trit to an unsigned index explicitly: x[(t as i3 + 1) as u3] reads x[0], x[1] and x[2] for t = -1, 0 and +1";
                      tr: "Trit'i açıkça işaretsiz bir indekse çevirin: x[(t as i3 + 1) as u3], t = -1, 0 ve +1 için x[0], x[1] ve x[2]'yi okur"),
            )
            .with_note(
                NoteKind::Reason,
                lstr!(en: "a Trit is -1, 0 or +1 and -1 is no bit or element position; an index reads the bits that encode the Trit as an unsigned number";
                      tr: "Trit -1, 0 ya da +1'dir ve -1 bir bit ya da eleman konumu değildir; indeks Trit'i kodlayan bitleri işaretsiz bir sayı olarak okur"),
            ),
        );
    }

    /// §3.5 — bit/dizi indeksi. Sabit indekste sınır denetimi yapılır.
    pub(super) fn index_result(&mut self, base_ty: TypeId, index: Idx<Expr>, span: Span) -> TypeId {
        if self.reject_index_type(base_ty, index, IndexForm::Index) {
            return self.types.error();
        }
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
            Ty::Bool
            | Ty::Trit
            | Ty::Clock
            | Ty::Reset { .. }
            | Ty::Tuple(_)
            | Ty::Struct(_)
            | Ty::Enum(_)
            | Ty::Instance(_)
            | Ty::Builtin { .. }
            | Ty::Delayed { .. }
            | Ty::IntLit => {
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
        let bad_hi = self.reject_index_type(base_ty, hi, IndexForm::Range(true));
        let bad_lo = self.reject_index_type(base_ty, lo, IndexForm::Range(false));
        if bad_hi || bad_lo || self.types.is_error(base_ty) {
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
            (Some(h), Some(l)) => self.const_range_result(h, l, width, span, (hi, lo)),
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
    fn const_range_result(
        &mut self,
        h: i128,
        l: i128,
        width: u16,
        span: Span,
        (hi, lo): (Idx<Expr>, Idx<Expr>),
    ) -> TypeId {
        if h < l {
            self.error(
                ErrorCode::E2007,
                span,
                lstr!(en: "range is reversed (hi < lo)"; tr: "aralık ters (hi < lo)"),
                lstr!(en: "the high bit must be written first"; tr: "yüksek bit önce yazılmalı"),
                lstr!(en: "write [{l}:{h}]"; tr: "[{l}:{h}] yazın"),
            );
            // suggestion: e2007_reversed_range
            self.suggest_last(
                Suggestion::replace(
                    self.ast.exprs[hi].span,
                    l.to_string(),
                    Applicability::MaybeIncorrect,
                )
                .and_replace(self.ast.exprs[lo].span, h.to_string()),
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
        if self.reject_index_type(base_ty, start, IndexForm::Part(ascending))
            || self.types.is_error(base_ty)
        {
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
            Ty::Bool
            | Ty::UInt { .. }
            | Ty::SInt { .. }
            | Ty::Bits { .. }
            | Ty::Trit
            | Ty::Clock
            | Ty::Reset { .. }
            | Ty::Array { .. }
            | Ty::Enum(_)
            | Ty::Delayed { .. }
            | Ty::IntLit
            | Ty::UIntFlex { .. }
            | Ty::SIntFlex { .. } => {
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

    /// Struct alanının bildirilen tipi; bilinmeyen alan E1008 (ADR-0077).
    fn struct_field_type(&mut self, s: StructId, field: &Name) -> TypeId {
        let ast = self.ast;
        let field_ty = self.res.item_of_def.get(&DefId(s.0)).and_then(|&item_idx| {
            let ItemKind::Struct(decl) = &ast.items_arena[item_idx].kind else {
                return None;
            };
            decl.fields
                .iter()
                .find(|f| f.name.text == field.text)
                .map(|f| f.ty)
        });
        match field_ty {
            Some(t) => self.resolve_type_ref(t),
            None => {
                self.unknown_struct_field(s, field);
                self.types.error()
            }
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
