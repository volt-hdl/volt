//! Dosya sınırında görünürlük (ADR-0042 §3, name-resolution.md §3.3).
//!
//! Kök kapsam derleme birimi genelinde tektir; bir dosya başka dosyanın
//! öğesini yalnız `use` ile getirdiyse (adıyla, `as` takma adıyla ya da
//! glob ile) yalın adla kullanır. Kendi dosyasının öğeleri her zaman
//! görünür. Monomorfize örnekler (`Delay_4_8`) şablon adının (`Delay`)
//! görünürlüğünü devralır.
//!
//! Kural burada, tek yerdedir (#139): HIR çözücüsünün kök kapsam araması
//! (`volt-hir` `resolve::imports::scope_get`) ve ayrıştırıcının ad eşleyen
//! birim sonu adımları (sabit değerlendirmesi, bundle ve payload tipleri,
//! kapsam zincirinin kök çerçevesi) aynı [`RootFilter`]'ı sorar. Dosya
//! kapsamlarını `use` bildirimlerinden kuran `volt_syntax::unit::check_imports`
//! E1004/E1010/E1011'i de orada verir.

use std::collections::HashMap;

use volt_span::FileId;

use crate::{ItemKind, Name};

/// Paket yolu — `soc::gpio` → `["soc", "gpio"]`.
pub type PackagePath = Vec<String>;

/// Bir dosyanın import'lardan gelen görünürlüğü.
#[derive(Debug, Default, Clone)]
pub struct FileScope {
    /// Bu dosyada yalın adıyla kullanılabilen dış öğeler: yerel ad →
    /// öğenin tanımlandığı dosya (name-resolution.md §3.3
    /// `file.visible[local] → def`).
    pub visible: HashMap<String, FileId>,
    /// Yerel ad → kök kapsamdaki gerçek ad (`use a::B as C` → C→B).
    pub aliases: HashMap<String, String>,
    /// Yerel ad → (paket, öğe adı, ilk import konumu) — E1010 için.
    pub origin: HashMap<String, (PackagePath, String, Name)>,
}

/// Derleme biriminin dosya başına kapsamları.
pub type FileScopes = HashMap<FileId, FileScope>;

/// Kök kapsam süzgeci. `scopes` yoksa (tek dosya) süzgeç yoktur: her kök
/// öğe görünür.
#[derive(Debug, Clone, Copy)]
pub struct RootFilter<'a> {
    scopes: Option<&'a FileScopes>,
}

impl<'a> RootFilter<'a> {
    pub fn new(scopes: Option<&'a FileScopes>) -> Self {
        RootFilter { scopes }
    }

    /// `file`da yazılan yalın `local` adının kök kapsamdaki adı: takma ad
    /// (`use a::B as C`'de `C`) gerçek ada (`B`) açılır.
    pub fn root_name<'n>(&self, file: FileId, local: &'n str) -> &'n str
    where
        'a: 'n,
    {
        self.scopes
            .and_then(|s| s.get(&file))
            .and_then(|s| s.aliases.get(local))
            .map_or(local, String::as_str)
    }

    /// `def_file`da tanımlı kök öğe, `file`da `local` yerel adıyla görünür
    /// mü? Kendi dosyasının öğesi her zaman; başka dosyanınki yalnız o
    /// dosyadan getirilmişse (monomorf örnek şablon adıyla).
    pub fn sees(&self, file: FileId, local: &str, def_file: FileId) -> bool {
        let Some(scopes) = self.scopes else {
            return true;
        };
        if def_file == file {
            return true;
        }
        scopes.get(&file).is_some_and(|s| {
            s.visible.get(local) == Some(&def_file)
                || s.visible.get(mono_base(local)) == Some(&def_file)
        })
    }

    /// `file`da `local` adıyla görünen aday: `candidates` kök adı aynı
    /// olan öğelerdir (tanım dosyası, değer). Aynı ad iki dosyada E1003'tür
    /// (ADR-0042 §4); yine de görünen tanım seçilir, öteki asla.
    pub fn pick<T: Copy>(
        &self,
        file: FileId,
        local: &str,
        candidates: &[(FileId, T)],
    ) -> Option<T> {
        candidates
            .iter()
            .find(|(def_file, _)| self.sees(file, local, *def_file))
            .map(|&(_, v)| v)
    }
}

/// AST öğesinin adı (test/hata öğeleri adsızdır).
pub fn item_name(kind: &ItemKind) -> Option<&Name> {
    match kind {
        ItemKind::Module(m) => Some(&m.name),
        ItemKind::Domain(d) => Some(&d.name),
        ItemKind::Fn(f) => Some(&f.name),
        ItemKind::Struct(s) => Some(&s.name),
        ItemKind::Enum(e) => Some(&e.name),
        ItemKind::Const(c) => Some(&c.name),
        ItemKind::TypeAlias(t) => Some(&t.name),
        ItemKind::Extern(x) => Some(&x.name),
        ItemKind::Test(_) | ItemKind::Error => None,
    }
}

/// `Fifo_8_16` → `Fifo`: monomorfizasyon son ekini (`_<sayı>`*) atar.
pub fn mono_base(name: &str) -> &str {
    let mut base = name;
    while let Some((head, tail)) = base.rsplit_once('_') {
        if tail.is_empty() || !tail.bytes().all(|b| b.is_ascii_digit()) {
            break;
        }
        base = head;
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;
    use volt_span::Span;

    fn scopes() -> FileScopes {
        // Dosya 1, dosya 0'dan `X`'i `J` adıyla ve `Fifo`'yu getirir.
        let mut s = FileScope::default();
        s.visible.insert("J".into(), FileId(0));
        s.aliases.insert("J".into(), "X".into());
        s.visible.insert("Fifo".into(), FileId(0));
        s.origin.insert(
            "J".into(),
            (
                vec!["lib".into()],
                "X".into(),
                Name {
                    text: "J".into(),
                    span: Span::new(FileId(1), 0, 1),
                },
            ),
        );
        HashMap::from([(FileId(1), s)])
    }

    #[test]
    fn own_file_items_are_always_visible() {
        let s = scopes();
        let v = RootFilter::new(Some(&s));
        assert!(v.sees(FileId(1), "K", FileId(1)));
        assert!(v.sees(FileId(2), "K", FileId(2)));
    }

    #[test]
    fn other_file_items_are_visible_only_when_imported_from_that_file() {
        let s = scopes();
        let v = RootFilter::new(Some(&s));
        assert!(v.sees(FileId(1), "J", FileId(0)));
        assert!(!v.sees(FileId(1), "K", FileId(0)));
        // Getirilen ad başka bir dosyanın aynı adlı öğesini açmaz.
        assert!(!v.sees(FileId(1), "J", FileId(2)));
        // Getirmeyen dosya hiçbir dış öğeyi görmez.
        assert!(!v.sees(FileId(2), "J", FileId(0)));
    }

    #[test]
    fn monomorph_inherits_the_template_import() {
        let s = scopes();
        let v = RootFilter::new(Some(&s));
        assert!(v.sees(FileId(1), "Fifo_8_16", FileId(0)));
        assert!(!v.sees(FileId(1), "Fifo_8_16", FileId(2)));
    }

    #[test]
    fn alias_maps_to_the_root_name_only_in_its_file() {
        let s = scopes();
        let v = RootFilter::new(Some(&s));
        assert_eq!(v.root_name(FileId(1), "J"), "X");
        assert_eq!(v.root_name(FileId(1), "K"), "K");
        assert_eq!(v.root_name(FileId(0), "J"), "J");
    }

    #[test]
    fn without_unit_scopes_everything_is_visible() {
        let v = RootFilter::new(None);
        assert!(v.sees(FileId(1), "K", FileId(0)));
        assert_eq!(v.root_name(FileId(1), "J"), "J");
    }

    #[test]
    fn pick_takes_the_visible_candidate() {
        let s = scopes();
        let v = RootFilter::new(Some(&s));
        let c = [(FileId(2), 'a'), (FileId(0), 'b')];
        assert_eq!(v.pick(FileId(1), "J", &c), Some('b'));
        assert_eq!(v.pick(FileId(2), "J", &c), Some('a'));
        assert_eq!(v.pick(FileId(3), "J", &c), None);
    }

    #[test]
    fn mono_base_strips_numeric_suffixes_only() {
        assert_eq!(mono_base("Fifo_8_16"), "Fifo");
        assert_eq!(mono_base("Fifo"), "Fifo");
        assert_eq!(mono_base("uart_tx"), "uart_tx");
        assert_eq!(mono_base("Shift_reg_4"), "Shift_reg");
        assert_eq!(mono_base("x_"), "x_");
        assert_eq!(mono_base("_"), "_");
    }
}
