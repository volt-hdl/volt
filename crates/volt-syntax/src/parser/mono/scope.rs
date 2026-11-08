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
//!
//! Aynı zincir ayrıştırıcının ad eşleyen öteki adımlarında da kullanılır
//! (#136, #137, #138; ADR-0081 eki "Ayrıştırıcının öteki adımları da
//! kapsam zincirinden geçer"): bundle düzleştirmesi, çift yönlü port
//! açılımı, `@mmio` tutamacı, pipeline aşama değerleri ve otomatik
//! kontrat çıkarımı gövdeyi gezerken blok `let`'ini ve döngü değişkenini
//! [`Scopes::declare_local`] ile bildirir, bir tabanı yeniden yazmadan
//! önce [`Scopes::is_local`] ile sorar; ayrıştırıcının sabit
//! değerlendirmesi kök const'u yalnız [`Scopes::bound_above_root`] yanlışsa
//! kabul eder.

use std::collections::HashMap;

use volt_ast::visibility::{item_name, RootFilter};
use volt_ast::{Idx, Item, ItemKind, Name, SourceFile};
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
    /// Bir modül gövdesinin kapsam zinciri: kök kapsam ve modül kapsamı
    /// (monomorfun generic parametreleri, portlar). Modül gövdesinin
    /// bildirimlerini gezen geçit sırayla ekler (§4, §5).
    ///
    /// Kök kapsam, modülün dosyasından görünen öğelerdir (§3.3, #139):
    /// kendi dosyasının öğeleri ve `use` ile getirilenler (takma adlılar
    /// yerel adlarıyla). Başka dosyanın getirilmemiş öğesi kökte yoktur:
    /// onu ne gölgeleme (W1002) ne de sabit değerlendirmesi görür.
    pub(crate) fn module(ast: &SourceFile, item: Idx<Item>) -> Scopes {
        let mut chain = Scopes::default();
        chain.push();
        let file = ast.items_arena[item].span.file;
        let filter = RootFilter::new(ast.file_scopes.as_ref());
        for &i in &ast.items {
            let it = &ast.items_arena[i];
            let Some(name) = item_name(&it.kind) else {
                continue;
            };
            if filter.sees(file, &name.text, it.span.file) {
                chain.declare_outer(name);
            }
        }
        let imported = ast.file_scopes.as_ref().and_then(|s| s.get(&file));
        for (local, (_, item, name)) in imported.map(|s| &s.origin).into_iter().flatten() {
            if local != item {
                chain.declare_outer(name);
            }
        }
        chain.push();
        if let ItemKind::Module(m) = &ast.items_arena[item].kind {
            if let Some(params) = ast.generate.mono_generics.get(&m.name.span) {
                for p in params {
                    chain.declare_outer(p);
                }
            }
            for p in &m.ports {
                chain.declare_outer(&p.name);
            }
        }
        chain
    }

    /// Çözücünün de gördüğü bir bildirim (öğe, port, modül sinyali).
    pub(crate) fn declare_outer(&mut self, name: &Name) {
        self.declare(
            &name.text,
            Entry {
                binding: Binding::Outer,
                span: name.span,
                resolver_sees: true,
            },
        );
    }

    /// Açılımda kaybolan modül kapsamı adı: bundle portu (düz portlara
    /// açılır) ya da `@mmio` tutamacı `regs`. Çözücü onu görmez; onu
    /// gölgeleyen yerelin W1002'sini gezen geçit verir.
    pub(crate) fn declare_vanishing(&mut self, name: &Name) {
        self.declare(
            &name.text,
            Entry {
                binding: Binding::Outer,
                span: name.span,
                resolver_sees: false,
            },
        );
    }

    /// Gezilen gövdenin yereli: blok `let`'i ya da döngü değişkeni. Dönüş:
    /// örttüğü dış bağ (gölgeleme, §6).
    pub(crate) fn declare_local(&mut self, name: &Name, loop_var: bool) -> Option<Entry> {
        self.declare(
            &name.text,
            Entry {
                binding: Binding::Inner { loop_var },
                span: name.span,
                resolver_sees: true,
            },
        )
    }

    /// Ad, gezilen gövdenin bir yereline (blok `let`'i, döngü değişkeni)
    /// mı bağlı? Öyleyse dıştaki port, tutamaç ya da aşama değeri
    /// gölgelenmiştir ve kullanım yeniden yazılmaz (§2, §6).
    pub(crate) fn is_local(&self, name: &str) -> bool {
        matches!(
            self.resolve(name).map(|e| &e.binding),
            Some(Binding::Inner { .. })
        )
    }

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

/// Sabit konumundaki ifadede (modül düzeyi `for` sınırı, bundle dizisi
/// uzunluğu ya da indeksi) birimin başka bir dosyasında bildirilmiş ama
/// buradan görünmeyen ilk yalın ad: zincirde (yerel, port, modül
/// bildirimi, ileride bildirilecek ad, modülün dosyasından görünen kök
/// öğe) bağı yok. Ayrıştırıcı bu konumları açarken adı yok eder; çözücü
/// onu göremeyeceğinden E1001'i (name-resolution.md §3.3) açılım verir
/// ([`invisible_name_error`], #139). Hiç tanımlanmamış ad burada değil:
/// onun tanısı ADR-0072'nin E2021/E2008'i kalır (#122).
pub(crate) fn invisible_name<'a>(
    ast: &'a SourceFile,
    chain: &Scopes,
    e: Idx<volt_ast::Expr>,
) -> Option<&'a Name> {
    use volt_ast::{ExprKind, UnOp};
    match &ast.exprs[e].kind {
        ExprKind::Path(p) if p.segments.len() == 1 => {
            let name = &p.segments[0];
            (chain.resolve(&name.text).is_none() && declared_elsewhere(ast, name).is_some())
                .then_some(name)
        }
        ExprKind::Binary { lhs, rhs, .. } => {
            invisible_name(ast, chain, *lhs).or_else(|| invisible_name(ast, chain, *rhs))
        }
        ExprKind::Unary {
            op: UnOp::Neg,
            operand,
        } => invisible_name(ast, chain, *operand),
        _ => None,
    }
}

/// `name`in yazıldığı dosyadan başka bir dosyada aynı adla bildirilmiş
/// kök öğe.
fn declared_elsewhere<'a>(ast: &'a SourceFile, name: &Name) -> Option<&'a Item> {
    ast.items.iter().map(|&i| &ast.items_arena[i]).find(|it| {
        it.span.file != name.span.file && item_name(&it.kind).is_some_and(|n| n.text == name.text)
    })
}

/// E1001 — başka dosyanın görünmeyen öğesi (name-resolution.md §3.3);
/// başlık ve etiket çözücünün E1001'iyle aynı. Yardım öğeyi görünür
/// kılmayı söyler: başka dosyanın öğesi yalnız `use` ile getirilirse
/// görünür, getirilebilmesi için `pub` olmalıdır.
pub(crate) fn invisible_name_error(ast: &SourceFile, name: &Name) -> Diagnostic {
    let text = &name.text;
    let help = match declared_elsewhere(ast, name).map(|it| it.visibility) {
        Some(volt_ast::Visibility::Public) => lstr!(
            en: "'{text}' is declared in another file of this design: import it with 'use <package>::{text};'";
            tr: "'{text}' bu tasarımın başka bir dosyasında bildirilmiş: 'use <paket>::{text};' ile içe aktarın"
        ),
        _ => lstr!(
            en: "'{text}' is private to another file of this design: mark it 'pub' there and import it with 'use <package>::{text};'";
            tr: "'{text}' bu tasarımın başka bir dosyasına özel: orada 'pub' yapın ve 'use <paket>::{text};' ile içe aktarın"
        ),
    };
    Diagnostic::error(
        ErrorCode::E1001,
        lstr!(en: "undefined name: '{text}'"; tr: "tanımsız isim: '{text}'"),
        LabeledSpan::primary(
            name.span,
            lstr!(en: "this name could not be resolved"; tr: "bu isim çözülemedi"),
        ),
        help,
    )
}

impl crate::parser::Parser<'_> {
    /// Gezilen gövdenin yerelini (blok `let`'i, döngü değişkeni) birim sonu
    /// geçidinin zincirine ([`Parser::scopes`]) bildirir. Yerel, çözücünün
    /// görmediği bir adı (bundle portu, `@mmio` tutamacı) gölgeliyorsa
    /// W1002'yi geçit verir (§6). Generic modülün monomorflarındaki
    /// kopyalar yalnız `Span.ctx`'leriyle ayrışır; sürücü onları
    /// `volt_hir::annotate_generate`'te katlar (ADR-0068), kaynak satırı
    /// başına bir uyarı kalır.
    pub(crate) fn declare_block_local(&mut self, name: &Name, loop_var: bool) {
        let Some(outer) = self.scopes.declare_local(name, loop_var) else {
            return;
        };
        if !outer.resolver_sees {
            self.diagnostics
                .push(shadowing_warning(&name.text, name.span, outer.span));
        }
    }
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

    fn name(text: &str, start: u32) -> Name {
        Name {
            text: text.into(),
            span: Span::new(volt_span::FileId(0), start, start + 1),
        }
    }

    /// Yeniden yazan adımların sorusu (#137): ad bir yerele mi bağlı? Port
    /// ya da tutamaç yerel değildir; blok `let`'i ve döngü değişkeni
    /// yereldir ve yalnız kendi kapsamında.
    #[test]
    fn only_a_block_let_or_a_loop_variable_is_local() {
        let mut s = Scopes::default();
        s.push();
        s.declare_outer(&name("aw", 0));
        s.declare_vanishing(&name("regs", 1));
        assert!(!s.is_local("aw"));
        assert!(!s.is_local("regs"));
        assert!(!s.is_local("unknown"));
        s.push();
        assert!(s.declare_local(&name("x", 2), false).is_none());
        let shadowed = s.declare_local(&name("aw", 3), false);
        assert_eq!(shadowed.map(|e| e.binding), Some(Binding::Outer));
        assert!(s.is_local("aw"));
        assert!(s.is_local("x"));
        s.push();
        s.declare_local(&name("regs", 4), true);
        assert!(s.is_local("regs"));
        s.pop();
        assert!(!s.is_local("regs"));
        s.pop();
        assert!(!s.is_local("aw"));
        assert!(!s.is_local("x"));
    }

    /// Kaybolan adı gölgeleyen yerel, çözücünün görmediği bağı döndürür
    /// (W1002'yi adım verir); görünen adı gölgeleyen yerel görüleni.
    #[test]
    fn a_local_reports_whether_the_resolver_sees_what_it_shadows() {
        let mut s = Scopes::default();
        s.push();
        s.declare_outer(&name("p", 0));
        s.declare_vanishing(&name("regs", 1));
        s.push();
        let seen = s.declare_local(&name("p", 2), false).expect("gölgeler");
        assert!(seen.resolver_sees);
        let vanished = s.declare_local(&name("regs", 3), false).expect("gölgeler");
        assert!(!vanished.resolver_sees);
        assert_eq!(vanished.span, name("regs", 1).span);
    }

    /// Modül zinciri: kök kapsamda birimin öğeleri, modül kapsamında
    /// portlar; port kök const'u gölgeler (`bound_above_root`), öğe
    /// gölgelemez (#136).
    #[test]
    fn the_module_chain_puts_items_at_the_root_and_ports_above_it() {
        let src = "const K : u32 = 1
const J : u32 = 2
module M {
    in  K : u8
    out y : u8
    y = K
}
";
        let parsed = crate::parse(volt_span::FileId(0), src);
        let ast = &parsed.ast;
        let item = ast
            .items
            .iter()
            .copied()
            .find(|&i| matches!(ast.items_arena[i].kind, ItemKind::Module(_)))
            .expect("modül");
        let s = Scopes::module(ast, item);
        assert!(s.bound_above_root("K"));
        assert!(!s.bound_above_root("J"));
        assert!(!s.bound_above_root("M"));
        assert!(s.resolve("J").is_some());
        assert!(s.resolve("y").is_some());
        assert!(!s.is_local("K"));
    }
}
