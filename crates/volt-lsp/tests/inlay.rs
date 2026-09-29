//! Inlay ipuçları (ADR-0091): tip, saat alanı, gecikme; güven kuralı
//! (hatalı modülde sessizlik, açılmış kopya uyuşmazlığı), görünür aralık
//! ve tür başına ayar.

use volt_lsp::analysis;
use volt_lsp::inlay::{inlay_hints, Hint, HintConfig, HintKind};

const TWO_CLOCKS: &str = "\
domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

module Two {
    in  fclk : clock @Fast
    in  sclk : clock @Slow
    in  a    : u8    @Fast
    in  b    : u8    @Fast
    out y    : u9    @Fast
    out w    : u8    @Fast
    out z    : u8    @Slow

    let s = a + b
    reg r : u8 = 0
    reg q : u8 = 0

    on fclk {
        r <= a
    }
    on sclk {
        q <= q + 1
    }

    y = s
    w = r
    z = q
}
";

const STRICT: &str = "\
@strict_timing
module Pipe {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg a : u32 = 0
    reg b : u32 = 0
    let s = a + 1

    on clk {
        a <= x
        b <= s
    }

    y = b
}
";

const SINGLE_CLOCK: &str = "\
module One {
    in  clk : clock
    in  a   : u8
    in  b   : u8
    out y   : u9

    let s = a + b
    reg r : u9 = 0
    on clk {
        let t = s
        r <= t
    }
    y = r
}
";

fn all(src: &str) -> Vec<Hint> {
    hints_with(src, HintConfig::default())
}

fn hints_with(src: &str, config: HintConfig) -> Vec<Hint> {
    let a = analysis::analyze_hints("test.volt", src);
    inlay_hints(&a, 0, src.len() as u32, config)
}

/// "ad etiketi" biçiminde: ipucunun hemen önündeki tanımlayıcı + etiket.
fn rendered(src: &str, hints: &[Hint]) -> Vec<String> {
    hints
        .iter()
        .map(|h| {
            let before = &src[..h.offset as usize];
            let word: String = before
                .chars()
                .rev()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            format!("{word} {}", h.label)
        })
        .collect()
}

#[test]
fn untyped_let_gets_its_inferred_type() {
    let hints = all(SINGLE_CLOCK);
    let got = rendered(SINGLE_CLOCK, &hints);
    assert!(got.contains(&"s : u9".to_string()), "{got:?}");
    // Blok içindeki let de.
    assert!(got.contains(&"t : u9".to_string()), "{got:?}");
    assert!(hints.iter().all(|h| h.kind == HintKind::Type), "{got:?}");
}

#[test]
fn single_clock_module_has_no_domain_hints() {
    assert!(all(SINGLE_CLOCK).iter().all(|h| h.kind != HintKind::Domain));
}

#[test]
fn multi_clock_module_shows_domains_of_unannotated_signals() {
    let hints = all(TWO_CLOCKS);
    let got = rendered(TWO_CLOCKS, &hints);
    assert_eq!(
        got,
        vec!["s : u9", "s @Fast", "u8 @Fast", "u8 @Slow"],
        "{hints:?}"
    );
    // `reg r : u8` → ipucu yazılmış tipin sonunda (ADR-0088 sırası).
    let r_ty_end = TWO_CLOCKS.find("reg r : u8").unwrap() + "reg r : u8".len();
    assert!(hints.iter().any(|h| h.offset as usize == r_ty_end));
}

#[test]
fn strict_timing_module_shows_latencies() {
    let got = rendered(STRICT, &all(STRICT));
    for want in ["u32 +0", "u32 +2", "u32 +1", "s : u33", "s +1"] {
        assert!(got.contains(&want.to_string()), "{want} yok: {got:?}");
    }
    // Saat zamanlama taşımaz; gecikme ipucu olmayan modülde yok.
    assert!(!got.iter().any(|g| g.starts_with("clock")), "{got:?}");
    assert!(all(SINGLE_CLOCK)
        .iter()
        .all(|h| h.kind != HintKind::Latency));
}

#[test]
fn delayed_annotation_is_not_repeated_as_a_hint() {
    let src = STRICT.replace("reg a : u32 = 0", "reg a : Delayed<u32, 1> = 0");
    let got = rendered(&src, &all(&src));
    assert!(!got.iter().any(|g| g.contains("> +")), "{got:?}");
    assert!(got.contains(&"u32 +2".to_string()), "{got:?}");
}

#[test]
fn each_kind_can_be_switched_off() {
    let only = |types, domains, latency| HintConfig {
        types,
        domains,
        latency,
    };
    let kinds = |src: &str, c| -> Vec<HintKind> {
        hints_with(src, c).into_iter().map(|h| h.kind).collect()
    };
    assert!(!kinds(TWO_CLOCKS, only(false, true, true)).contains(&HintKind::Type));
    assert!(!kinds(TWO_CLOCKS, only(true, false, true)).contains(&HintKind::Domain));
    assert!(!kinds(STRICT, only(true, true, false)).contains(&HintKind::Latency));
    assert!(kinds(STRICT, only(false, false, false)).is_empty());
}

#[test]
fn only_the_requested_range_is_hinted() {
    let a = analysis::analyze_hints("test.volt", SINGLE_CLOCK);
    let t_at = SINGLE_CLOCK.find("let t").unwrap() as u32;
    let hints = inlay_hints(&a, t_at, t_at + 6, HintConfig::default());
    assert_eq!(rendered(SINGLE_CLOCK, &hints), vec!["t : u9"]);
    let none = inlay_hints(&a, 0, 10, HintConfig::default());
    assert!(none.is_empty(), "{none:?}");
}

#[test]
fn a_module_with_an_error_gets_no_hints_but_its_clean_neighbour_does() {
    let broken = "\
module Broken {
    in  a : u8
    out y : u4
    let s = a + 1
    y = s
}
";
    let src = format!("{SINGLE_CLOCK}\n{broken}");
    let hints = all(&src);
    let broken_at = src.find("module Broken").unwrap() as u32;
    assert!(!hints.is_empty());
    assert!(hints.iter().all(|h| h.offset < broken_at), "{hints:?}");
}

#[test]
fn a_parse_error_silences_all_hints() {
    let src = SINGLE_CLOCK.replace("let s = a + b", "let s = a +");
    assert!(all(&src).is_empty());
}

#[test]
fn an_unresolved_name_silences_all_hints() {
    let src = SINGLE_CLOCK.replace("let s = a + b", "let s = a + nope");
    assert!(all(&src).is_empty());
}

const GENERIC: &str = "module Pass<const W: u32> {
    in  x : uint<W>
    out y : uint<W>
    let s = x
    y = s
}

module Top {
    in  a : u4
    in  b : u4
    out p : u4
    out q : u4
    let pa = Pass<4> { x: a }
    let pb = Pass<4> { x: b }
    p = pa.y
    q = pb.y
}
";

#[test]
fn generic_copies_agreeing_on_a_type_get_one_hint() {
    let got = rendered(GENERIC, &all(GENERIC));
    assert_eq!(got, vec!["s : u4"]);
}

#[test]
fn generic_copies_disagreeing_on_a_type_get_no_hint() {
    let src = GENERIC
        .replace("in  b : u4", "in  b : u8")
        .replace("out q : u4", "out q : u8")
        .replace("Pass<4> { x: b }", "Pass<8> { x: b }");
    let a = analysis::analyze_hints("test.volt", &src);
    assert!(a.editor_errors.is_empty());
    let got = rendered(
        &src,
        &inlay_hints(&a, 0, src.len() as u32, HintConfig::default()),
    );
    assert!(got.is_empty(), "{got:?}");
}

#[test]
fn an_error_outside_modules_silences_the_whole_file() {
    // Bozuk struct'a bağımlı modülün ipucu güvenilmez (`q : Bad`).
    let src = "\
struct Bad {
    a : u4
    a : u4
}

module M {
    in  p : Bad
    in  x : u4
    out y : u4
    let q = p
    let r = x
    y = r
}
";
    assert!(all(src).is_empty(), "{:?}", all(src));
}

#[test]
fn a_definition_typed_by_an_erroneous_neighbour_gets_no_type_hint() {
    // Leaf'in port genişliği hatalı (E2025, Leaf susar); Top temiz ama
    // `leaf.b`'nin tipi hata kurtarma tipidir — `q` ipucu almaz, `r` alır.
    let src = "\
module Leaf {
    in  a : u8
    out b : uint<0>
    b = 0
}

module Top {
    in  x : u8
    out y : u8
    let leaf = Leaf { a: x }
    let q = leaf.b
    let r = x
    y = r
}
";
    assert_eq!(rendered(src, &all(src)), vec!["r : u8"]);
}

// ── Çok dosyalı birim (`use`) ─────────────────────────────────────────
// İpuçları tanılarla aynı birim analizinden (`analyze_hints`): içe
// aktarılan adlar çözülür, ipucu yalnız açık dosyanın satırlarına konur.

fn example(rel: &str) -> (String, String) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel);
    let text = std::fs::read_to_string(&path).expect("örnek dosya");
    (path.to_string_lossy().into_owned(), text)
}

fn unit_hints(path: &str, text: &str) -> Vec<String> {
    let a = analysis::analyze_hints(path, text);
    rendered(
        text,
        &inlay_hints(&a, 0, text.len() as u32, HintConfig::default()),
    )
}

#[test]
fn imported_domains_give_the_vga_top_its_domain_hints() {
    // `@SysDomain`/`@PixDomain` vga_timing.volt'tan gelir; tek dosya
    // analizinde çözülmedikleri için modül çok saatli sayılmıyordu.
    let (path, text) = example("examples/vga/vga_top.volt");
    let got = unit_hints(&path, &text);
    // `rendered` tipi yazılı bildirimde tipi gösterir (`reg wx_r : u7`).
    assert_eq!(got.first().map(String::as_str), Some("u7 @SysDomain"));
    let count = |d: &str| got.iter().filter(|h| h.ends_with(d)).count();
    assert_eq!(
        (count("@SysDomain"), count("@PixDomain")),
        (5, 10),
        "{got:?}"
    );
}

#[test]
fn soc_top_is_clean_and_reads_types_from_used_files() {
    // Diskteki soc/top.volt'ta ipucu adayı yok (tek saat; tipsiz
    // `let`'lerin hepsi modül örneği) ama birim analizi hatasızdır —
    // tek dosya analizi `r.data.resp`'te hata verip modülü susturuyordu.
    let (path, text) = example("examples/soc/top.volt");
    let a = analysis::analyze_hints(&path, &text);
    assert!(a.editor_errors.is_empty(), "{:?}", a.editor_errors);
    assert!(a.typeck.is_some() && a.domain.is_some());
    // Editör tamponunda örnek çıkışını okuyan tipsiz `let`: tipi
    // bus.volt'taki BusDecoder'dan (AxiResp = u2) gelir.
    let edited = text.replace(
        "    aw.ready    =",
        "    let resp = dec_i.host_b_resp\n    let rdata = dec_i.host_r_data\n    aw.ready    =",
    );
    assert_eq!(unit_hints(&path, &edited), vec!["resp : u2", "rdata : u32"]);
}

/// Geçici birim: `lib.volt` (paket `lib`, modül Leaf) + onu kullanan
/// `top.volt`. Dönen yol top.volt'tur.
fn two_file_unit(tag: &str, leaf: &str, top: &str) -> (std::path::PathBuf, String) {
    let dir = std::env::temp_dir().join(format!("volt-lsp-unit-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    std::fs::write(dir.join("lib.volt"), leaf).expect("lib.volt");
    std::fs::write(dir.join("top.volt"), top).expect("top.volt");
    (dir, top.to_string())
}

const LEAF_OK: &str = "package lib;

pub module Leaf {
    in  a : u8
    out b : u8
    let t = a
    b = t
}
";

const TOP_USES_LEAF: &str = "use lib::Leaf;

module Top {
    in  x : u8
    out y : u8
    let leaf = Leaf { a: x }
    let r = leaf.b
    y = r
}

module Other {
    in  x : u8
    out y : u8
    let o = x
    y = o
}
";

#[test]
fn hints_stay_in_the_open_file_of_a_unit() {
    // lib.volt'taki `let t` ipucu adayıdır; top.volt açıkken yalnız
    // top.volt'un bildirimi ipucu alır, konumu top.volt'un kendi metninde.
    let (dir, top) = two_file_unit("ok", LEAF_OK, TOP_USES_LEAF);
    let path = dir.join("top.volt");
    let got = unit_hints(path.to_str().unwrap(), &top);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(got, vec!["r : u8", "o : u8"]);
}

#[test]
fn an_error_in_a_used_file_silences_the_whole_file() {
    // Leaf'in hatası (u8 → u4 daraltma) başka dosyada: Top'un `leaf.b`
    // ve bağımsız `r`, Leaf'i kullanmayan Other'ın `o` ipuçları da susar
    // (ADR-0091 modül dışı hata kuralı). Hatanın bayt ofseti top.volt'ta
    // Top'un aralığına düşer — dosya kimliği denetimi olmasa yalnız Top
    // susardı.
    let broken = LEAF_OK.replace("out b : u8", "out b : u4");
    let top = TOP_USES_LEAF.replace("let r = leaf.b", "let r = x\n    let q = leaf.b");
    let (dir, top) = two_file_unit("err", &broken, &top);
    let path = dir.join("top.volt");
    let a = analysis::analyze_hints(path.to_str().unwrap(), &top);
    let got = inlay_hints(&a, 0, top.len() as u32, HintConfig::default());
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        a.editor_errors.iter().any(|e| e.file != a.file_id),
        "hata lib.volt'ta olmalı: {:?}",
        a.editor_errors
    );
    let top_module = top.find("module Top").unwrap() as u32;
    let other_module = top.find("module Other").unwrap() as u32;
    assert!(
        a.editor_errors
            .iter()
            .any(|e| e.file != a.file_id && top_module < e.start && e.start < other_module),
        "{:?}",
        a.editor_errors
    );
    assert!(got.is_empty(), "{got:?}");
}

// ── fn gövdesi ────────────────────────────────────────────────────────

const FN_LETS: &str = "fn addw(a: u8, b: u8) -> u9 {
    let t = a + b
    let u : u9 = t
    u
}

module M {
    in  a : u8
    in  b : u8
    out y : u9
    y = addw(a, b)
}
";

#[test]
fn an_untyped_let_in_a_fn_body_gets_its_type() {
    // Yalnız tip ipucu; tipi yazılmış `u` tekrarlanmaz.
    assert_eq!(rendered(FN_LETS, &all(FN_LETS)), vec!["t : u9"]);
}

#[test]
fn an_error_in_a_fn_body_silences_the_whole_file() {
    // fn modül dışıdır: gövdesindeki hata (u9 → u4 daraltma) dosyanın
    // tamamını susturur — M'deki tipsiz `s` de ipucu almaz.
    let src = FN_LETS.replace("let u : u9 = t", "let u : u4 = t").replace(
        "y = addw(a, b)",
        "let s = a
    y = addw(s, b)",
    );
    let a = analysis::analyze_hints("test.volt", &src);
    assert!(!a.editor_errors.is_empty());
    assert!(all(&src).is_empty(), "{:?}", all(&src));
    // Hata kalkınca ikisi de ipucu alır.
    let fixed = src.replace("let u : u4 = t", "let u : u9 = t");
    assert_eq!(rendered(&fixed, &all(&fixed)), vec!["t : u9", "s : u8"]);
}
