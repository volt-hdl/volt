//! Ayrıştırıcı katmanının kapsam zinciri (name-resolution.md §2, §4, §6;
//! ADR-0081 eki "Ayrıştırıcının ikamesi kapsam zincirinden geçer", #130).
//!
//! Generic monomorfizasyonu (ADR-0041) ve modül düzeyi `for` açılımı
//! (ADR-0056) ad çözümünden (HIR) önce koşar ve gövdedeki adları
//! değiştirir: generic parametre ile döngü değişkeni literale, döngü
//! gövdesinin `let`'i yineleme sonekli ada. Hangi yolun değişeceğine bu
//! zincir karar verir; tek parçalı her ad [`Scopes::resolve`]'dan geçer.
//! Sıra şartnamenin ve HIR çözücüsünün (`volt-hir/src/resolve`) sırasıdır,
//! üreticinin `Emitter::resolve`'u da aynı sırayı izler:
//!
//! 1. içten dışa kapsamlar: blok (`on`/`comb`/`if`/`else`/kol gövdesi),
//!    döngü değişkeni, `match` kolu, modül, kök;
//! 2. kapsam içinde bildirim noktasından sonrası görünür (§4, ADR-0083
//!    Karar 8); aynı kapsamda ikinci bildirim bağlanmaz (E1003 çözücünün);
//! 3. görünür bağ yoksa ad, içinde bulunulan sıralı gövdede DAHA AŞAĞIDA
//!    bildirilmiş olabilir (`pending`): çözücü orada E1002 verir, bu
//!    yüzden açılım adı yine yineleme adına çevirir ki E1002 o ada düşsün.
//!
//! Zincir aynı zamanda gölgelemeyi bilir: bir bildirim dış kapsamdaki bir
//! adı örttüğünde [`Scopes::declare`] örtülen bağı döndürür. Çözücü
//! (HIR) gölgelemenin iki ucunu da gördüğünde W1002'yi kendisi verir;
//! uçlardan biri açılımda kaybolursa (döngü değişkeni, yeniden adlanan
//! gövde `let`'i) W1002'yi açılım verir ([`shadowing_warning`]).

use std::collections::HashMap;

use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

/// Tek parçalı bir adın ayrıştırıcı katmanındaki bağı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Binding {
    /// Const generic parametre (ADR-0041): yolu literal olur. Desende
    /// değer desenidir (§5.1).
    Generic(i128),
    /// Açılan modül düzeyi `for`'un değişkeni (ADR-0056): yolu literal
    /// olur. Desende E1015 (§5.1, döngü değişkeni sabit değildir).
    LoopVar(i128),
    /// Açılan `for` gövdesinin `let`'i: yolu yineleme sonekli ad olur.
    Renamed(String),
    /// Klonlanan ağacın içinde bildirilmiş ad (blok `let`'i, `comb` içi
    /// döngü değişkeni, desen bağı, iç içe modül düzeyi `for`'un
    /// değişkeni): dokunulmaz. `loop_var`: desende E1015 gerektiren bir
    /// döngü değişkeni mi.
    Inner { loop_var: bool },
    /// Klonun dışında bildirilmiş ad (kök öğe, port, modül sinyali,
    /// klonlanmış modülün generic parametresi): dokunulmaz, çözücü bağlar.
    Outer,
}

/// Bir bildirim: bağı, kaynaktaki adın span'i ve çözücünün (HIR) bu
/// bildirimi görüp görmediği. Görmediği bildirimler açılımda kaybolur:
/// modül düzeyi `for`'un değişkeni ve yeniden adlanan gövde `let`'i.
#[derive(Debug, Clone)]
pub(crate) struct Entry {
    pub binding: Binding,
    pub span: Span,
    pub resolver_sees: bool,
}

#[derive(Debug, Clone, Default)]
struct Frame {
    names: HashMap<String, Entry>,
    /// Bu kapsamın sıralı gövdesinde daha aşağıda bildirilecek adlar.
    pending: HashMap<String, Entry>,
}

/// İçten dışa kapsam yığını.
#[derive(Debug, Clone, Default)]
pub(crate) struct Scopes {
    frames: Vec<Frame>,
}

impl Scopes {
    pub(crate) fn push(&mut self) {
        self.frames.push(Frame::default());
    }

    pub(crate) fn pop(&mut self) {
        self.frames.pop();
    }

    /// En içteki kapsamın sıralı gövdesinde ileride bildirilecek ad.
    pub(crate) fn expect(&mut self, name: &str, entry: Entry) {
        if let Some(top) = self.frames.last_mut() {
            top.pending.entry(name.to_string()).or_insert(entry);
        }
    }

    /// Adı en içteki kapsama bağlar. Aynı kapsamda zaten varsa bağlamaz
    /// (çözücünün E1003'ü; ilk bildirim bağlı kalır, çözücüde de öyle).
    /// Dönüş: bildirimin örttüğü dış bağ (gölgeleme, §6).
    pub(crate) fn declare(&mut self, name: &str, entry: Entry) -> Option<Entry> {
        let shadowed = self.lookup_outer(name).cloned();
        let top = self.frames.last_mut()?;
        top.pending.remove(name);
        if top.names.contains_key(name) {
            return None;
        }
        top.names.insert(name.to_string(), entry);
        shadowed
    }

    /// Adın bağı: önce görünür bildirimler içten dışa, yoksa sıralı
    /// gövdede ileride bildirilecek olan (çözücüde E1002).
    pub(crate) fn resolve(&self, name: &str) -> Option<&Entry> {
        self.frames
            .iter()
            .rev()
            .find_map(|f| f.names.get(name))
            .or_else(|| self.frames.iter().rev().find_map(|f| f.pending.get(name)))
    }

    /// Ad kök kapsamın (en dıştaki kapsam) içinde bir bildirime bağlı mı?
    /// Öyleyse kök const'u gölgeler: parser katmanının sabit
    /// değerlendirmesi onu const saymaz.
    pub(crate) fn bound_above_root(&self, name: &str) -> bool {
        self.frames
            .iter()
            .skip(1)
            .any(|f| f.names.contains_key(name))
    }

    /// En içteki kapsamın dışında görünür bildirim.
    fn lookup_outer(&self, name: &str) -> Option<&Entry> {
        let n = self.frames.len();
        self.frames[..n.saturating_sub(1)]
            .iter()
            .rev()
            .find_map(|f| f.names.get(name))
    }
}

/// W1002 — iç kapsamdaki bildirim dış kapsamdaki aynı adı gölgeliyor
/// (name-resolution.md §6). Çözücü (HIR) ile açılım aynı metni kullanır.
pub fn shadowing_warning(shown: &str, span: Span, outer: Span) -> Diagnostic {
    let mut d = Diagnostic::warning(
        ErrorCode::W1002,
        lstr!(en: "'{}' shadows a definition in an outer scope", shown;
              tr: "'{}' dış kapsamdaki tanımı gölgeliyor", shown),
        LabeledSpan::primary(
            span,
            lstr!(en: "inner definition here"; tr: "iç tanım burada"),
        ),
        lstr!(en: "use a different name to avoid confusion";
              tr: "karışıklığı önlemek için farklı bir isim kullanın"),
    );
    if outer.file.0 != u32::MAX {
        d = d.with_secondary(
            outer,
            lstr!(en: "shadowed definition here"; tr: "gölgelenen tanım burada"),
        );
    }
    d
}

/// E1015 — desendeki ad modül düzeyi `for`'un döngü değişkeni (§5.1).
/// Çözücü döngü değişkenini görmez (açılım onu literale çevirir), bu
/// yüzden tanıyı açılım verir; metin çözücünün E1015'iyle aynıdır.
pub(crate) fn loop_var_pattern_error(text: &str, span: Span, decl: Span) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E1015,
        lstr!(en: "'{text}' in a pattern must be a constant, but it is a loop variable";
              tr: "desendeki '{text}' bir sabit olmalı, ama döngü değişkeni"),
        LabeledSpan::primary(
            span,
            lstr!(en: "a pattern compares with a constant value";
                  tr: "desen sabit bir değerle karşılaştırır"),
        ),
        lstr!(en: "compare explicitly: 'if <value> == {text} {{ ... }}'; or match a 'const' / a literal";
              tr: "açıkça karşılaştırın: 'if <değer> == {text} {{ ... }}'; ya da bir 'const' / literal ile eşleyin"),
    )
    .with_note(
        NoteKind::Note,
        lstr!(en: "Volt has no binding patterns: a name in a pattern is a value, never a new variable";
              tr: "Volt'ta bağlama deseni yok: desendeki ad bir değerdir, asla yeni bir değişken değil"),
    )
    .with_secondary(
        decl,
        lstr!(en: "'{text}' is declared here"; tr: "'{text}' burada bildiriliyor"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(binding: Binding) -> Entry {
        Entry {
            binding,
            span: Span::new(volt_span::FileId(0), 0, 1),
            resolver_sees: true,
        }
    }

    #[test]
    fn the_innermost_visible_declaration_wins() {
        let mut s = Scopes::default();
        s.push();
        s.declare("n", entry(Binding::Generic(3)));
        s.push();
        assert_eq!(
            s.resolve("n").map(|e| &e.binding),
            Some(&Binding::Generic(3))
        );
        let shadowed = s.declare("n", entry(Binding::Inner { loop_var: false }));
        assert_eq!(shadowed.map(|e| e.binding), Some(Binding::Generic(3)));
        assert_eq!(
            s.resolve("n").map(|e| &e.binding),
            Some(&Binding::Inner { loop_var: false })
        );
        s.pop();
        assert_eq!(
            s.resolve("n").map(|e| &e.binding),
            Some(&Binding::Generic(3))
        );
    }

    #[test]
    fn a_second_declaration_in_the_same_scope_keeps_the_first() {
        let mut s = Scopes::default();
        s.push();
        assert!(s.declare("n", entry(Binding::Generic(3))).is_none());
        assert!(s.declare("n", entry(Binding::Outer)).is_none());
        assert_eq!(
            s.resolve("n").map(|e| &e.binding),
            Some(&Binding::Generic(3))
        );
    }

    #[test]
    fn a_later_declaration_is_found_only_when_nothing_is_visible() {
        let mut s = Scopes::default();
        s.push();
        s.declare("i", entry(Binding::LoopVar(0)));
        s.push();
        s.expect("i", entry(Binding::Renamed("i_0".into())));
        s.expect("a", entry(Binding::Renamed("a_0".into())));
        // Görünür döngü değişkeni, ileride bildirilecek `let i`'den önce gelir.
        assert_eq!(
            s.resolve("i").map(|e| &e.binding),
            Some(&Binding::LoopVar(0))
        );
        assert_eq!(
            s.resolve("a").map(|e| &e.binding),
            Some(&Binding::Renamed("a_0".into()))
        );
        s.declare("i", entry(Binding::Renamed("i_0".into())));
        assert_eq!(
            s.resolve("i").map(|e| &e.binding),
            Some(&Binding::Renamed("i_0".into()))
        );
        assert!(s.resolve("b").is_none());
    }
}
