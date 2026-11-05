//! Ayrıştırıcının ikamesi kapsam zincirinden geçer (#130): tanılar.
//! Generic monomorfizasyonu (ADR-0041) ve modül düzeyi `for` açılımı
//! (ADR-0056) ad çözümünden önce koşar. Gölgeleme yine şartnamedeki
//! yerinde uyarı verir (name-resolution.md §6 "aynı kapsamda yasak, iç
//! kapsamda izinli + uyarı"): iç bildirimin satırında bir W1002, başka
//! yerde yok. Generic parametre kök öğeyi gölgelediğinde de W1002 (§5,
//! #122). Aynı kapsamdaki çift tanım E1003, desendeki döngü değişkeni
//! E1015 (§5.1), sabit beklenen yerdeki `let` E2005/E2008/E2021.
//!
//! Değerleri (simülasyon) `parser_scope_sim_tests` sınar; bu dosya
//! tanıları ve derlemenin başarısını sınar, SV metnini karşılaştırmaz.

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-scopediag-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// `volt check --format json`'un zarfı (tanılar `diagnostics`'te).
fn check_json(tag: &str, src: &str) -> Value {
    let dir = temp_dir(tag);
    let file = dir.join("case.volt");
    std::fs::write(&file, src).expect("yaz");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "check", "--format", "json"])
        .arg(&file)
        .current_dir(&dir)
        .output()
        .expect("volt çalışmalı");
    let _ = std::fs::remove_dir_all(&dir);
    serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("JSON değil ({e}): {}", String::from_utf8_lossy(&out.stderr)))
}

/// `volt check --format json`; (önem, kod, birincil satır) kaynak sırasıyla.
fn check(tag: &str, src: &str) -> Vec<(String, String, u64)> {
    let env = check_json(tag, src);
    let mut v: Vec<(String, String, u64)> = env["diagnostics"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .map(|d| {
            let line = d["spans"]
                .as_array()
                .and_then(|s| s.iter().find(|s| s["primary"] == true))
                .and_then(|s| s["start"]["line"].as_u64())
                .unwrap_or(0);
            (
                d["severity"].as_str().unwrap_or_default().to_string(),
                d["code"].as_str().unwrap_or_default().to_string(),
                line,
            )
        })
        .collect();
    v.sort_by_key(|d| d.2);
    v
}

fn lines_of(diags: &[(String, String, u64)], code: &str) -> Vec<u64> {
    diags
        .iter()
        .filter(|(_, c, _)| c == code)
        .map(|(_, _, l)| *l)
        .collect()
}

fn errors(diags: &[(String, String, u64)]) -> Vec<(String, u64)> {
    diags
        .iter()
        .filter(|(s, _, _)| s == "error")
        .map(|(_, c, l)| (c.clone(), *l))
        .collect()
}

/// Satırları 1'den sayılan kaynakta `marker` içeren satırların numaraları.
fn lines_with(src: &str, marker: &str) -> Vec<u64> {
    src.lines()
        .enumerate()
        .filter(|(_, l)| l.contains(marker))
        .map(|(i, _)| i as u64 + 1)
        .collect()
}

fn expected(src: &str, markers: &[&str]) -> Vec<u64> {
    let mut v: Vec<u64> = markers.iter().flat_map(|m| lines_with(src, m)).collect();
    v.sort_unstable();
    v
}

/// `volt build` başarılı.
fn builds(tag: &str, src: &str) {
    let dir = temp_dir(tag);
    let file = dir.join("case.volt");
    std::fs::write(&file, src).expect("yaz");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "build", "--target-dir"])
        .arg(dir.join("out"))
        .arg(&file)
        .current_dir(&dir)
        .output()
        .expect("volt çalışmalı");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// §6: iç kapsamdaki her gölgeleyen bildirimin satırında tek W1002;
/// gölgelemeyen bildirimde ve kullanımda yok. Şablon iki kez örneklenir
/// (G<3>, G<4>): uyarı kaynak satırı başına birdir.
#[test]
fn w1002_marks_every_shadowing_declaration_once_and_only_those() {
    let src = "const N : u32 = 7
const I : u32 = 1
const C : u8 = 5
module G<const N: u32> {
    in  s  : u8
    out y  : u8
    out ys : [u8; 2]
    out zs : [u8; 2]
    comb {
        let N = s
        y = N
    }
    comb { for N in 0..2 { ys[N] = N } }
    for N in 0..2 { zs[N] = N }
}
module Top {
    in  s  : u8
    in  i  : u8
    in  xs : [u8; 4]
    out y  : u8
    out z  : u8
    out w  : u8
    out v  : u8
    out ys : [u8; 4]
    out a  : [u8; 2]
    out b  : [u8; 2]
    out c  : [u8; 2]
    out d  : [u8; 2]
    let g = G<3> { s: s }
    let h = G<4> { s: s }
    y = g.y
    a = g.ys
    b = g.zs
    c = h.ys
    d = h.zs
    z = h.y ^ i
    for I in 0..2 { ys[I] = xs[I] }
    for i in 2..4 { ys[i] = xs[i] }
    for k in 0..1 {
        let k = s
        let C = k
        w = C
        for j in 0..1 {
            let C = s
            v = C
        }
    }
}
";
    let diags = check("w1002", src);
    let want = expected(
        src,
        &[
            "module G<const N: u32>", // generic N, kök const N (#122)
            "let N = s",              // blok let'i N, generic N
            "comb { for N in 0..2",   // comb döngü değişkeni N, generic N
            "    for N in 0..2 { zs", // modül düzeyi for N, generic N
            "for I in 0..2",          // modül düzeyi for I, kök const I
            "for i in 2..4",          // modül düzeyi for i, port i
            "let k = s",              // gövde let'i k, döngü değişkeni k
            "let C = k",              // gövde let'i C, kök const C
            "let C = s",              // iç gövdenin let'i C, dış gövdenin C'si
        ],
    );
    assert_eq!(lines_of(&diags, "W1002"), want, "{diags:?}");
    assert!(errors(&diags).is_empty(), "{diags:?}");
    builds("w1002", src);
}

/// Örneklenmemiş şablonda da aynı W1002 (çözücü şablonu görür): generic
/// parametre kök const'u gölgeler (#122).
#[test]
fn a_generic_parameter_of_an_uninstantiated_template_warns_too() {
    let src = "const N : u32 = 7
module G<const N: u32> {
    out y : u8
    y = N
}
module Top {
    out y : u8
    y = 0
}
";
    let diags = check("w1002_template", src);
    assert_eq!(
        lines_of(&diags, "W1002"),
        lines_with(src, "module G<"),
        "{diags:?}"
    );
}

/// Generic parametreyle aynı kapsamdaki port ve modül `let`'i çift tanım
/// (§5: generic parametreler modül kapsamının ilk bildirimleri; §6): E1003.
/// Önce: kabul, gövde portu değil 3'ü okuyordu.
#[test]
fn a_port_or_module_let_named_like_the_generic_is_a_duplicate() {
    let src = "module G<const N: u32> {
    in  N : u8
    out y : u8
    y = N
}
module H<const N: u32> {
    in  s : u8
    out y : u8
    let N = s
    y = N
}
module Top {
    in  s : u8
    out y : u8
    out z : u8
    let g = G<3> { N: s }
    let h = H<3> { s: s }
    y = g.y
    z = h.y
}
";
    let diags = check("e1003", src);
    assert_eq!(
        errors(&diags),
        vec![
            ("E1003".to_string(), lines_with(src, "in  N : u8")[0]),
            ("E1003".to_string(), lines_with(src, "let N = s")[0]),
        ],
        "{diags:?}"
    );
}

/// §5.1: desendeki ad değerdir ve sabit olmalı; döngü değişkeni E1015.
/// Modül düzeyi döngü değişkeni de, iç içe döngününki de (açılım verir;
/// çözücü döngüyü görmez). Önce: ikisi de literale ikame edilip kabul.
#[test]
fn a_module_level_loop_variable_in_a_pattern_is_e1015() {
    let src = "module M {
    in  s : u8
    out y : u8
    out z : u8
    for i in 0..1 {
        y = match s {
            i => 1,
            _ => 0,
        }
        for j in 0..1 {
            z = match s {
                j => 1,
                _ => 0,
            }
        }
    }
}
";
    let diags = check("e1015_loop", src);
    let want: Vec<(String, u64)> = ["i => 1", "j => 1"]
        .iter()
        .map(|m| ("E1015".to_string(), lines_with(src, m)[0]))
        .collect();
    assert_eq!(errors(&diags), want, "{diags:?}");
}

/// Generic parametreyi gölgeleyen blok `let`'i desende `let`'tir: E1015
/// (çözücü verir). Önce: generic'in değeriyle (3) kabul.
#[test]
fn a_block_let_shadowing_the_generic_in_a_pattern_is_e1015() {
    let src = "module G<const N: u32> {
    in  s : u8
    in  t : u8
    out y : u8
    comb {
        let N = t
        y = match s {
            N => 1,
            _ => 0,
        }
    }
}
module Top {
    in  s : u8
    out y : u8
    let g = G<3> { s: s, t: s }
    y = g.y
}
";
    let diags = check("e1015_let", src);
    assert_eq!(
        errors(&diags),
        vec![("E1015".to_string(), lines_with(src, "N => 1")[0])],
        "{diags:?}"
    );
}

/// Sabit beklenen yerde gölgeleyen `let` sabit değildir: tip genişliği
/// (E2005 blokta, E2021 modül düzeyi for gövdesinde; adı taze olan aynı
/// girdiyle aynı kod), generic argüman (E2008). Önce: generic'in ya da
/// döngü değişkeninin değeriyle kabul.
#[test]
fn a_shadowing_let_where_a_constant_is_required_is_rejected() {
    let g = "module G<const N: u32> {
    in  s : u8
    out y : u8
    comb {
        let N = s
        let t : uint<N> = 0
        y = s
    }
}
module Top {
    in  s : u8
    out y : u8
    let g = G<3> { s: s }
    y = g.y
}
";
    let diags = check("e2005", g);
    assert_eq!(
        errors(&diags),
        vec![("E2005".to_string(), lines_with(g, "let t : uint<N>")[0])],
        "{diags:?}"
    );
    let f = "module M {
    in  s : u8
    out y : u8
    for i in 1..2 {
        let i = s
        let t : uint<i> = 0
        y = s
    }
}
";
    let diags = check("e2021", f);
    assert_eq!(
        errors(&diags),
        vec![("E2021".to_string(), lines_with(f, "let t : uint<i>")[0])],
        "{diags:?}"
    );
    let w = "module W<const K: u32> {
    out y : u8
    y = K
}
module M {
    out y : u8
    for i in 0..1 {
        let i = 5
        let w = W<i> { }
        y = w.y
    }
}
";
    let diags = check("e2008", w);
    assert_eq!(
        errors(&diags),
        vec![("E2008".to_string(), lines_with(w, "W<i>")[0])],
        "{diags:?}"
    );
}

/// Modül düzeyi `for`'un sınırı `for`un kapsamında çözülür: kök const'u
/// gölgeleyen port sabit değildir, E2021 (const-eval.md §8). Önce: const'un
/// değeriyle açılıyordu.
#[test]
fn a_loop_bound_that_names_a_port_shadowing_a_const_is_not_constant() {
    let src = "const K : u32 = 2
module M {
    in  K : u8
    out y : [bool; 4]
    y[2] = false
    y[3] = false
    for i in 0..K {
        y[i] = true
    }
}
";
    let diags = check("bound", src);
    assert_eq!(
        errors(&diags),
        vec![("E2021".to_string(), lines_with(src, "for i in 0..K")[0])],
        "{diags:?}"
    );
}

/// Bekçi: gövde `let`'inden önceki kullanım E1002 olarak kalır (§4).
#[test]
fn a_use_before_the_body_let_is_still_e1002() {
    let src = "module M {
    in  s : u8
    out y : u8
    for i in 0..1 {
        y = a
        let a = s
    }
}
";
    let diags = check("e1002", src);
    assert_eq!(
        errors(&diags),
        vec![("E1002".to_string(), lines_with(src, "y = a")[0])],
        "{diags:?}"
    );
}

// ═══ Hedefli mutasyon koşusunun kör noktaları (ADR-0109, #130) ══════════

/// İç içe modül düzeyi `for`'un gövde `let`'i desende bir `let`'tir, döngü
/// değişkeni değil (§5.1): E1015'i çözücü verir ("'let' binding"); açılım
/// yalnız döngü değişkeni için verir.
#[test]
fn a_let_of_an_inner_module_level_loop_in_a_pattern_is_a_let() {
    let src = "module M {
    in  s : u8
    out y : u8
    for i in 0..1 {
        for j in 0..1 {
            let a = s
            y = match s {
                a => 1,
                _ => 0,
            }
        }
    }
}
";
    let env = check_json("e1015_inner_let", src);
    let msgs: Vec<String> = env["diagnostics"]
        .as_array()
        .expect("dizi")
        .iter()
        .filter(|d| d["code"] == "E1015")
        .map(|d| d["message"].as_str().unwrap_or_default().to_string())
        .collect();
    assert_eq!(msgs.len(), 1, "{env}");
    assert!(msgs[0].contains("'let' binding"), "{msgs:?}");
}

/// Açılım, çözücünün kendisinin göreceği gölgelemede W1002 vermez: modül
/// düzeyi `for`'daki `match` deyiminin (E0003) kolundaki iç `for`'un
/// değişkeni bir bloğun döngüsüdür, modül düzeyi değil. Girdi E0003 ile
/// reddedilir; açılımdan başka tanı çıkmaz.
#[test]
fn a_loop_inside_a_match_arm_is_not_a_module_level_loop() {
    let src = "const C : u32 = 1
module M {
    in  s : u8
    out y : u8
    y = s
    for i in 0..1 {
        match s {
            0 => {
                for C in 0..1 { }
            }
            _ => { }
        }
    }
}
";
    let diags = check("arm_for", src);
    let codes: Vec<&str> = diags.iter().map(|(_, c, _)| c.as_str()).collect();
    assert_eq!(codes, vec!["E0003"], "{diags:?}");
}

/// Tip konumundaki generic argüman `N<4>` çıplak parametre adı değildir:
/// ikame edilmez, örnekleme argümanı literal olmadığından E2008 (ADR-0041).
#[test]
fn a_type_argument_with_its_own_arguments_is_not_a_bare_parameter() {
    let src = "module B<const K: u32> {
    out y : u8
    y = K
}
module A<const N: u32> {
    out y : u8
    let b = B<N<4>> { }
    y = b.y
}
module Top {
    out y : u8
    let a = A<3> { }
    y = a.y
}
";
    let diags = check("type_arg", src);
    assert_eq!(
        errors(&diags),
        vec![("E2008".to_string(), lines_with(src, "B<N<4>>")[0])],
        "{diags:?}"
    );
}

/// Klonlanan dizi literali ve demet literali, elle yazılmış eşdeğeriyle
/// aynı tanıları verir (klonlama düğümü kaybetmez ya da çökmez).
#[test]
fn array_and_tuple_literals_in_a_clone_give_the_handwritten_diagnostics() {
    let generic = "module G<const N: u32> {
    in  a : u4
    in  b : u4
    out t : u4
    comb {
        let xs = [a, b]
        t = xs[N]
    }
    let tp : (u4, u4) = (a, b)
}
module Top {
    in  a : u4
    in  b : u4
    out t : u4
    let g = G<1> { a: a, b: b }
    t = g.t
}
";
    let handwritten = generic
        .replace("module G<const N: u32> {", "module G {")
        .replace("t = xs[N]", "t = xs[1]")
        .replace("let g = G<1> {", "let g = G {");
    let codes = |d: Vec<(String, String, u64)>| -> Vec<(String, u64)> {
        d.into_iter().map(|(_, c, l)| (c, l)).collect()
    };
    let g = codes(check("agg_generic", generic));
    let h = codes(check("agg_hand", &handwritten));
    assert_eq!(g, h);
    assert!(g.iter().any(|(c, _)| c == "E2005"), "{g:?}");
    assert!(g.iter().any(|(c, _)| c == "E0003"), "{g:?}");
}

/// Monomorf modülün `let`'inin saat alanı bildirimi (ADR-0088) klonda
/// kalır: `@Slow` `let`'e `@Fast` değer E3001.
#[test]
fn a_domain_annotation_on_a_let_survives_monomorphization() {
    let src = "domain Fast {}
domain Slow {}
module G<const N: u32> {
    in  fast_clk : clock @Fast
    in  slow_clk : clock @Slow
    in  a        : bool  @Fast
    out y        : bool  @Slow
    let s : bool @Slow = a
    y = s
}
module Top {
    in  fast_clk : clock @Fast
    in  slow_clk : clock @Slow
    in  a        : bool  @Fast
    out y        : bool  @Slow
    let g = G<1> { fast_clk: fast_clk, slow_clk: slow_clk, a: a }
    y = g.y
}
";
    let diags = check("domain_let", src);
    assert_eq!(
        errors(&diags),
        vec![(
            "E3001".to_string(),
            lines_with(src, "let s : bool @Slow")[0]
        )],
        "{diags:?}"
    );
}

/// Klonlanan desen argümanlarını taşır ve yapı deseni kısaltması kol
/// kapsamına ad bağlar (§5.1): yük taşıyan enum desteklenmez (E0003, iki
/// desen ve iki bildirim), kısaltma `N` generic'i gölgeler (W1002); elle
/// yazılmış eşdeğerle aynı tanılar.
#[test]
fn enum_patterns_in_a_clone_keep_their_arguments_and_bindings() {
    let generic = "enum Msg {
    Idle,
    Data { q: u8 },
    Pair(u8),
}
module G<const N: u32> {
    in  m : Msg
    out y : u8
    out z : u8
    y = match m {
        Msg::Data { N } => N,
        _ => 0,
    }
    z = match m {
        Msg::Pair(N) => 1,
        _ => 0,
    }
}
module Top {
    in  m : Msg
    out y : u8
    out z : u8
    let g = G<3> { m: m }
    y = g.y
    z = g.z
}
";
    let diags = check("payload", generic);
    let got: Vec<(String, u64)> = diags.iter().map(|(_, c, l)| (c.clone(), *l)).collect();
    let mut want = vec![
        (
            "W1002".to_string(),
            lines_with(generic, "Msg::Data { N }")[0],
        ),
        (
            "E0003".to_string(),
            lines_with(generic, "Msg::Data { N }")[0],
        ),
        ("E0003".to_string(), lines_with(generic, "Msg::Pair(N)")[0]),
        ("E0003".to_string(), lines_with(generic, "in  m : Msg")[0]),
        ("E0003".to_string(), lines_with(generic, "in  m : Msg")[1]),
    ];
    want.sort_by_key(|w| w.1);
    let mut got_sorted = got.clone();
    got_sorted.sort();
    want.sort();
    assert_eq!(got_sorted, want, "{got:?}");
}

/// W1002 gölgelenen tanımı da gösterir (ikincil etiket, §6): açılımın
/// verdiği W1002'de (döngü değişkeni kök const'u gölgeler) ve çözücününkinde
/// (generic parametre kök const'u gölgeler).
#[test]
fn w1002_labels_the_shadowed_definition() {
    let src = "const I : u32 = 1
const N : u32 = 2
module G<const N: u32> {
    out y : u8
    y = N
}
module M {
    in  xs : [u8; 2]
    out ys : [u8; 2]
    out y  : u8
    let g = G<3> { }
    y = g.y
    for I in 0..2 { ys[I] = xs[I] }
}
";
    let env = check_json("w1002_label", src);
    let diags = env["diagnostics"].as_array().expect("dizi");
    let secondaries: Vec<(u64, u64)> = diags
        .iter()
        .filter(|d| d["code"] == "W1002")
        .map(|d| {
            let spans = d["spans"].as_array().expect("span'ler");
            let line = |primary: bool| {
                spans
                    .iter()
                    .find(|s| s["primary"] == primary)
                    .and_then(|s| s["start"]["line"].as_u64())
                    .unwrap_or(0)
            };
            let label_ok = spans.iter().any(|s| {
                s["primary"] == false && s["label"].as_str() == Some("shadowed definition here")
            });
            assert!(label_ok, "{d}");
            (line(true), line(false))
        })
        .collect();
    let mut want = vec![
        (
            lines_with(src, "for I in")[0],
            lines_with(src, "const I")[0],
        ),
        (
            lines_with(src, "module G<")[0],
            lines_with(src, "const N : u32 = 2")[0],
        ),
    ];
    want.sort_unstable();
    let mut got = secondaries;
    got.sort_unstable();
    assert_eq!(got, want);
}

/// Açılım bütçesi (ADR-0068 §6) yeniden adlanan her bildirimin metnini bir
/// kez sayar: kaynak adı ve yineleme adı, `(yeni + kaynak) / 64` düğüm.
/// Dört 1000 karakterlik `let`'li bir yineleme ≈198 düğüm tutar, bütçe
/// 2^18 düğüm: 40 yineleme sığar (ad başına çarpım sayılsa sığmazdı), 1500
/// yineleme sığmaz (yalnız sonek sayılsa sığardı). Eşik bugün 1324; bu iki
/// değer ondan yeterince uzak.
#[test]
fn the_expansion_budget_counts_a_renamed_declaration_once() {
    let src = |count: usize| {
        let lets: String = (0..4)
            .map(|k| format!("        let n{k}_{} = s\n", "x".repeat(997)))
            .collect();
        format!(
            "module M {{\n    in  s : u8\n    out y : u8\n    y = s\n    for i in 0..{count} {{\n{lets}    }}\n}}\n"
        )
    };
    let small = check("budget_small", &src(40));
    assert!(!small.iter().any(|(_, c, _)| c == "E2027"), "{small:?}");
    let large = check("budget_large", &src(1500));
    assert!(large.iter().any(|(_, c, _)| c == "E2027"), "{large:?}");
}

/// Generic modüldeki deyim niteliği klonda kalır: `@multicycle(2)` monomorfun
/// SDC'sine girer (ADR-0054, ADR-0065: `set_multicycle_path` biçimi).
#[test]
fn a_statement_attribute_in_a_generic_module_reaches_the_constraints() {
    let src = "domain Fast {
    clock = posedge,
    reset = sync active_high,
    frequency = 100.mhz,
}
module G<const N: u32> {
    in  clk  : clock @Fast
    in  data : u8    @Fast
    out acc  : u8    @Fast
    @multicycle(2)
    reg acc_r : u8 = 0
    on clk { acc_r <= acc_r + data }
    acc = acc_r
}
pub module Top {
    in  clk  : clock @Fast
    in  data : u8    @Fast
    out acc  : u8    @Fast
    let g = G<1> { clk: clk, data: data }
    acc = g.acc
}
";
    let dir = temp_dir("multicycle");
    let file = dir.join("case.volt");
    std::fs::write(&file, src).expect("yaz");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "build", "--emit=sdc", "--target-dir"])
        .arg(dir.join("out"))
        .arg(&file)
        .current_dir(&dir)
        .output()
        .expect("volt çalışmalı");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let sdc = std::fs::read_to_string(dir.join("out").join("constraints").join("G_1.sdc"))
        .expect("G_1.sdc");
    assert!(
        sdc.contains("set_multicycle_path 2 -setup -to [get_cells {acc_r_reg*}]"),
        "{sdc}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
