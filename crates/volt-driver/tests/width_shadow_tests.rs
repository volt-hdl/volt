//! Genişlik gölgesi (#407 Aşama 1, ADR-0136): SV üreticinin her ifadeye
//! verdiği genişlik (`width_of`) tip denetiminin `expr_types`'taki
//! genişliğiyle karşılaştırılır. Girdi: `tests/fixtures`, `tests/ui/pass`,
//! `tests/ui/multifile`, `tests/real_bugs`, `examples`, `templates`, `demo`
//! ve kitabın `volt` blokları (`book/tools/check_book.py` gibi); sınıflama
//! gölgesiyle ortak yükleyici `shadow_corpus` (ADR-0137).
//!
//! Test katıdır (ADR-0136 §2, #215'in düzeltmesinden sonra): (a) ve (b)
//! sınıfında hiç ayrışma olmamalı, (c) sınıfındaki her ayrışma ADR-0136
//! §5'in dört kuralından birine dayanmalı ([`C_RULES`]); değilse test
//! ne olduğunu, nedenini ve ne yapılabileceğini söyleyerek düşer
//! (ADR-0125 biçimi, [`verdict`]). Tablo `--nocapture` ile yazdırılır.
//! Kayıt üretilen SV'yi değiştirmez: her birim kayıtlı ve kayıtsız iki kez
//! üretilir, metinler aynı olmalı.

mod shadow_corpus;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::path::Path;

use shadow_corpus::{display_name, has_error};
use volt_ast::{BinOp, Expr, ExprKind, Idx, ItemKind, MatchArmBody, SourceFile, UnOp};
use volt_hir::ty::{Ty, TypeArena, TypeId};
use volt_sv_emit::width_shadow::{record, WidthNote};
use volt_sv_emit::{ConstArrayStyle, Sig, SvaMode};

/// Tip denetiminin bir ifadeye verdiği genişlik.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Checked {
    /// Somut genişlik ve işaret.
    Width { width: u32, signed: bool },
    /// Esnek aralık (ADR-0025): `lo` işlem genişliği, `hi` doğal genişlik.
    Flex { lo: u32, hi: u32 },
    /// Elemanı genişlikli dizi: paketlenmiş toplam genişlik (ADR-0056).
    Array { total: u32 },
    /// Genişliği olmayan tip (boyutsuz literal, örnek ...): gösterimi.
    NoWidth(String),
    /// `expr_types`'ta kayıt yok.
    Missing,
}

/// Genişlik tip denetiminin tek kaynağından, `TypeArena::bit_width`'ten
/// okunur (#419): `taps`, W3003 ve kısmi sürücü kapsamı da onu okur. Gölge
/// #419'a kadar kendi kopyasıyla (`Trit` 2, `bool` 1) karşılaştırıyordu;
/// o kopya doğruydu, karar veren fonksiyon (`signal_width`) değildi, bu
/// yüzden gölge `Trit`'in genişliğini hiç sınamadı.
fn checked(types: &TypeArena, ty: TypeId) -> Checked {
    let no_width = || Checked::NoWidth(types.display(ty));
    match types.ty(ty) {
        Ty::UIntFlex { lo, hi } | Ty::SIntFlex { lo, hi } => Checked::Flex {
            lo: u32::from(*lo),
            hi: u32::from(*hi),
        },
        Ty::Delayed { inner, .. } => checked(types, *inner),
        Ty::Array { elem, .. } => match (checked(types, *elem), types.bit_width(ty)) {
            (Checked::Width { .. }, Some(total)) => Checked::Array { total },
            _ => no_width(),
        },
        Ty::IntLit | Ty::Tuple(_) | Ty::Instance(_) | Ty::Builtin { .. } | Ty::Error => no_width(),
        Ty::Bool
        | Ty::Clock
        | Ty::Reset { .. }
        | Ty::UInt { .. }
        | Ty::Bits { .. }
        | Ty::SInt { .. }
        | Ty::Trit
        | Ty::Enum(_)
        | Ty::Struct(_) => match types.bit_width(ty) {
            Some(width) => Checked::Width {
                width,
                signed: matches!(types.ty(ty), Ty::SInt { .. } | Ty::Trit),
            },
            None => no_width(),
        },
    }
}

/// Bir karşılaştırmanın sınıfı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Class {
    /// İki taraf aynı genişliği veriyor.
    Agree,
    /// İki taraf da genişlik vermiyor.
    BothNone,
    /// (a) iki taraf da somut ve farklı: sessiz-yanlış adayı.
    A,
    /// (b) bir taraf genişlik vermiyor: kapsam boşluğu.
    B,
    /// (c) dilin tanımladığı fark: beklenen.
    C,
}

/// İfadenin, karşılaştırmayı dilin kuralıyla açıklayan özellikleri.
#[derive(Debug, Clone, Copy, Default)]
struct Shape {
    /// Kendi tipi olmayan boyutsuz literal ağacı (type-inference.md §3.1,
    /// §4): soneksiz literal; onun `-`/`~`'si; iki yanı böyle olan
    /// aritmetik; kolları böyle olan `if`/`match`.
    literal: bool,
    /// Tip denetimi ifadeye bağlamın tipini kaydetti (check kipi):
    /// ADR-0041'in genişletme yolunda aritmetik ve iki operandı (üçünün
    /// kaydı da hedef tiptir, `typeck/check.rs` `widen_operands`), ya da
    /// kaydı hiçbir kolununkiyle aynı olmayan `if`/`match` (§3.7, §3.9:
    /// beklenen tip kollara itilir).
    context: bool,
    /// Ad (sinyal, `let`, sabit): üreticinin genişliği bildirimin
    /// genişliğidir.
    path: bool,
}

/// (c) kuralları, ADR-0136 §5'in tablosundaki sırayla. Her biri dilin bir
/// kuralıdır; [`classify`] (c)'yi yalnız bunlardan biriyle verir.
const RULE_LITERAL: &str =
    "boyutsuz literal bağlamdan tip alır (type-inference §4): üretici genişlik vermez";
const RULE_CONTEXT: &str = "bağlam genişliği (ADR-0041): tip denetimi hedef tipi kaydeder, üretici doğal genişliği verip W'(x) yazar";
const RULE_ARRAY: &str =
    "dizi portu/teli paketlenmiş vektör (ADR-0056): üretici toplam genişliği verir";
const RULE_FLEX: &str =
    "esnek aralık (ADR-0025): üretici işlem genişliğini verir, okuyan genişliği seçer";

/// (c) sınıfının izinli gerekçeleri: ADR-0136 §5'in dört kuralı. Yeni bir
/// (c) kuralı dilin bir kuralına dayanır ve bir ADR ile buraya girer.
const C_RULES: [&str; 4] = [RULE_LITERAL, RULE_CONTEXT, RULE_ARRAY, RULE_FLEX];

/// Karşılaştırma: sınıf ve gerekçe etiketi.
fn classify(emitted: Option<Sig>, checked: &Checked, shape: Shape) -> (Class, &'static str) {
    match (emitted, checked) {
        (Some(s), Checked::Width { width, .. }) if s.width == *width => {
            (Class::Agree, "aynı genişlik")
        }
        (Some(s), Checked::Width { width, signed })
            if shape.context && s.width < *width && s.signed == *signed =>
        {
            (Class::C, RULE_CONTEXT)
        }
        (Some(_), Checked::Width { .. }) => (Class::A, "farklı somut genişlik"),
        (Some(s), Checked::Flex { hi, .. }) if s.width == *hi => {
            (Class::Agree, "esnek: doğal genişlik")
        }
        // Bir ad (tipsiz `let`'in teli) esnek tipliyse tel doğal
        // genişliktedir (ADR-0025): üreticinin verdiği, telin bildirildiği
        // genişliktir ve `hi` olmalıdır (#215). İşlem, `if` ve `match`
        // ise aralıktaki genişliği okuyana göre alır.
        (Some(s), Checked::Flex { lo, hi }) if !shape.path && *lo <= s.width && s.width < *hi => {
            (Class::C, RULE_FLEX)
        }
        (Some(_), Checked::Flex { .. }) if shape.path => {
            (Class::A, "esnek tipli ad doğal genişlikten dar")
        }
        (Some(_), Checked::Flex { .. }) => (Class::A, "esnek aralığın dışında"),
        (Some(s), Checked::Array { total }) if s.width == *total => (Class::C, RULE_ARRAY),
        (Some(_), Checked::Array { .. }) => (Class::A, "dizinin paketlenmiş genişliğinden farklı"),
        (None, Checked::NoWidth(_)) => (Class::BothNone, "iki taraf da genişliksiz"),
        (None, Checked::Width { .. } | Checked::Flex { .. }) if shape.literal => {
            (Class::C, RULE_LITERAL)
        }
        (None, Checked::Width { .. } | Checked::Flex { .. } | Checked::Array { .. }) => {
            (Class::B, "üretici genişlik vermiyor")
        }
        (Some(_), Checked::NoWidth(_)) => (Class::B, "tip denetimi genişlik vermiyor"),
        (_, Checked::Missing) => (Class::B, "tip denetimi ifadeyi kaydetmiyor"),
    }
}

fn is_arith(op: BinOp) -> bool {
    matches!(
        op,
        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem
    )
}

/// Boyutsuz literal ağacı mı ([`Shape::literal`]).
fn is_literal(ast: &SourceFile, e: Idx<Expr>) -> bool {
    match &ast.exprs[e].kind {
        ExprKind::IntLit { suffix, .. } => suffix.is_none(),
        ExprKind::Unary {
            op: UnOp::Neg | UnOp::BitNot,
            operand,
        } => is_literal(ast, *operand),
        ExprKind::Binary { op, lhs, rhs } if is_arith(*op) => {
            is_literal(ast, *lhs) && is_literal(ast, *rhs)
        }
        ExprKind::If {
            then_expr,
            else_expr,
            ..
        } => is_literal(ast, *then_expr) && is_literal(ast, *else_expr),
        ExprKind::Match { arms, .. } => arms.iter().all(|a| match a.body {
            MatchArmBody::Expr(b) => is_literal(ast, b),
            MatchArmBody::Block(_) => false,
        }),
        _ => false,
    }
}

/// Kök AST'deki her ifadenin [`Shape`]'i.
fn shapes(ast: &SourceFile, types: &HashMap<Idx<Expr>, TypeId>) -> HashMap<Idx<Expr>, Shape> {
    let mut out: HashMap<Idx<Expr>, Shape> = HashMap::new();
    for (idx, e) in ast.exprs.iter_idx() {
        let shape = out.entry(idx).or_default();
        shape.literal = is_literal(ast, idx);
        shape.path = matches!(e.kind, ExprKind::Path(_));
        let Some(own) = types.get(&idx) else {
            continue;
        };
        let own = Some(own);
        let context = match &e.kind {
            ExprKind::Binary { op, lhs, rhs }
                if is_arith(*op) && types.get(lhs) == own && types.get(rhs) == own =>
            {
                for i in [*lhs, *rhs] {
                    out.entry(i).or_default().context = true;
                }
                true
            }
            ExprKind::If {
                then_expr,
                else_expr,
                ..
            } => types.get(then_expr) != own && types.get(else_expr) != own,
            ExprKind::Match { arms, .. } => arms.iter().all(|a| match a.body {
                MatchArmBody::Expr(b) => types.get(&b) != own,
                MatchArmBody::Block(_) => true,
            }),
            _ => false,
        };
        if context {
            out.entry(idx).or_default().context = true;
        }
    }
    out
}

/// Bir ayrışma örneği (yazdırmak için).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Finding {
    class: Class,
    reason: &'static str,
    location: String,
    snippet: String,
    emitted: String,
    checked: String,
}

#[derive(Default)]
struct Totals {
    units: usize,
    /// Atlanan birimler, nedene göre: dosya ve ilk hata kodları.
    skipped: BTreeMap<&'static str, Vec<String>>,
    /// Karşılaştırılan (birim, ifade) çiftleri.
    exprs: usize,
    /// Karşılaştırmalar (bir ifadeye üretici farklı bağlamlarda birden çok
    /// genişlik verebilir; her farklı genişlik bir karşılaştırmadır).
    comparisons: usize,
    /// Üreticinin aynı ifadeye birden çok farklı genişlik verdiği ifadeler.
    multi: usize,
    by_class: BTreeMap<(Class, &'static str), usize>,
    /// Kopya AST'den gelen, kök AST'deki aynı indeksli ifadeyle eşleşen
    /// ve eşleşmeyen ifadeler (ADR-0081 fn açılımı, ADR-0077 struct
    /// indirgemesi).
    copy_matched: usize,
    copy_unmatched: usize,
    /// Tip denetiminin kaydettiği ifadeler ve üreticinin genişliğini
    /// sorduğu kısmı (gölgenin kapsamı).
    typed: usize,
    typed_compared: usize,
    findings: BTreeSet<Finding>,
}

fn sig_text(sig: Option<Sig>) -> String {
    match sig {
        Some(s) if s.signed => format!("{} (işaretli)", s.width),
        Some(s) => s.width.to_string(),
        None => "-".to_string(),
    }
}

/// Atlanan birimi nedeni ve hata kodlarıyla kaydeder.
fn skip(
    totals: &mut Totals,
    reason: &'static str,
    main: &Path,
    diagnostics: &[volt_diagnostics::Diagnostic],
) {
    let codes: BTreeSet<&str> = diagnostics
        .iter()
        .filter(|d| !d.code.is_warning())
        .map(|d| d.code.as_str())
        .collect();
    let name = display_name(main);
    let codes: Vec<&str> = codes.into_iter().collect();
    totals
        .skipped
        .entry(reason)
        .or_default()
        .push(format!("{name} [{}]", codes.join(" ")));
}

/// Bir birimi derler, iki tarafı karşılaştırır. Derlenmeyen birim
/// atlanır (sayılır).
fn compare_unit(main: &Path, totals: &mut Totals) {
    let (unit, analysis) = match shadow_corpus::analyze(main) {
        Ok(a) => (a.unit, a.analysis),
        Err(s) => {
            skip(totals, s.reason, main, &s.diagnostics);
            return;
        }
    };
    let ast = &unit.parsed.ast;
    let names = unit.source_names();
    let sources = volt_sv_emit::unit_source_texts(&names, &unit.map);
    let emit = || {
        volt_sv_emit::emit_unit(
            ast,
            "shadow.volt",
            &sources,
            SvaMode::None,
            ConstArrayStyle::default(),
        )
    };
    let plain = emit();
    let (recorded, notes) = record(emit);
    assert_eq!(
        plain.sv,
        recorded.sv,
        "{}: genişlik kaydı üretilen SV'yi değiştirdi",
        main.display()
    );
    if has_error(&recorded.diagnostics) {
        skip(totals, "üretici hatası", main, &recorded.diagnostics);
        return;
    }
    totals.units += 1;
    compare_notes(ast, &analysis.typeck, &unit.map, &notes, totals);
}

fn compare_notes(
    ast: &SourceFile,
    typeck: &volt_hir::TypeckResult,
    map: &volt_span::SourceMap,
    notes: &[WidthNote],
    totals: &mut Totals,
) {
    let root = ast as *const SourceFile as usize;
    let root_exprs: HashMap<Idx<Expr>, &Expr> = ast.exprs.iter_idx().collect();
    // Kök AST indeksi → üreticinin verdiği farklı genişlikler.
    let mut widths: HashMap<Idx<Expr>, BTreeSet<Option<(u32, bool)>>> = HashMap::new();
    let mut copies: HashMap<(usize, Idx<Expr>), bool> = HashMap::new();
    for n in notes {
        if n.ast != root {
            // Kopyadaki ifade, kök AST'de aynı indeksli, aynı konumlu ve
            // aynı türden ifade varsa onunla eşleşir.
            let same = root_exprs
                .get(&n.expr)
                .is_some_and(|e| e.span == n.span && std::mem::discriminant(&e.kind) == n.kind);
            copies.insert((n.ast, n.expr), same);
            if !same {
                continue;
            }
        }
        widths
            .entry(n.expr)
            .or_default()
            .insert(n.sig.map(|s| (s.width, s.signed)));
    }
    totals.copy_matched += copies.values().filter(|&&m| m).count();
    totals.copy_unmatched += copies.values().filter(|&&m| !m).count();
    let shapes = shapes(ast, &typeck.expr_types);
    totals.typed += typeck.expr_types.len();
    totals.typed_compared += widths
        .keys()
        .filter(|k| typeck.expr_types.contains_key(k))
        .count();
    for (idx, sigs) in widths {
        totals.exprs += 1;
        if sigs.len() > 1 {
            totals.multi += 1;
        }
        let ty = typeck.expr_types.get(&idx).copied();
        let want = ty.map_or(Checked::Missing, |t| checked(&typeck.types, t));
        for sig in sigs {
            let sig = sig.map(|(width, signed)| Sig { width, signed });
            totals.comparisons += 1;
            let shape = shapes.get(&idx).copied().unwrap_or_default();
            let (class, reason) = classify(sig, &want, shape);
            *totals.by_class.entry((class, reason)).or_default() += 1;
            if matches!(class, Class::A | Class::B | Class::C) {
                let span = ast.exprs[idx].span;
                let (line, col) = map.line_col(span);
                let file = map
                    .path(span.file)
                    .file_name()
                    .map(|f| f.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let text = map
                    .source(span.file)
                    .get(span.start as usize..span.end as usize)
                    .unwrap_or("<üretilen kaynak>");
                let snippet: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
                let snippet = if snippet.chars().count() > 60 {
                    format!("{}…", snippet.chars().take(60).collect::<String>())
                } else {
                    snippet
                };
                totals.findings.insert(Finding {
                    class,
                    reason,
                    location: format!("{file}:{line}:{col}"),
                    snippet,
                    emitted: sig_text(sig),
                    checked: ty.map_or("-".to_string(), |t| typeck.types.display(t)),
                });
            }
        }
    }
}

#[test]
fn emitter_widths_are_compared_with_the_type_checker() {
    let mut totals = Totals::default();
    let (files, book) =
        shadow_corpus::for_each_input("width-shadow", &mut |path| compare_unit(path, &mut totals));

    let mut report = String::new();
    let _ = writeln!(
        report,
        "genişlik gölgesi: {} dosya + {} kitap bloğu; derlenen birim {}; atlanan {:?}",
        files,
        book,
        totals.units,
        totals
            .skipped
            .iter()
            .map(|(r, v)| (*r, v.len()))
            .collect::<BTreeMap<_, _>>()
    );
    let _ = writeln!(
        report,
        "ifade {}, karşılaştırma {}, birden çok genişlik {}, kopya eşleşen {}, kopya eşleşmeyen {}",
        totals.exprs, totals.comparisons, totals.multi, totals.copy_matched, totals.copy_unmatched
    );
    let _ = writeln!(
        report,
        "tip denetiminin kaydettiği ifade {}, üreticinin genişliğini sorduğu {}",
        totals.typed, totals.typed_compared
    );
    for ((class, reason), n) in &totals.by_class {
        let _ = writeln!(report, "  {class:?}\t{n}\t{reason}");
    }
    for (reason, files) in &totals.skipped {
        for f in files {
            let _ = writeln!(report, "  atlandı ({reason}): {f}");
        }
    }
    for f in &totals.findings {
        let _ = writeln!(
            report,
            "  [{:?}] {} | {} | `{}` | üretici {} | tip denetimi {}",
            f.class, f.reason, f.location, f.snippet, f.emitted, f.checked
        );
    }
    println!("{report}");

    assert!(totals.units > 0, "hiç birim karşılaştırılmadı\n{report}");
    assert!(totals.exprs > 0, "hiç ifade karşılaştırılmadı\n{report}");
    assert!(book > 0, "kitapta volt bloğu bulunmadı\n{report}");
    // Katı karar: (a) ve (b) sıfır, (c) yalnız listeli kurallarla. Tablo
    // yukarıda yazdırıldı; düşen testin çıktısında o da görünür.
    if let Err(message) = verdict(&totals) {
        panic!("{message}");
    }
}

/// Portun genişliği iki temsilde aynıdır (#419): tip denetiminin
/// `TypeArena::bit_width`'i (tanımın tipi) ile AST'yi okuyan arka uçların
/// `volt_ast::bit_width::type_ref_bits`'i (SV testbench'i, test dili;
/// ADR-0136 Aşama 2'ye kadar tip denetimine ulaşamazlar). İkisi skaler
/// genişliği aynı tablodan okur (`Scalar::bits`); dizi, enum, struct ve
/// takma ad kuralını ayrı temsillerde uygularlar, bu test onları gölgenin
/// bütün girdisinde karşılaştırır.
#[test]
fn port_widths_agree_between_the_type_checker_and_the_ast() {
    let mut units = 0;
    let mut ports = 0;
    let mut with_width = 0;
    let mut unmapped = 0;
    let mut bad: Vec<String> = Vec::new();
    let (files, book) = shadow_corpus::for_each_input("port-width", &mut |path| {
        let Ok(a) = shadow_corpus::analyze(path) else {
            return;
        };
        units += 1;
        let ast = &a.unit.parsed.ast;
        let typeck = &a.analysis.typeck;
        for &idx in &ast.items {
            let decl_ports = match &ast.items_arena[idx].kind {
                ItemKind::Module(m) => &m.ports,
                ItemKind::Extern(e) => &e.ports,
                _ => continue,
            };
            for port in decl_ports {
                let def = a.analysis.resolve.decl_spans.get(&port.name.span);
                let Some(&ty) = def.and_then(|d| typeck.def_types.get(d)) else {
                    unmapped += 1;
                    continue;
                };
                ports += 1;
                let checked = typeck.types.bit_width(ty);
                let from_ast = volt_ast::bit_width::type_ref_bits(ast, port.ty);
                if checked.is_some() {
                    with_width += 1;
                }
                if checked != from_ast {
                    let (line, col) = a.unit.map.line_col(port.name.span);
                    let file = display_name(a.unit.map.path(port.name.span.file));
                    bad.push(format!(
                        "  {file}:{line}:{col} `{}`: tip denetimi {} ({checked:?}), AST {from_ast:?}",
                        port.name.text,
                        typeck.types.display(ty)
                    ));
                }
            }
        }
    });
    println!(
        "port genişliği: {files} dosya + {book} kitap bloğu; birim {units}; port {ports}          (genişlikli {with_width}, tanımı eşlenmeyen {unmapped}); ayrışan {}",
        bad.len()
    );
    assert!(units > 0 && with_width > 0, "hiç port karşılaştırılmadı");
    assert!(
        bad.is_empty(),
        "ne oldu: tip denetimi ile AST {} portun genişliğinde ayrışıyor:
{}
         neden: SV testbench'i ve test dili port genişliğini AST'den okur,          tip denetimi tanımın tipinden; ayrışınca testbench porta tip          denetiminin söylediğinden farklı genişlikte yazar ya da okur (#419'da          `taps`'ın iki genişliği).
         ne yapabilirsin: spec'e göre doğru genişliği bul; yanlış olan tarafı          (`TypeArena::bit_width` ya da `volt_ast::bit_width::type_ref_bits`)          düzelt, skaler genişliği yalnız `Scalar::bits`'te değiştir.",
        bad.len(),
        bad.join("
")
    );
}

/// Gölgenin katı kararı (ADR-0136 §2): (a) ve (b) sınıfında ayrışma yok,
/// (c) sınıfındaki her ayrışmanın gerekçesi [`C_RULES`]'tan. Değilse hata
/// metni ADR-0125'in üç parçasını taşır: ne oldu (ayrışan ifadeler), neden
/// (üretilen SV'nin anlamı ve Aşama 2'nin kilidi), ne yapabilirsin.
fn verdict(totals: &Totals) -> Result<(), String> {
    let bad: Vec<&Finding> = totals
        .findings
        .iter()
        .filter(|f| match f.class {
            Class::A | Class::B => true,
            Class::C => !C_RULES.contains(&f.reason),
            Class::Agree | Class::BothNone => false,
        })
        .collect();
    if bad.is_empty() {
        return Ok(());
    }
    let mut m = String::new();
    let _ = writeln!(
        m,
        "genişlik gölgesi: SV üreticisi ile tip denetimi {} ifadede ayrışıyor",
        bad.len()
    );
    let _ = writeln!(m, "ne oldu:");
    for f in bad.iter().take(20) {
        let class = match f.class {
            Class::A => "(a) sessiz-yanlış adayı",
            Class::B => "(b) kapsam boşluğu",
            Class::C | Class::Agree | Class::BothNone => "(c) listede olmayan gerekçe",
        };
        let _ = writeln!(
            m,
            "  {class}: {} `{}`: üretici {}, tip denetimi {} ({})",
            f.location, f.snippet, f.emitted, f.checked, f.reason
        );
    }
    if bad.len() > 20 {
        let _ = writeln!(m, "  … ve {} ifade daha", bad.len() - 20);
    }
    let _ = writeln!(
        m,
        "neden: üretilen SystemVerilog ifadeyi üreticinin genişliğinde hesaplar, \
         Volt'taki anlamı ise tip denetiminin verdiği genişliktir. (a) iki somut \
         genişliğin farkıdır: donanım spec'in söylediğinden farklı bir değer \
         hesaplayabilir (#215'te 200 + 100 = 44). (b) bir tarafın genişlik \
         vermemesidir: gölge o ifadeyi karşılaştıramaz. Listede olmayan bir (c) \
         gerekçesi dilin bir kuralına dayandırılmamıştır. Genişlik, (a) ya da (b) \
         kaldıkça tip denetiminden okunmaya geçemez (ADR-0136 §2)."
    );
    let _ = writeln!(
        m,
        "ne yapabilirsin: spec'e göre doğru genişliği bul (type-inference.md, \
         ilgili ADR). Üretici yanlışsa üreticiyi düzelt ve en küçük örneği \
         tests/real_bugs'a ekle; tip denetimi yanlışsa onu düzelt. Fark dilin \
         tanımladığı bir kuralsa onu (c) olarak, kuralı yazan bir ADR ile \
         `classify`'a ve `C_RULES`'a ekle. Hangisi olduğuna karar veremiyorsan \
         \"silent-wrong?\" başlıklı bir issue aç; bu test o ayrışma çözülene \
         kadar kırmızı kalır, ayrışmayı susturan bir istisna eklenmez."
    );
    Err(m)
}

/// Sınanan bir karar için tek bulgulu toplam.
fn totals_with(class: Class, reason: &'static str) -> Totals {
    let mut totals = Totals::default();
    totals.findings.insert(Finding {
        class,
        reason,
        location: "yeni.volt:3:9".to_string(),
        snippet: "match c { true => a + b, _ => a - b }".to_string(),
        emitted: "8".to_string(),
        checked: "u8..u9".to_string(),
    });
    totals
}

/// Yeni bir (a) ayrışması testi düşürür; mesaj ADR-0125'in üç parçasını
/// ve ayrışan ifadeyi taşır.
#[test]
fn a_new_silent_wrong_candidate_fails_with_what_why_and_how() {
    let message =
        verdict(&totals_with(Class::A, "farklı somut genişlik")).expect_err("(a) testi düşürmeli");
    for part in [
        "ne oldu:",
        "neden:",
        "ne yapabilirsin:",
        "(a) sessiz-yanlış adayı",
        "yeni.volt:3:9",
        "üretici 8, tip denetimi u8..u9",
    ] {
        assert!(message.contains(part), "`{part}` yok:\n{message}");
    }
}

/// Yeni bir (b) boşluğu da testi düşürür.
#[test]
fn a_new_coverage_gap_fails() {
    let message = verdict(&totals_with(Class::B, "üretici genişlik vermiyor"))
        .expect_err("(b) testi düşürmeli");
    assert!(message.contains("(b) kapsam boşluğu"), "{message}");
}

/// Listede olmayan bir (c) gerekçesi testi düşürür; listedeki dördü
/// düşürmez, uyuşan ve iki tarafı genişliksiz karşılaştırma da.
#[test]
fn only_the_four_listed_rules_are_expected_differences() {
    let message = verdict(&totals_with(Class::C, "yeni bir gerekçe"))
        .expect_err("listede olmayan (c) testi düşürmeli");
    assert!(message.contains("(c) listede olmayan gerekçe"), "{message}");
    for rule in C_RULES {
        assert!(verdict(&totals_with(Class::C, rule)).is_ok(), "{rule}");
    }
    assert!(verdict(&totals_with(Class::Agree, "aynı genişlik")).is_ok());
    assert!(verdict(&totals_with(Class::BothNone, "iki taraf da genişliksiz")).is_ok());
}
