//! Struct bildirim denetimi, literal ve dönüşüm kuralları (ADR-0077).
//!
//! Bit düzeni kuralı `volt_ast::struct_layout`'tadır (sv-emit, sürücü
//! analizi, simülasyon raporu ve LSP ile ortak); burada ihlaller tanıya
//! çevrilir: E2013 (geçersiz bildirim), E1003 (yinelenen alan), E4010
//! (düzleştirme bütçesi), E2014 (literalde eksik/yinelenen alan), E1008
//! (bilinmeyen alan erişimi), E2009 (`as` kuralları, Karar 4). Geçerli
//! düzenin genişliği `TypeArena`'ya yazılır — `signal_width` (W3003)
//! oradan okur.

use volt_ast::struct_layout::{self, LayoutError, StructLayout};
use volt_ast::{FieldInit, ItemKind, Name, StructDecl};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan};
use volt_span::Span;

use super::TypeChecker;
use crate::resolve::{closest_match, DefId};
use crate::ty::{StructId, Ty, TypeId};

impl<'a> TypeChecker<'a, '_> {
    /// Birimdeki her düz struct bildirimini denetler, geçerli düzenlerin
    /// genişliğini kaydeder.
    pub(super) fn check_struct_decls(&mut self) {
        let ast = self.ast;
        for &item_idx in &ast.items {
            let ItemKind::Struct(decl) = &ast.items_arena[item_idx].kind else {
                continue;
            };
            if decl.is_port {
                continue;
            }
            self.check_duplicate_fields(decl);
            self.check_field_kinds(decl);
            let Some(&def) = self.res.decl_spans.get(&decl.name.span) else {
                continue;
            };
            match struct_layout::layout(ast, decl, &mut |e| self.try_const_eval(e)) {
                Ok(layout) => self.types.set_struct_width(StructId(def.0), layout.width),
                Err(LayoutError::TooDeep) => self.flatten_budget(decl, false),
                Err(LayoutError::TooManyLeaves) => self.flatten_budget(decl, true),
                // Alansız struct ve bundle alanı `check_field_kinds`'te
                // (E2013); generic, struct dizisi ve desteklenmeyen alan
                // tipi yalnız sinyal tipi olarak kullanılınca E0003 alır.
                Err(_) => {}
            }
        }
    }

    /// E1003 — aynı adlı iki alan.
    fn check_duplicate_fields(&mut self, decl: &StructDecl) {
        for (j, f) in decl.fields.iter().enumerate() {
            if decl.fields[..j].iter().any(|g| g.name.text == f.name.text) {
                let (sname, name) = (&decl.name.text, &f.name.text);
                self.error(
                    ErrorCode::E1003,
                    f.name.span,
                    lstr!(en: "field '{name}' is declared twice in struct '{sname}'"; tr: "'{name}' alanı '{sname}' struct'ında iki kez bildirilmiş"),
                    lstr!(en: "duplicate field"; tr: "yinelenen alan"),
                    lstr!(en: "rename or remove one of the two fields"; tr: "iki alandan birini yeniden adlandırın ya da kaldırın"),
                );
            }
        }
    }

    /// E2013 — alansız struct, alanda domain notasyonu, bundle tipli alan.
    fn check_field_kinds(&mut self, decl: &StructDecl) {
        let ast = self.ast;
        let sname = decl.name.text.clone();
        if decl.fields.is_empty() {
            self.error(
                ErrorCode::E2013,
                decl.name.span,
                lstr!(en: "struct '{sname}' has no fields"; tr: "'{sname}' struct'ının alanı yok"),
                lstr!(en: "empty struct"; tr: "boş struct"),
                lstr!(en: "a zero-width value is no signal; add a field: struct {sname} {{ a : u4 }}"; tr: "sıfır genişlikli değer sinyal değildir; alan ekleyin: struct {sname} {{ a : u4 }}"),
            );
        }
        for f in &decl.fields {
            let fname = f.name.text.clone();
            if let Some(domain) = &f.domain {
                let d = domain.text.clone();
                self.error(
                    ErrorCode::E2013,
                    domain.span,
                    lstr!(en: "field '{fname}' of struct '{sname}' has a clock-domain annotation '@{d}'"; tr: "'{sname}' struct'ının '{fname}' alanında '@{d}' saat alanı notasyonu var"),
                    lstr!(en: "a plain struct field has no domain"; tr: "düz struct alanının domain'i yoktur"),
                    lstr!(en: "put the domain on the signal that holds the struct (in p : {sname} @{d}), or declare a 'struct port' for per-field domains"; tr: "domain'i struct'ı tutan sinyale yazın (in p : {sname} @{d}) ya da alan başına domain için 'struct port' bildirin"),
                );
            }
            let target = struct_layout::struct_of_type(ast, f.ty);
            let bundle = match &ast.types[f.ty].kind {
                volt_ast::TypeRefKind::Path { path, .. } if target.is_none() => path
                    .segments
                    .last()
                    .map(|s| s.text.clone())
                    .filter(|n| struct_layout::is_bundle(ast, n)),
                _ => None,
            };
            if let Some(b) = bundle {
                self.error(
                    ErrorCode::E2013,
                    ast.types[f.ty].span,
                    lstr!(en: "field '{fname}' of struct '{sname}' has the 'struct port' bundle type '{b}'"; tr: "'{sname}' struct'ının '{fname}' alanı 'struct port' bundle tipi '{b}' taşıyor"),
                    lstr!(en: "a value cannot contain directed port fields"; tr: "bir değer yönlü port alanları içeremez"),
                    lstr!(en: "use a plain struct for the field, or make '{sname}' a 'struct port' itself"; tr: "alan için düz bir struct kullanın ya da '{sname}' tipini de 'struct port' yapın"),
                );
            }
        }
    }

    /// E4010 — iç içe struct derinliği ya da yaprak sayısı bütçeyi aşıyor.
    fn flatten_budget(&mut self, decl: &StructDecl, leaves: bool) {
        let sname = decl.name.text.clone();
        let (message, label) = if leaves {
            let max = struct_layout::MAX_LEAVES;
            (
                lstr!(en: "flattening struct '{sname}' would produce more than {max} leaf signals"; tr: "'{sname}' struct'ını açmak {max}'dan çok yaprak sinyal üretirdi"),
                lstr!(en: "too many leaf signals"; tr: "çok fazla yaprak sinyal"),
            )
        } else {
            let max = struct_layout::MAX_NESTING;
            (
                lstr!(en: "struct '{sname}' nests deeper than {max} levels"; tr: "'{sname}' struct'ı {max} seviyeden derin iç içe"),
                lstr!(en: "nested too deep"; tr: "çok derin iç içe"),
            )
        };
        self.error(
            ErrorCode::E4010,
            decl.name.span,
            message,
            label,
            lstr!(en: "flatten the type hierarchy or split the value into several signals (ADR-0077)"; tr: "tip hiyerarşisini sadeleştirin ya da değeri birkaç sinyale bölün (ADR-0077)"),
        );
    }

    /// E2021 — struct register'ının reset değeri sabit bir struct değeri
    /// olmalı: sabit alanlı literal ya da `const` struct (Karar 4).
    pub(super) fn check_struct_reset(&mut self, r: &volt_ast::RegDecl, ty: TypeId) {
        let Ty::Struct(_) = self.types.ty(ty) else {
            return;
        };
        let before = self.ev.diagnostics.len();
        let value = self.ev.const_eval(r.init);
        self.ev.diagnostics.truncate(before);
        if !matches!(value, crate::ConstValue::Error) {
            return;
        }
        let (name, shown) = (r.name.text.clone(), self.show(ty));
        self.error(
            ErrorCode::E2021,
            self.ast.exprs[r.init].span,
            lstr!(en: "the reset value of struct register '{name}' is not a compile-time constant"; tr: "'{name}' struct register'ının reset değeri derleme zamanı sabiti değil"),
            lstr!(en: "not a constant"; tr: "sabit değil"),
            lstr!(en: "write a literal with constant fields ({shown} {{ ... }}) or name a const {shown}; load run-time values in an 'on' block"; tr: "sabit alanlı bir literal ({shown} {{ ... }}) yazın ya da bir const {shown} kullanın; çalışma zamanı değerlerini 'on' bloğunda yükleyin"),
        );
    }

    /// `StructId`'nin bildirimi.
    pub(super) fn struct_decl(&self, s: StructId) -> Option<&'a StructDecl> {
        let ast = self.ast;
        let &item_idx = self.res.item_of_def.get(&DefId(s.0))?;
        match &ast.items_arena[item_idx].kind {
            ItemKind::Struct(decl) => Some(decl),
            _ => None,
        }
    }

    /// Geçerli struct'ın bit düzeni (Karar 3); geçersizse `None`.
    pub(super) fn struct_layout(&mut self, s: StructId) -> Option<StructLayout> {
        let decl = self.struct_decl(s)?;
        let ast = self.ast;
        struct_layout::layout(ast, decl, &mut |e| self.try_const_eval(e)).ok()
    }

    /// E2014 — literalde eksik ya da yinelenen alan; bilinmeyen alan
    /// çözümlemede E1008 aldı.
    pub(super) fn check_struct_lit_fields(
        &mut self,
        decl: &StructDecl,
        fields: &[FieldInit],
        span: Span,
    ) {
        let sname = decl.name.text.clone();
        for (j, init) in fields.iter().enumerate() {
            if fields[..j].iter().any(|g| g.name.text == init.name.text) {
                let name = init.name.text.clone();
                self.error(
                    ErrorCode::E2014,
                    init.name.span,
                    lstr!(en: "field '{name}' is given twice in the '{sname}' literal"; tr: "'{name}' alanı '{sname}' literalinde iki kez verilmiş"),
                    lstr!(en: "repeated field"; tr: "yinelenen alan"),
                    lstr!(en: "give each field exactly once"; tr: "her alanı tam bir kez verin"),
                );
            }
        }
        let missing: Vec<String> = decl
            .fields
            .iter()
            .filter(|f| !fields.iter().any(|i| i.name.text == f.name.text))
            .map(|f| f.name.text.clone())
            .collect();
        if missing.is_empty() {
            return;
        }
        let list = missing
            .iter()
            .map(|m| format!("'{m}'"))
            .collect::<Vec<_>>()
            .join(", ");
        let example = missing
            .iter()
            .map(|m| format!("{m}: ..."))
            .collect::<Vec<_>>()
            .join(", ");
        self.error(
            ErrorCode::E2014,
            span,
            lstr!(en: "the '{sname}' literal is missing field(s) {list}"; tr: "'{sname}' literalinde {list} alan(lar)ı eksik"),
            lstr!(en: "every field needs a value"; tr: "her alanın değeri olmalı"),
            lstr!(en: "add the missing fields: {sname} {{ ..., {example} }} — every bit needs an explicit source"; tr: "eksik alanları ekleyin: {sname} {{ ..., {example} }} — her bitin açık bir kaynağı olmalı"),
        );
    }

    /// E1008 — struct'ta olmayan alana erişim (`p.nope`).
    pub(super) fn unknown_struct_field(&mut self, s: StructId, field: &Name) {
        let Some(decl) = self.struct_decl(s) else {
            return;
        };
        let known: Vec<String> = decl.fields.iter().map(|f| f.name.text.clone()).collect();
        let (sname, name) = (decl.name.text.clone(), field.text.clone());
        let help = match closest_match(&name, &known) {
            Some(m) => lstr!(en: "did you mean '{m}'?"; tr: "'{m}' mi demek istediniz?"),
            None => {
                lstr!(en: "available fields: {}", known.join(", "); tr: "mevcut alanlar: {}", known.join(", "))
            }
        };
        self.diagnostics.push(Diagnostic::error(
            ErrorCode::E1008,
            lstr!(en: "struct '{sname}' has no field '{name}'"; tr: "'{sname}' yapısında '{name}' alanı yok"),
            LabeledSpan::primary(field.span, lstr!(en: "unknown field"; tr: "bilinmeyen alan")),
            help,
        ));
    }

    /// Struct dönüşümleri (Karar 4). Struct tarafı yoksa `false`.
    ///
    /// - `p as uN` / `p as bits<N>`: `N ≥ W` (sıfır genişletme).
    /// - `raw as P`: `raw` `uN`/`bits<N>`, `N == W`, `P`'de enum/`Trit`
    ///   yaprağı yok.
    /// - Diğer her şey E2009.
    pub(super) fn check_struct_cast(&mut self, src: TypeId, dst: TypeId, span: Span) -> bool {
        match (self.types.ty(src).clone(), self.types.ty(dst).clone()) {
            (Ty::Struct(s), Ty::UInt { width } | Ty::Bits { width }) => {
                let Some(layout) = self.struct_layout(s) else {
                    return true; // geçersiz düzen kendi tanısını aldı
                };
                let w = layout.width;
                if u32::from(width) < w {
                    let (src_s, dst_s) = (self.show(src), self.show(dst));
                    self.error(
                        ErrorCode::E2009,
                        span,
                        lstr!(en: "cast '{src_s}' → '{dst_s}' loses information: the struct is {w} bits wide"; tr: "'{src_s}' → '{dst_s}' dönüşümü bilgi kaybeder: struct {w} bit genişliğinde"),
                        lstr!(en: "target narrower than the struct"; tr: "hedef struct'tan dar"),
                        lstr!(en: "cast to at least {w} bits: p as u{w} (the first field is the most significant, ADR-0077)"; tr: "en az {w} bite dönüştürün: p as u{w} (ilk alan en anlamlı bitlerde, ADR-0077)"),
                    );
                }
                true
            }
            (Ty::Struct(_), _) => {
                let (src_s, dst_s) = (self.show(src), self.show(dst));
                self.error(
                    ErrorCode::E2009,
                    span,
                    lstr!(en: "cast '{src_s}' → '{dst_s}' is invalid"; tr: "'{src_s}' → '{dst_s}' dönüşümü geçersiz"),
                    lstr!(en: "this cast is not defined"; tr: "bu dönüşüm tanımlı değil"),
                    lstr!(en: "a struct converts only to an unsigned uN or bits<N> at least as wide as the struct"; tr: "struct yalnız kendisi kadar geniş işaretsiz uN ya da bits<N>'e dönüşür"),
                );
                true
            }
            (Ty::UInt { width } | Ty::Bits { width }, Ty::Struct(s)) => {
                self.check_bits_to_struct(src, dst, s, u32::from(width), span);
                true
            }
            (_, Ty::Struct(_)) => {
                let (src_s, dst_s) = (self.show(src), self.show(dst));
                self.error(
                    ErrorCode::E2009,
                    span,
                    lstr!(en: "cast '{src_s}' → '{dst_s}' is invalid"; tr: "'{src_s}' → '{dst_s}' dönüşümü geçersiz"),
                    lstr!(en: "this cast is not defined"; tr: "bu dönüşüm tanımlı değil"),
                    lstr!(en: "only an unsigned uN or bits<N> exactly as wide as the struct converts to it; or build it with a literal: {dst_s} {{ ... }}"; tr: "struct'a yalnız tam onun genişliğindeki işaretsiz uN ya da bits<N> dönüşür; ya da literalle kurun: {dst_s} {{ ... }}"),
                );
                true
            }
            _ => false,
        }
    }

    /// `raw as P` — genişlik eşit olmalı, enum/`Trit` yaprağı olmamalı.
    fn check_bits_to_struct(
        &mut self,
        src: TypeId,
        dst: TypeId,
        s: StructId,
        width: u32,
        span: Span,
    ) {
        let Some(layout) = self.struct_layout(s) else {
            return;
        };
        let (src_s, dst_s) = (self.show(src), self.show(dst));
        if let Some(leaf) = layout
            .leaves
            .iter()
            .find(|l| l.kind != struct_layout::LeafKind::Plain)
        {
            let field = leaf.dotted();
            let (lo, hi) = (leaf.lsb, leaf.msb());
            self.error(
                ErrorCode::E2009,
                span,
                lstr!(en: "cast '{src_s}' → '{dst_s}' is invalid: field '{field}' is an enum or Trit, and raw bits may hold a code that is no valid value"; tr: "'{src_s}' → '{dst_s}' dönüşümü geçersiz: '{field}' alanı enum ya da Trit ve ham bitler geçerli olmayan bir kod taşıyabilir"),
                lstr!(en: "no implicit decoding into an enum field"; tr: "enum alanına örtük çözme yok"),
                lstr!(en: "build the struct field by field and decode the enum explicitly: {dst_s} {{ {field}: decode(raw[{hi}:{lo}]), ... }} (decode with a match)"; tr: "struct'ı alan alan kurun ve enum'u açıkça çözün: {dst_s} {{ {field}: coz(raw[{hi}:{lo}]), ... }} (match ile çözün)"),
            );
            return;
        }
        let w = layout.width;
        if width != w {
            self.error(
                ErrorCode::E2009,
                span,
                lstr!(en: "cast '{src_s}' → '{dst_s}' is invalid: the struct is {w} bits wide, the source {width}"; tr: "'{src_s}' → '{dst_s}' dönüşümü geçersiz: struct {w} bit, kaynak {width} bit"),
                lstr!(en: "widths differ"; tr: "genişlikler farklı"),
                lstr!(en: "cast the source to exactly {w} bits first: (raw as u{w}) as {dst_s} — no implicit truncation or extension"; tr: "önce kaynağı tam {w} bite dönüştürün: (raw as u{w}) as {dst_s} — örtük kesme ya da genişletme yok"),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{codes, def_ty, diagnostics};

    const P: &str = "struct P {\n    a : u4\n    b : bool\n}\n";

    fn module(decls: &str, body: &str) -> String {
        format!(
            "{P}{decls}module M {{\n    in  clk : clock\n    in  x : u4\n    in  go : bool\n    in  raw : u5\n    in  p : P\n    in  q : P\n    out y : bool\n\n{body}\n    y = go\n}}\n"
        )
    }

    #[test]
    fn declaration_rules_are_e2013_and_e1003() {
        let empty = module("struct E { }\n", "");
        assert_eq!(codes(&empty), ["E2013"]);
        let dup = module("struct D {\n    a : u4\n    a : bool\n}\n", "");
        assert_eq!(codes(&dup), ["E1003"]);
        let bundle = module(
            "struct port B {\n    out d : u4\n}\nstruct S {\n    x : B\n}\n",
            "",
        );
        assert_eq!(codes(&bundle), ["E2013"]);
        let domain = module(
            "domain Fast {\n    clock = posedge\n}\nstruct F {\n    a : u4 @Fast\n}\n",
            "",
        );
        assert_eq!(codes(&domain), ["E2013"]);
    }

    #[test]
    fn literal_must_give_every_field_once() {
        assert!(codes(&module("", "    let _l : P = P { a: x, b: go }")).is_empty());
        let missing = module("", "    let _l : P = P { a: x }");
        assert_eq!(codes(&missing), ["E2014"]);
        let d = diagnostics(&missing);
        assert!(d[0].message.contains("'b'"), "{}", d[0].message);
        assert_eq!(
            codes(&module("", "    let _l : P = P { a: x, a: x, b: go }")),
            ["E2014"]
        );
    }

    #[test]
    fn field_assignments_and_literal_fields_are_checked_against_the_field_type() {
        let reg = "    reg r : P = P { a: 0, b: false }
    on clk { r.a <= go }";
        assert_eq!(codes(&module("", reg)), ["E2003"]);
        let d = diagnostics(&module("", reg));
        assert!(
            d[0].message.contains("'u4'") && d[0].message.contains("'bool'"),
            "{}",
            d[0].message
        );
        assert_eq!(
            codes(&module("", "    let _l : P = P { a: go, b: go }")),
            ["E2003"]
        );
        assert!(codes(&module(
            "",
            "    reg r : P = P { a: 0, b: false }
    on clk { r.a <= x }"
        ))
        .is_empty());
    }

    #[test]
    fn unknown_field_access_is_e1008() {
        let d = diagnostics(&module("", "    let _l = p.nope"));
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].code.as_str(), "E1008");
        assert!(d[0].message.contains("'P'"), "{}", d[0].message);
    }

    #[test]
    fn ordering_and_operators_on_structs_are_e2003() {
        for body in [
            "    let _x = p < q",
            "    let _x = ~p",
            "    let _x = p + q",
            "    let _x = p & q",
            "    let _x = p[0]",
        ] {
            assert_eq!(codes(&module("", body)), ["E2003"], "{body}");
        }
        assert!(codes(&module("", "    let _x = p == q\n    let _n = p != q")).is_empty());
    }

    #[test]
    fn messages_name_the_structs() {
        let d = diagnostics(&module(
            "struct Q {\n    c : u5\n}\n",
            "    let _x = p == (raw as Q)",
        ));
        assert!(d[0].message.contains("'P' and 'Q'"), "{}", d[0].message);
        let d = diagnostics(&module("", "    let _x : bool = p == x"));
        assert!(d[0].message.contains("'P'"), "{}", d[0].message);
    }

    #[test]
    fn struct_casts_follow_the_layout_width() {
        let ok = module("", "    let _u = p as u5\n    let _w = p as u8\n    let _b = p as bits<5>\n    let _s = raw as P");
        assert!(codes(&ok).is_empty(), "{:?}", codes(&ok));
        assert_eq!(def_ty(&ok, "_s"), "struct");
        for body in [
            "    let _x = p as u4",
            "    let _x = p as i8",
            "    let _x = p as bool",
            "    let _x = x as P",
            "    let _x = go as P",
        ] {
            assert_eq!(codes(&module("", body)), ["E2009"], "{body}");
        }
    }

    #[test]
    fn bits_to_struct_with_enum_field_is_e2009() {
        let src = "enum St { Idle, Run }\nstruct E {\n    s : St\n    n : u3\n}\nmodule M {\n    in  raw : u4\n    out y : bool\n    let _e = raw as E\n    y = true\n}\n";
        let d = diagnostics(src);
        assert_eq!(d.len(), 1, "{d:?}");
        assert_eq!(d[0].code.as_str(), "E2009");
        assert!(
            d[0].help.as_deref().unwrap_or("").contains("E {"),
            "{:?}",
            d[0].help
        );
    }
}
