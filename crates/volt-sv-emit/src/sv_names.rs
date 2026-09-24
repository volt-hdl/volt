//! Üretilen SV tanımlayıcılarının anahtar sözcük denetimi (ADR-0078, E1013).
//!
//! Volt adları SV'ye aynen ya da `_` ile birleşerek iner; bir
//! SystemVerilog anahtar sözcüğüne denk gelen ad çıktıyı her araçta
//! sözdizimi hatası yapar. Denetim emitter'dadır, çünkü son SV adını
//! (struct yaprağı, bundle alanı, `for` açılımı, enum localparam'ı, örnek
//! çıkış teli) yalnız burada bilinir; `volt check` ve LSP aynı emit'i
//! çıktısız koşar (ADR-0070), yani üç yol aynı tanıyı görür.
//!
//! İki katman:
//! 1. Kesin denetimler — adın üretildiği yerde, kaynağa işaret eden
//!    span'le: [`Emitter::audit_unit_names`], [`Emitter::audit_module_names`],
//!    [`Emitter::check_sv_name`] (örnek çıkış telleri).
//! 2. Güvenlik ağı — [`Emitter::audit_emitted_text`]: üretilen modül
//!    metnindeki her belirteç ya emitter'ın kendi sözdizimi sözcüğüdür
//!    ([`EMITTER_KEYWORDS`]) ya da anahtar sözcük değildir. Kesin
//!    denetimlerin kaçırdığı yeni bir ad yolu burada yakalanır.

use std::collections::HashSet;

use volt_ast::reserved::is_sv_keyword;
use volt_ast::{ItemKind, ModuleDecl, StmtKind};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan};
use volt_span::Span;

use crate::Emitter;

/// Emitter'ın SV sözdizimi olarak yazdığı anahtar sözcükler (bütün SVA
/// kipleri dahil). Üretilen metinde bunların DIŞINDAKİ bir anahtar sözcük
/// belirteci, bir tanımlayıcı olarak yazılmıştır. Yeni bir SV yapısı
/// üreten değişiklik sözcüğünü buraya ekler (eklemezse güvenlik ağı
/// bütün tasarımlarda E1013 verir ve testler düşer). Sıralı.
pub(crate) const EMITTER_KEYWORDS: &[&str] = &[
    "always",
    "always_comb",
    "always_ff",
    "assert",
    "assign",
    "assume",
    "automatic",
    "begin",
    "bind",
    "case",
    "context",
    "cover",
    "default",
    "disable",
    "else",
    "end",
    "endcase",
    "endfunction",
    "endmodule",
    "endproperty",
    "final",
    "for",
    "function",
    "if",
    "iff",
    "import",
    "initial",
    "inout",
    "input",
    "int",
    "localparam",
    "logic",
    "longint",
    "module",
    "negedge",
    "or",
    "output",
    "posedge",
    "property",
    "signed",
    "string",
    "tri1",
    "void",
    "wire",
];

/// Adın ne olduğu — tanı etiketinde.
#[derive(Clone, Copy)]
pub(crate) enum NameKind {
    Module,
    ExternModule,
    ExternPort,
    Port,
    Register,
    Wire,
    Let,
    Instance,
    Const,
    EnumVariant,
    InstanceOutput,
}

impl NameKind {
    fn label(self) -> String {
        match self {
            NameKind::Module => lstr!(en: "module name"; tr: "modül adı"),
            NameKind::ExternModule => lstr!(en: "extern module name"; tr: "extern modül adı"),
            NameKind::ExternPort => lstr!(en: "extern module port"; tr: "extern modül portu"),
            NameKind::Port => lstr!(en: "port"; tr: "port"),
            NameKind::Register => lstr!(en: "register"; tr: "register"),
            NameKind::Wire => lstr!(en: "wire"; tr: "wire"),
            NameKind::Let => lstr!(en: "let wire"; tr: "let teli"),
            NameKind::Instance => lstr!(en: "instance name"; tr: "örnek adı"),
            NameKind::Const => lstr!(en: "constant"; tr: "sabit"),
            NameKind::EnumVariant => lstr!(en: "enum localparam"; tr: "enum localparam'ı"),
            NameKind::InstanceOutput => {
                lstr!(en: "instance output wire"; tr: "örnek çıkış teli")
            }
        }
    }
}

impl Emitter<'_> {
    /// Kaynakta `span`'in kapsadığı metin (birleşik adın kullanıcı parçası).
    fn written_at(&self, span: Span) -> Option<&str> {
        let src = self.sources.iter().find(|s| s.file == span.file)?;
        src.text.get(span.start as usize..span.end as usize)
    }

    /// `name` bir SV anahtar sözcüğüyse E1013. `span` adın kaynaktaki
    /// yeri; SV adı kaynakta yazılandan uzunsa (`pulsestyle` →
    /// `pulsestyle_ondetect`) ileti birleşimi adlandırır.
    pub(crate) fn check_sv_name(&mut self, name: &str, span: Span, kind: NameKind) {
        if !is_sv_keyword(name) {
            return;
        }
        self.sv_name_reported.insert(name.to_string());
        let written = self.written_at(span).filter(|w| *w != name);
        let (message, help) = match written {
            None => (
                lstr!(
                    en: "'{name}' is a SystemVerilog keyword; the generated SystemVerilog would not compile";
                    tr: "'{name}' bir SystemVerilog anahtar sözcüğü; üretilen SystemVerilog derlenmez"
                ),
                lstr!(
                    en: "rename it, for example '{name}_' (Volt keeps names verbatim in SystemVerilog; see volt explain E1013)";
                    tr: "yeniden adlandırın, örneğin '{name}_' (Volt adları SystemVerilog'da aynen korur; bkz. volt explain E1013)"
                ),
            ),
            Some(w) => (
                lstr!(
                    en: "'{w}' becomes the SystemVerilog name '{name}', which is a keyword; the generated SystemVerilog would not compile";
                    tr: "'{w}' SystemVerilog'da '{name}' adını alıyor ve bu bir anahtar sözcük; üretilen SystemVerilog derlenmez"
                ),
                lstr!(
                    en: "rename '{w}' or the name joined to it with '_' (struct field, enum variant or port; see volt explain E1013)";
                    tr: "'{w}' adını ya da ona '_' ile eklenen adı (struct alanı, enum varyantı ya da port) değiştirin (bkz. volt explain E1013)"
                ),
            ),
        };
        let diag = Diagnostic::error(
            ErrorCode::E1013,
            message,
            LabeledSpan::primary(span, kind.label()),
            help,
        );
        if !self.diagnostics.contains(&diag) {
            self.diagnostics.push(diag);
        }
    }

    /// Birim düzeyindeki adlar: modüller (ve [`Self::audit_module_names`]),
    /// extern modüller ve portları, sabitler, enum localparam'ları
    /// (`<Enum>_<Varyant>`). Enum'un bütün varyantları denetlenir,
    /// kullanılmayanlar da: localparam yalnız kullanılan varyant için
    /// üretilir, ama ad bildirimde belirlenir ve düzeltilecek yer oradadır
    /// (sabitlerle aynı tek kural, ADR-0078).
    pub(crate) fn audit_unit_names(&mut self) {
        let ast = self.ast;
        for &item in &ast.items {
            match &ast.items_arena[item].kind {
                ItemKind::Module(m) => {
                    self.check_sv_name(&m.name.text, m.name.span, NameKind::Module);
                    self.audit_module_names(m);
                }
                ItemKind::Extern(e) => {
                    self.check_sv_name(&e.name.text, e.name.span, NameKind::ExternModule);
                    for p in &e.ports {
                        self.check_sv_name(&p.name.text, p.name.span, NameKind::ExternPort);
                    }
                }
                ItemKind::Const(c) => {
                    self.check_sv_name(&c.name.text, c.name.span, NameKind::Const)
                }
                ItemKind::Enum(e) => {
                    for v in &e.variants {
                        let name = format!("{}_{}", e.name.text, v.name.text);
                        self.check_sv_name(&name, v.name.span, NameKind::EnumVariant);
                    }
                }
                _ => {}
            }
        }
    }

    /// Modülün SV adları: portlar (struct yaprakları ve bundle alanları
    /// açılmış hâlde) ve gövde bildirimleri.
    pub(crate) fn audit_module_names(&mut self, module: &ModuleDecl) {
        let ast = self.ast;
        for p in &module.ports {
            // Düzleştirilmiş bundle alanının span'i yapaydır (benzersiz
            // bildirim anahtarı); kullanıcının yazdığı port `bundle`'dadır.
            let span = p.bundle.as_ref().map_or(p.name.span, |b| b.port.span);
            self.check_sv_name(&p.name.text, span, NameKind::Port);
        }
        for &stmt in &module.body {
            let (name, kind) = match &ast.stmts[stmt].kind {
                StmtKind::Reg(r) => (&r.name, NameKind::Register),
                StmtKind::Wire(w) => (&w.name, NameKind::Wire),
                StmtKind::Let(l) => (&l.name, NameKind::Let),
                // Yerleşik primitifin (ADR-0027) adı SV'de yalnız önektir
                // (`buf_mem`); o adlar güvenlik ağından geçer.
                StmtKind::Instance(i) if crate::instance::user_instance_target(i).is_some() => {
                    (&i.name, NameKind::Instance)
                }
                _ => continue,
            };
            self.check_sv_name(&name.text, name.span, kind);
        }
    }

    /// Güvenlik ağı: üretilen modül metninde emitter sözdizimi olmayan
    /// anahtar sözcük belirteci E1013'tür (span: modül adı).
    pub(crate) fn audit_emitted_text(&mut self, module: &ModuleDecl, sv: &str) {
        let mut seen = HashSet::new();
        for token in sv_identifiers(sv) {
            if is_sv_keyword(token)
                && EMITTER_KEYWORDS.binary_search(&token).is_err()
                && !self.sv_name_reported.contains(token)
                && seen.insert(token)
            {
                let m = &module.name.text;
                self.sv_name_reported.insert(token.to_string());
                self.diagnostics.push(Diagnostic::error(
                    ErrorCode::E1013,
                    lstr!(
                        en: "the SystemVerilog generated for module '{m}' uses the keyword '{token}' as a name";
                        tr: "'{m}' modülü için üretilen SystemVerilog '{token}' anahtar sözcüğünü ad olarak kullanıyor"
                    ),
                    LabeledSpan::primary(
                        module.name.span,
                        lstr!(en: "in this module"; tr: "bu modülde"),
                    ),
                    lstr!(
                        en: "rename the Volt names '{token}' is built from (see volt explain E1013)";
                        tr: "'{token}' adının kurulduğu Volt adlarını değiştirin (bkz. volt explain E1013)"
                    ),
                ));
            }
        }
    }
}

/// SV metnindeki tanımlayıcı-biçimli belirteçler: yorumlar, dizeler,
/// `` `yönerge``, `$sistem` adları ve tabanlı sayıların (`8'hFF`) harfleri
/// atlanır.
pub(crate) fn sv_identifiers(sv: &str) -> impl Iterator<Item = &str> {
    let bytes = sv.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    std::iter::from_fn(move || {
        while i < n {
            let c = bytes[i];
            // Bayt düzeyinde: ASCII dışı bir baytta `&sv[i..]` karakter
            // sınırı dışında dilimleyip panik verirdi.
            let rest = &bytes[i..];
            if rest.starts_with(b"//") {
                i = rest.iter().position(|&b| b == b'\n').map_or(n, |j| i + j);
            } else if rest.starts_with(b"/*") {
                i = rest[2..]
                    .windows(2)
                    .position(|w| w == b"*/")
                    .map_or(n, |j| i + 2 + j + 2);
            } else if c == b'"' {
                i += 1;
                while i < n && bytes[i] != b'"' {
                    i += if bytes[i] == b'\\' { 2 } else { 1 };
                }
                i += 1;
            } else if c == b'\'' {
                i += 1;
                if i < n && matches!(bytes[i], b's' | b'S') {
                    i += 1;
                }
                if i < n
                    && matches!(
                        bytes[i],
                        b'b' | b'B' | b'o' | b'O' | b'd' | b'D' | b'h' | b'H'
                    )
                {
                    i += 1;
                    while i < n
                        && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'_' | b'?'))
                    {
                        i += 1;
                    }
                }
            } else if c == b'`' || c == b'$' || c.is_ascii_digit() {
                i += 1;
                while i < n && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
            } else if c.is_ascii_alphabetic() || c == b'_' {
                let start = i;
                while i < n && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'_' | b'$'))
                {
                    i += 1;
                }
                return Some(&sv[start..i]);
            } else {
                i += 1;
            }
        }
        None
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emitter_keywords_are_sorted_sv_keywords() {
        assert!(EMITTER_KEYWORDS.windows(2).all(|w| w[0] < w[1]));
        for w in EMITTER_KEYWORDS {
            assert!(is_sv_keyword(w), "{w}");
        }
    }

    #[test]
    fn identifiers_skip_comments_strings_numbers_and_system_names() {
        let sv = "// packed\n/* table */ assign y = 8'hdeadbeef + $signed(a_b) + `define_x;\n\
                  $display(\"time %d\", 4'sd5); logic [7:0] packed_q;";
        let toks: Vec<&str> = sv_identifiers(sv).collect();
        assert_eq!(toks, ["assign", "y", "a_b", "logic", "packed_q"]);
    }

    #[test]
    fn non_ascii_text_does_not_split_a_char() {
        let sv = "// ölçü — yorum\nlogic ç_x; /* şş */ wire ğ table \"ı\" y";
        let toks: Vec<&str> = sv_identifiers(sv).collect();
        assert_eq!(toks, ["logic", "_x", "wire", "table", "y"]);
    }

    #[test]
    fn identifiers_keep_generated_keyword_names() {
        let sv = "    input  logic [7:0] pulsestyle_ondetect,\n    output logic y";
        let kws: Vec<&str> = sv_identifiers(sv)
            .filter(|t| is_sv_keyword(t) && EMITTER_KEYWORDS.binary_search(t).is_err())
            .collect();
        assert_eq!(kws, ["pulsestyle_ondetect"]);
    }
}
