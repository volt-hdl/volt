//! Ağaç derinliği sınırı (ADR-0080).
//!
//! Parser'dan sonraki her geçit (mono klonlayıcı, çözümleme, tip
//! denetimi, saat alanı, SV/SVA üretimi, LSP) AST'yi özyinelemeyle
//! yürür. Yığın taşması Rust'ta panik değil süreç sonudur (abort) —
//! LSP sunucusunu da öldürür. Sınır tek noktada konur: parser
//! [`MAX_DEPTH`]'ten derin ağaç ÜRETMEZ, sonraki geçitler kendi
//! korumalarına ihtiyaç duymaz.
//!
//! Sayaç özyinelemeyi değil ağaç yüksekliğini izler: sol-derin operatör
//! zinciri (`a + a + …`), postfix zinciri (`x[0][0]…`, `a as u8 as u8`)
//! ve `else if` zinciri parser'da döngüdür ama ağaçta her halka bir
//! kattır; hepsi aynı sayaca yazılır. Sınırda zincirin ya da grubun
//! kalanı TÜKETİLİR (tek E0018, kaskad yok) ve ağaca girmez.

use volt_ast::Name;
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use crate::token::TokenKind::{self, *};

use super::recovery::set_contains;
use super::Parser;

/// Atlamanın durduğu, iç içe grup dışındaki belirsizliksiz deyim ve öğe
/// başlangıçları (`if`/`match`/ad ifade de başlatır, durak değildir).
const SKIP_STOP: &[TokenKind] = &[
    KwReg, KwLet, KwWire, KwOn, KwComb, KwFor, KwIn, KwOut, KwInout, KwModule, KwPipeline,
    KwDomain, KwFn, KwStruct, KwEnum, KwConst, KwType, KwExtern, KwPub, KwUse, KwPackage,
];

/// Ağaç yüksekliği sınırı. Gerçek tasarımların en derini (ölçüm,
/// ADR-0080 §1) bunun çok altındadır; yığın bütçesi
/// [`crate::stack::COMPILER_STACK_SIZE`] bu değere göre seçilmiştir.
pub const MAX_DEPTH: u32 = 256;

impl Parser<'_> {
    /// Ağaçta bir kat iner. Sınırdaysa E0018 üretir ve `false` döner
    /// (sayaç değişmez). Çağıran girişteki derinliği saklayıp geri yükler.
    pub(crate) fn descend(&mut self, at: Span) -> bool {
        if self.depth >= MAX_DEPTH {
            self.err_too_deep(at);
            return false;
        }
        self.depth += 1;
        true
    }

    /// Sınırdaki alt ağacı atlar. Açılış ayracındaysa eşleşen kapanışa
    /// kadar (dahil) dengeli grubu; değilse (`if`, `~`, ad…) kapsayan
    /// grubun kapanışına ya da açık bir deyim/öğe başlangıcına kadar
    /// (tüketmeden) — ağaç orada zaten sınırda, kalanı tek parça atlanır ki
    /// kaskad tanı çıkmasın. Yinelemeli: grup ne kadar derin olursa olsun
    /// yığın kullanmaz.
    pub(crate) fn skip_nested_group(&mut self) -> Span {
        let start = self.pos;
        let single_group = matches!(self.current(), Some(LParen | LBracket | LBrace));
        let mut open = 0usize;
        while let Some(kind) = self.current() {
            match kind {
                LParen | LBracket | LBrace => open += 1,
                RParen | RBracket | RBrace | Semi if open == 0 => break,
                RParen | RBracket | RBrace => open -= 1,
                _ if open == 0 && set_contains(SKIP_STOP, kind) => break,
                _ => {}
            }
            self.bump_any();
            if single_group && open == 0 {
                break;
            }
        }
        if !single_group {
            self.depth_skip_end = Some(self.pos);
        }
        self.span_from(start)
    }

    /// Derinlik atlaması kapsayan grubun kapanışında durdu mu? O zaman
    /// başlığı atlanan yapının (`match x {`, `if c {`) gövdesi de atlandı:
    /// çağıran `{` beklememeli ve kapsayan grubun `}`'ını TÜKETMEMELİ —
    /// aksi hâlde dış katmanların ayraçları kayar (kaskad tanı).
    pub(crate) fn cut_by_depth(&self) -> bool {
        self.depth_skip_end == Some(self.pos)
    }

    fn err_too_deep(&mut self, at: Span) {
        // Aynı derin yapının iç katlarında tekrar raporlanmaz.
        if self.depth_reported {
            return;
        }
        self.depth_reported = true;
        self.diagnostics.push(
            Diagnostic::error(
                ErrorCode::E0018,
                lstr!(en: "nesting is too deep: more than {MAX_DEPTH} levels";
                      tr: "iç içelik çok derin: {MAX_DEPTH} kattan fazla"),
                LabeledSpan::primary(
                    at,
                    lstr!(en: "the limit is reached here; the rest is skipped";
                          tr: "sınıra burada ulaşıldı; kalanı atlandı"),
                ),
                lstr!(en: "split the expression with intermediate 'let' bindings, or the nested blocks into separate modules";
                      tr: "ifadeyi ara 'let' bağlamalarıyla, iç içe blokları ayrı modüllere bölün"),
            )
            .with_note(
                NoteKind::Reason,
                lstr!(en: "operator chains, '.'/'[]'/'as' chains and 'else if' chains count one level per link";
                      tr: "operatör zincirleri, '.'/'[]'/'as' zincirleri ve 'else if' zincirleri halka başına bir kat sayılır"),
            ),
        );
    }
}

/// E0018 — tip, takma ad / alan / varyant zinciri boyunca açıldığında
/// sınırdan derin (`type_graph`, ADR-0080).
pub(crate) fn err_type_too_deep(name: &Name) -> Diagnostic {
    let n = name.text.as_str();
    Diagnostic::error(
        ErrorCode::E0018,
        lstr!(en: "type '{n}' is nested too deep: more than {MAX_DEPTH} levels once its aliases, fields and variants are expanded";
              tr: "'{n}' tipi çok derin: takma adları, alanları ve varyantları açıldığında {MAX_DEPTH} kattan fazla"),
        LabeledSpan::primary(
            name.span,
            lstr!(en: "the chain of type declarations exceeds the limit here";
                  tr: "tip bildirimi zinciri sınırı burada aşıyor"),
        ),
        lstr!(en: "shorten the chain: refer to the underlying type directly instead of through another alias or wrapper";
              tr: "zinciri kısaltın: alttaki tipe başka bir takma ad ya da sarmalayıcı yerine doğrudan başvurun"),
    )
}
