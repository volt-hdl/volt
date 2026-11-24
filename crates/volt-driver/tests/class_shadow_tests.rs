//! Sınıflama gölgesi (#412, ADR-0137; ADR-0136 Aşama 1'in biçimi): bir
//! portun saat, reset ya da veri olduğu iki yoldan hesaplanır ve iki yol
//! aynı cevabı vermelidir.
//!
//! - Anlamsal: tip denetiminin tanıma verdiği tip
//!   (`volt_hir::port_class::DefTypes`, `Ty::Clock`, `Ty::Reset`).
//! - Sözdizimsel: takma ad zinciri (`volt_ast::alias::signal_class`). Onu
//!   çağıran yerler (`signal_class_callers_tests.rs`'teki izin listesi) iki
//!   görünümden birinde okur:
//!   - [`View::Unit`]: birimin bütün AST'si. W1001 süzgeci, test blokları,
//!     LSP'nin yarım kod yolu, `@mmio` açılımı ve otomatik kontratlar
//!     ayrıştırma bittikten sonra koşar.
//!   - [`View::ParseOrder`]: ayrıştırma sırası. Pipeline saati (E5011) ve
//!     E0019 önerisi öğe ayrıştırılırken koşar; AST'de o an önceki
//!     dosyaların bütün öğeleri ve kendi dosyasında ondan önce gelenler
//!     vardır (`volt_syntax::parse_unit`).
//!
//! Girdi, genişlik gölgesininkiyle aynıdır (`shadow_corpus`): derlenen her
//! birimin her modül ve extern portu. Bir fark ya [`KNOWN`]'daki bir
//! nedene (issue numarasıyla) uyar ya da test ne olduğunu, nedenini ve ne
//! yapılabileceğini söyleyerek düşer (ADR-0125 biçimi, [`verdict`]).
//! Karşılaştırma yalnız testte koşar: derleyiciye yeni seçenek girmez.
//! Tablo `--nocapture` ile yazdırılır.

mod shadow_corpus;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::path::Path;

use volt_ast::alias::{signal_class, SignalClass};
use volt_ast::{Idx, Item, ItemKind, Port, SourceFile};
use volt_hir::port_class::DefTypes;
use volt_span::{FileId, SourceMap, Span};

/// Sözdizimsel sınıflamanın okunduğu görünüm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum View {
    /// Birimin bütün AST'si.
    Unit,
    /// Öğe ayrıştırılırken AST'de bulunan öğeler.
    ParseOrder,
}

impl View {
    fn label(self) -> &'static str {
        match self {
            View::Unit => "birim (W1001, test blokları, LSP, @mmio, otomatik kontrat)",
            View::ParseOrder => "ayrıştırma sırası (pipeline saati, E0019 önerisi)",
        }
    }
}

/// Bir portun iki yoldan sınıfları.
#[derive(Debug, Clone, Copy)]
struct Case {
    semantic: SignalClass,
    unit: SignalClass,
    parse_order: SignalClass,
}

impl Case {
    fn syntactic(self, view: View) -> SignalClass {
        match view {
            View::Unit => self.unit,
            View::ParseOrder => self.parse_order,
        }
    }
}

/// Bilinen bir farkın nedeni: bugün iki yolun farklı cevap verdiği,
/// issue'su açık bir durum. `applies` farkı yapısıyla tanır; nedeni
/// çözen PR girdiyi ve (gerekiyorsa) görünümü kaldırır.
struct Known {
    issue: u32,
    view: View,
    cause: &'static str,
    applies: fn(&Case) -> bool,
}

/// Bilinen farklar. Yeni bir neden yalnız issue'suyla girer.
const KNOWN: [Known; 1] = [Known {
    issue: 415,
    view: View::ParseOrder,
    cause: "takma ad onu kullanan öğeden sonra bildirilmiş: ayrıştırma sırasında koşan \
            geçit (pipeline saati, E0019 önerisi) onu görmez",
    // Aynı kural bütün birimde tip denetimiyle aynı cevabı veriyorsa fark
    // yalnız bildirim sırasındandır.
    applies: |c| c.unit == c.semantic,
}];

/// Bir karşılaştırmanın sonucu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Outcome {
    Agree,
    Known(u32),
    New,
}

fn outcome(case: &Case, view: View) -> Outcome {
    if case.syntactic(view) == case.semantic {
        return Outcome::Agree;
    }
    KNOWN
        .iter()
        .find(|k| k.view == view && (k.applies)(case))
        .map_or(Outcome::New, |k| Outcome::Known(k.issue))
}

/// Farklı çıkan bir port (yazdırmak için).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Finding {
    outcome: Outcome,
    view: View,
    location: String,
    port: String,
    semantic: String,
    syntactic: String,
}

#[derive(Default)]
struct Totals {
    units: usize,
    skipped: BTreeMap<&'static str, usize>,
    items: usize,
    ports: usize,
    /// (görünüm, sonuç) → port sayısı.
    by_outcome: BTreeMap<(View, Outcome), usize>,
    /// Anlamsal sınıfa göre port sayısı.
    by_class: BTreeMap<String, usize>,
    findings: BTreeSet<Finding>,
}

fn class_text(c: SignalClass) -> String {
    match c {
        SignalClass::Clock => "saat".to_string(),
        SignalClass::Reset(None) => "reset".to_string(),
        SignalClass::Reset(Some(spec)) => format!("reset({:?}, {:?})", spec.sync, spec.polarity),
        SignalClass::Data => "veri".to_string(),
    }
}

/// Ayrıştırma sırasında `a` `b`'den önce mi okundu: dosyalar bağımlılık
/// sırasıyla (`LoadedUnit::files`), bir dosyanın içinde kaynak sırasıyla.
/// Sıralamada olmayan (üretilen) dosya en sonda.
fn parsed_before(rank: &HashMap<FileId, usize>, a: Span, b: Span) -> bool {
    let r = |s: Span| rank.get(&s.file).copied().unwrap_or(usize::MAX);
    (r(a), a.start) < (r(b), b.start)
}

/// Portu olan öğeler: modüller (pipeline açılımı ve monomorflar dahil) ve
/// extern modüller.
fn ports_of(item: &Item) -> Option<(&str, &[Port])> {
    match &item.kind {
        ItemKind::Module(m) => Some((m.name.text.as_str(), m.ports.as_slice())),
        ItemKind::Extern(e) => Some((e.name.text.as_str(), e.ports.as_slice())),
        ItemKind::Domain(_)
        | ItemKind::Fn(_)
        | ItemKind::Struct(_)
        | ItemKind::Enum(_)
        | ItemKind::Const(_)
        | ItemKind::TypeAlias(_)
        | ItemKind::Test(_)
        | ItemKind::Error => None,
    }
}

/// Bir birimin bütün portlarını iki yoldan sınıflar.
fn compare_ports(
    ast: &SourceFile,
    files: &[(FileId, std::path::PathBuf)],
    classes: DefTypes<'_>,
    res: &volt_hir::ResolveResult,
    map: &SourceMap,
    totals: &mut Totals,
) {
    let rank: HashMap<FileId, usize> = files
        .iter()
        .enumerate()
        .map(|(i, (f, _))| (*f, i))
        .collect();
    let is_alias = |i: Idx<Item>| matches!(ast.items_arena[i].kind, ItemKind::TypeAlias(_));
    // Ayrıştırma sırası görünümü: takma adlardan yalnız öğeden önce
    // okunanlar kalır; arenalar aynıdır.
    let mut view = ast.clone();
    for &idx in &ast.items {
        let item = &ast.items_arena[idx];
        let Some((name, ports)) = ports_of(item) else {
            continue;
        };
        totals.items += 1;
        view.items = ast
            .items
            .iter()
            .copied()
            .filter(|&j| !is_alias(j) || parsed_before(&rank, ast.items_arena[j].span, item.span))
            .collect();
        for port in ports {
            totals.ports += 1;
            let case = Case {
                semantic: classes.port(res, port),
                unit: signal_class(ast, port.ty),
                parse_order: signal_class(&view, port.ty),
            };
            *totals
                .by_class
                .entry(class_text(case.semantic))
                .or_default() += 1;
            for v in [View::Unit, View::ParseOrder] {
                let o = outcome(&case, v);
                *totals.by_outcome.entry((v, o)).or_default() += 1;
                if o == Outcome::Agree {
                    continue;
                }
                let (line, col) = map.line_col(port.name.span);
                let file = shadow_corpus::display_name(map.path(port.name.span.file));
                totals.findings.insert(Finding {
                    outcome: o,
                    view: v,
                    location: format!("{file}:{line}:{col}"),
                    port: format!("{name}.{}", port.name.text),
                    semantic: class_text(case.semantic),
                    syntactic: class_text(case.syntactic(v)),
                });
            }
        }
    }
}

/// Bir birimi yükler ve portlarını karşılaştırır; derlenmeyen birim
/// atlanır (sayılır).
fn compare_unit(main: &Path, totals: &mut Totals) {
    let a = match shadow_corpus::analyze(main) {
        Ok(a) => a,
        Err(s) => {
            *totals.skipped.entry(s.reason).or_default() += 1;
            return;
        }
    };
    totals.units += 1;
    compare_ports(
        &a.unit.parsed.ast,
        &a.unit.files,
        DefTypes::of(&a.analysis.typeck),
        &a.analysis.resolve,
        &a.unit.map,
        totals,
    );
}

fn report(files: usize, book: usize, totals: &Totals) -> String {
    let mut r = String::new();
    let _ = writeln!(
        r,
        "sınıflama gölgesi: {files} dosya + {book} kitap bloğu; derlenen birim {}; atlanan {:?}",
        totals.units, totals.skipped
    );
    let _ = writeln!(
        r,
        "portlu öğe {}, port {}; anlamsal sınıf {:?}",
        totals.items, totals.ports, totals.by_class
    );
    for ((view, outcome), n) in &totals.by_outcome {
        let _ = writeln!(r, "  {}\t{outcome:?}\t{n}", view.label());
    }
    for k in &KNOWN {
        let n = totals
            .by_outcome
            .get(&(k.view, Outcome::Known(k.issue)))
            .copied()
            .unwrap_or(0);
        let _ = writeln!(
            r,
            "  bilinen #{} ({}): {n} port; {}",
            k.issue,
            k.view.label(),
            k.cause
        );
    }
    for f in &totals.findings {
        let _ = writeln!(
            r,
            "  [{:?}] {} | {} `{}` | tip denetimi {} | takma ad zinciri {}",
            f.outcome,
            f.view.label(),
            f.location,
            f.port,
            f.semantic,
            f.syntactic
        );
    }
    r
}

#[test]
fn port_classes_agree_between_the_type_checker_and_the_alias_chain() {
    let mut totals = Totals::default();
    let (files, book) =
        shadow_corpus::for_each_input("class-shadow", &mut |path| compare_unit(path, &mut totals));
    let report = report(files, book, &totals);
    println!("{report}");

    assert!(totals.units > 0, "hiç birim karşılaştırılmadı\n{report}");
    assert!(book > 0, "kitapta volt bloğu bulunmadı\n{report}");
    // Karşılaştırma gerçekten saat ve reset portu gördü.
    for class in ["saat", "veri"] {
        assert!(
            totals.by_class.get(class).is_some_and(|&n| n > 0),
            "hiç {class} portu karşılaştırılmadı\n{report}"
        );
    }
    assert!(
        totals.by_class.keys().any(|k| k.starts_with("reset")),
        "hiç reset portu karşılaştırılmadı\n{report}"
    );
    if let Err(message) = verdict(&totals) {
        panic!("{message}\n{report}");
    }
}

/// Gölgenin kararı: [`KNOWN`]'a uymayan fark yok. Değilse hata metni
/// ADR-0125'in üç parçasını taşır.
fn verdict(totals: &Totals) -> Result<(), String> {
    let new: Vec<&Finding> = totals
        .findings
        .iter()
        .filter(|f| f.outcome == Outcome::New)
        .collect();
    if new.is_empty() {
        return Ok(());
    }
    let mut m = String::new();
    let _ = writeln!(
        m,
        "sınıflama gölgesi: tip denetimi ile takma ad zinciri {} portta farklı sınıf veriyor",
        new.len()
    );
    let _ = writeln!(m, "ne oldu:");
    for f in new.iter().take(20) {
        let _ = writeln!(
            m,
            "  {} `{}` ({}): tip denetimi {}, takma ad zinciri {}",
            f.location,
            f.port,
            f.view.label(),
            f.semantic,
            f.syntactic
        );
    }
    if new.len() > 20 {
        let _ = writeln!(m, "  … ve {} port daha", new.len() - 20);
    }
    let _ = writeln!(
        m,
        "neden: tip denetiminden önce koşan geçitler (ayrıştırıcının geçitleri, W1001 \
         süzgeci, test blokları, LSP'nin yarım kod yolu) portun saat ya da reset \
         olduğunu takma ad zincirinden okur; derleyicinin geri kalanı tip denetiminden. \
         İki cevap ayrışınca aynı port bir geçitte saat, ötekinde veri olur: pipeline \
         saatsiz kalır, test saati sürer, ham reset portu sahte uyarı alır (#234). \
         Derleyici çıktısını etkileyen karar tip denetiminden gelmelidir (ADR-0137)."
    );
    let _ = writeln!(
        m,
        "ne yapabilirsin: takma adın zincirde neden farklı çözüldüğünü bul (sıra, kapsam, \
         içe aktarma). Zincir yanlışsa `volt_ast::alias`'ı düzelt ya da çağıranı tip \
         denetiminin sonucunu okuyacak yere taşı (ADR-0137 geçiş sırası). Fark bugün \
         çözülemeyen bilinen bir sınırsa önce bir issue aç, sonra nedeni issue \
         numarasıyla `KNOWN`'a ekle; nedeni yazılmamış bir fark bu testte susturulmaz."
    );
    Err(m)
}

/// Tek bulgulu toplam.
fn totals_with(outcome: Outcome) -> Totals {
    let mut totals = Totals::default();
    totals.findings.insert(Finding {
        outcome,
        view: View::Unit,
        location: "yeni.volt:3:9".to_string(),
        port: "M.clk".to_string(),
        semantic: "saat".to_string(),
        syntactic: "veri".to_string(),
    });
    totals
}

/// Yeni bir fark testi düşürür; mesaj ADR-0125'in üç parçasını ve portu
/// taşır. Bilinen fark ve uyuşma düşürmez.
#[test]
fn a_new_difference_fails_with_what_why_and_how() {
    let message = verdict(&totals_with(Outcome::New)).expect_err("yeni fark testi düşürmeli");
    for part in [
        "ne oldu:",
        "neden:",
        "ne yapabilirsin:",
        "yeni.volt:3:9 `M.clk`",
        "tip denetimi saat, takma ad zinciri veri",
    ] {
        assert!(message.contains(part), "`{part}` yok:\n{message}");
    }
    assert!(verdict(&totals_with(Outcome::Known(415))).is_ok());
    assert!(verdict(&totals_with(Outcome::Agree)).is_ok());
}

/// Bir kaynağı geçici bir dizinde derler ve karşılaştırır.
fn compare_source(tag: &str, source: &str) -> Totals {
    let dir = std::env::temp_dir().join(format!("volt-class-shadow-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("geçici dizin");
    let main = dir.join("main.volt");
    std::fs::write(&main, source).expect("kaynak yazılmalı");
    let mut totals = Totals::default();
    compare_unit(&main, &mut totals);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(totals.units, 1, "kaynak derlenmeli: {:?}", totals.skipped);
    totals
}

/// #415'in biçimi: takma ad modülden sonra bildirilmiş. Tip denetimi ve
/// bütün birim görünümü portu saat sayar; ayrıştırma sırası görünümü veri
/// sayar ve fark bilinen #415 nedenine uyar. Modül E0019'a düşseydi
/// önerisi `on <clock>` olurdu (`divergence_triage_tests.rs`).
#[test]
fn an_alias_declared_after_its_module_is_the_known_415_difference() {
    let totals = compare_source(
        "415",
        "module M {\n    in  clk : Clk\n    in  d   : u8\n    out q   : u8\n    \
         reg r : u8 = 0\n    on clk { r <= d }\n    q = r\n}\n\ntype Clk = clock\n",
    );
    let findings: Vec<_> = totals.findings.iter().collect();
    assert_eq!(findings.len(), 1, "{}", report(1, 0, &totals));
    let f = findings[0];
    assert_eq!(
        (
            f.outcome,
            f.view,
            f.port.as_str(),
            f.semantic.as_str(),
            f.syntactic.as_str()
        ),
        (
            Outcome::Known(415),
            View::ParseOrder,
            "M.clk",
            "saat",
            "veri"
        )
    );
}

/// Takma ad öğeden önce bildirilince (ya da zincir, ya da reset) iki
/// görünüm de tip denetimiyle aynı sınıfı verir.
#[test]
fn an_alias_declared_before_its_module_agrees_in_both_views() {
    let totals = compare_source(
        "before",
        "type ClkBase = clock\ntype Clk = ClkBase\ntype Rst = reset(async, active_low)\n\n\
         domain D { clock = posedge, reset = async active_low }\n\n\
         module M {\n    in  clk : Clk @D\n    in  rst : Rst\n    in  d   : u8\n    \
         out q   : u8\n    reg r : u8 = 0\n    on clk { r <= d }\n    q = r\n}\n",
    );
    assert!(totals.findings.is_empty(), "{}", report(1, 0, &totals));
    assert_eq!(totals.ports, 4);
    assert_eq!(totals.by_class.get("saat"), Some(&1));
    assert_eq!(totals.by_class.get("reset(Async, ActiveLow)"), Some(&1));
}
