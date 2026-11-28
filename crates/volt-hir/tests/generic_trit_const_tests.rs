//! Generic `const S : Trit` parametresi tip denetiminde Trit'tir (#468).
//!
//! Ayrıştırıcı parametreyi örnekte değerinin literaline çevirir (ADR-0041);
//! tip denetimi önce o literalin S'den geldiğini görmüyordu: `S * x`
//! (x : i8) literal çarpımı kuralıyla i16'ydı, `-S` bir literaldi, `x[S]`
//! tanısız kabul ediliyordu. Bakımcının kararı (#468): S'nin geçtiği her
//! ifade, aynı yerde bir Trit sinyali olsaydı alacağı tipi alır. Beklenen
//! tipler ve kodlar spec'ten elle: type-inference.md §3.3 (`Trit * iN` iN,
//! `Trit * Trit` Trit, `Trit ± Trit` i3, literal Trit'in tipini alır), §3.4
//! (`-Trit` Trit, `-x` i(N+1)), §3.5 ve ADR-0146/ADR-0147 (Trit ya da
//! işaretli indeks E2003), ADR-0142 (`~` Trit'te E2003). Trit dışındaki
//! generic const tiplerinin tiplenmesi değişmez (`const K : i8` literal
//! kalır).

use volt_ast::{ItemKind, StmtKind};
use volt_diagnostics::Diagnostic;
use volt_hir::{analyze, AnalysisResult};
use volt_syntax::{parse, FileId};

fn analyzed(src: &str) -> (volt_ast::SourceFile, AnalysisResult) {
    let parsed = parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse hataları: {:?}\n{src}",
        parsed.error_codes()
    );
    let result = analyze(&parsed.ast);
    (parsed.ast, result)
}

fn diags(src: &str) -> Vec<Diagnostic> {
    analyzed(src).1.diagnostics
}

/// Uyarılar hariç hata kodları, sıralı.
fn error_codes(src: &str) -> Vec<String> {
    let mut c: Vec<String> = diags(src)
        .iter()
        .filter(|d| d.code.as_str().starts_with('E'))
        .map(|d| d.code.as_str().to_string())
        .collect();
    c.sort();
    c
}

/// `module_name` modülündeki `let`'lerin (ad, değerinin tipi), sırayla.
fn let_types(src: &str, module_name: &str) -> Vec<(String, String)> {
    let (ast, result) = analyzed(src);
    let mut out = Vec::new();
    for &item in &ast.items {
        let ItemKind::Module(m) = &ast.items_arena[item].kind else {
            continue;
        };
        if m.name.text != module_name {
            continue;
        }
        for &s in &m.body {
            if let StmtKind::Let(l) = &ast.stmts[s].kind {
                let ty = result
                    .typeck
                    .expr_types
                    .get(&l.value)
                    .map(|&t| result.typeck.types.display(t))
                    .unwrap_or_else(|| "?".to_string());
                out.push((l.name.text.clone(), ty));
            }
        }
    }
    assert!(!out.is_empty(), "{module_name} modülünde let yok:\n{src}");
    out
}

/// `G<arg>` örneğinin gövdesi `body` olan tasarım; G'nin portları x : i8,
/// z : i2, t : Trit, c : bool, k : i8.
fn design(arg: &str, body: &str) -> String {
    format!(
        "module G<const S : Trit> {{
    in  x : i8
    in  z : i2
    in  t : Trit
    in  c : bool
    in  k : i8
    out o : bool
{body}
    o = c
}}

module Top {{
    in  x : i8
    in  z : i2
    in  t : Trit
    in  c : bool
    in  k : i8
    out o : bool
    let g = G<{arg}> {{ x: x, z: z, t: t, c: c, k: k }}
    o = g.o
}}
"
    )
}

/// S'li her biçim ve aynı yerde Trit sinyali `t` ile karşılığı; elle
/// beklenen tip.
const FORMS: &[(&str, &str, &str)] = &[
    // §3.3: Trit × iN → iN (ternary MAC), iki sırada; `-S` Trit olduğu
    // için `-S * x` de.
    ("S * x", "t * x", "i8"),
    ("x * S", "x * t", "i8"),
    ("-S * x", "-t * x", "i8"),
    ("S * z", "t * z", "i2"),
    // §3.4: `-z` (z : i8) i9.
    ("-(S * x)", "-(t * x)", "i9"),
    // §3.3: Trit × Trit kapalı; literal Trit'in tipini alır.
    ("S * t", "t * t", "Trit"),
    ("S * S", "t * t", "Trit"),
    ("S * 1", "t * 1", "Trit"),
    // §3.3: Trit ± Trit → i3; `t + 1` (§4: literal Trit'in tipini alır).
    ("S + t", "t + t", "i3"),
    ("S + 1", "t + 1", "i3"),
    // §3.4, §3.7: `-Trit` Trit; dalları Trit olan `if` Trit.
    ("-S", "-t", "Trit"),
    ("S", "t", "Trit"),
    ("if c { S } else { -S }", "if c { t } else { -t }", "Trit"),
    // §3.6: Trit → iN dönüşümü.
    ("S as i3", "t as i3", "i3"),
    // Bir işlemin işleneni olan çarpım iN'dir; `+` i9.
    ("S * x + k", "t * x + k", "i9"),
    // Karşılaştırma bool.
    ("S == 1", "t == 1", "bool"),
];

/// Her biçimin tipi, S = +1, 0 ve -1 için, elle beklenen tiptir ve aynı
/// yerdeki Trit sinyalinin tipiyle aynıdır. Önce (main) `S * x` i16, `-(S *
/// x)` i17, `-S`, `S`, `S * S`, `S + 1` literal, `if` i32 idi.
#[test]
fn every_form_with_s_has_the_type_of_the_same_form_with_a_trit_signal() {
    let mut body = String::new();
    for (k, (s, t, _)) in FORMS.iter().enumerate() {
        body.push_str(&format!("    let s{k} = {s}\n    let t{k} = {t}\n"));
    }
    for (arg, mono) in [("1", "G_1"), ("0", "G_0"), ("-1", "G_n1")] {
        let src = design(arg, &body);
        let got = let_types(&src, mono);
        for (k, (s, t, want)) in FORMS.iter().enumerate() {
            let s_ty = &got[2 * k].1;
            let t_ty = &got[2 * k + 1].1;
            assert_eq!(s_ty, want, "G<{arg}>: `{s}`");
            assert_eq!(t_ty, want, "G<{arg}>: `{t}`");
        }
        assert_eq!(error_codes(&src), Vec::<String>::new(), "G<{arg}>");
    }
}

/// S bir Trit'tir: Trit'te tanımsız işlemler ve Trit'i almayan hedefler
/// Trit sinyalininkiyle aynı kodu alır. Önce hepsi tanısız ya da başka
/// kodla geçiyordu (`S * u` u16, `S * 2` literal, `S << 1` literal, `w = S`
/// i8'e literal).
#[test]
fn an_operation_a_trit_does_not_have_is_rejected_with_s_as_with_a_trit_signal() {
    let cases: &[(&str, &str, &str, &[&str])] = &[
        // §3.3: Trit × uN tanımsız (yalnız Trit × iN).
        (
            "    in  u : u8",
            "    let v = S * u",
            "    let v = t * u",
            &["E2003"],
        ),
        // §4: Trit bağlamındaki literal {-1, 0, +1} olmalı.
        ("", "    let v = S * 2", "    let v = t * 2", &["E2011"]),
        // Trit kaydırılmaz.
        ("", "    let v = S << 1", "    let v = t << 1", &["E2003"]),
        // §5: Trit, i8'e atanamaz.
        ("    out w : i8", "    w = S", "    w = t", &["E2003"]),
        // §3.6: Trit → uN dönüşümü tanımsız; `-S` bir literal değildir
        // (literal olsaydı `-1 as u2` kesilip W2010 alırdı).
        (
            "    out w : u2",
            "    w = (-S) as u2",
            "    w = (-t) as u2",
            &["E2009"],
        ),
        // Trit çarpımı u1'e atanamaz, literal Trit'in kümesinde değil; `S * 3`
        // bir literal ağacı değildir (olsaydı değeri u1'e sığmaz, E2010).
        (
            "    out w : u1",
            "    w = S * 3",
            "    w = t * 3",
            &["E2003", "E2011"],
        ),
    ];
    for &(port, with_s, with_t, codes) in cases {
        let s_src = design("1", &format!("{port}\n{with_s}"));
        let t_src = design("1", &format!("{port}\n{with_t}"));
        assert_eq!(error_codes(&s_src), codes, "{with_s}");
        assert_eq!(error_codes(&t_src), codes, "{with_t}");
    }
}

/// ADR-0146 (Trit indeks) ve ADR-0147 (işaretli indeks): `x[S]`, `x[S +:
/// 2]` E2003; `x[S + 1 : S]`'in üst sınırı i3 (işaretli), alt sınırı Trit:
/// iki E2003. Önce dördü de tanısız kabul ediliyordu (ADR-0146 Sınırlar).
#[test]
fn a_generic_trit_index_is_e2003() {
    let src = "module P<const S : Trit> {
    in  x : u8
    out a : bool
    out b : bits<2>
    out c : bits<2>
    a = x[S]
    b = x[S + 1 : S]
    c = x[S +: 2]
}

module M {
    in  x : u8
    out a : bool
    out b : bits<2>
    out c : bits<2>
    let p = P<1> { x: x }
    a = p.a
    b = p.b
    c = p.c
}
";
    let d = diags(src);
    let errors: Vec<(&str, &str)> = d
        .iter()
        .filter(|d| d.code.as_str().starts_with('E'))
        .map(|d| (d.code.as_str(), d.message.as_str()))
        .collect();
    assert_eq!(
        errors,
        [
            ("E2003", "an index must not be a Trit, found 'Trit'"),
            ("E2003", "an index must be unsigned, found 'i3'"),
            ("E2003", "an index must not be a Trit, found 'Trit'"),
            ("E2003", "an index must not be a Trit, found 'Trit'"),
        ],
        "{d:#?}"
    );
}

/// ADR-0142: `~S` E2003'tür ve bir kez raporlanır (S'nin Trit tipi tek
/// yoldan: işlenenin tipi).
#[test]
fn bit_not_of_a_generic_trit_is_one_e2003() {
    let src = design("1", "    out n : Trit\n    n = ~S");
    let d = diags(src.as_str());
    let errors: Vec<(&str, &str)> = d
        .iter()
        .filter(|d| d.code.as_str().starts_with('E'))
        .map(|d| (d.code.as_str(), d.message.as_str()))
        .collect();
    assert_eq!(
        errors,
        [("E2003", "operator '~' is not defined for 'Trit'")],
        "{d:#?}"
    );
}

/// ADR-0003: Trit'in değer kümesi {-1, 0, +1}. Kümenin dışındaki argüman
/// (`G<2>`, `G<-2>`) Trit bağlamındaki literal gibi E2011 alır. Önce `G<2>`
/// tanısız kabul ediliyordu ve `S * x` 2x veriyordu.
#[test]
fn a_generic_trit_argument_outside_the_value_set_is_e2011() {
    for arg in ["2", "-2"] {
        let src = design(arg, "    out y : i8\n    y = S * x");
        assert_eq!(error_codes(&src), vec!["E2011"], "G<{arg}>");
    }
}

/// Kapsam: Trit dışındaki generic const tiplerinin tiplenmesi değişmez.
/// `const K : i8` ve `const W : u32` parametresi literal kalır: `K * x` (x
/// : i8) literal × i8 kuralıyla i16, `-K` literal, `W + a` (a : u8) u9;
/// `x[K]` tanısız (ADR-0147 Sınırlar).
#[test]
fn other_generic_const_types_keep_the_literal_typing() {
    let src = "module Q<const K : i8, const W : u32> {
    in  x : i8
    in  a : u8
    in  u : u8
    out o : bool
    out b : bool
    let p = K * x
    let n = -K
    let q = W + a
    b = u[K]
    o = true
}

module Top {
    in  x : i8
    in  a : u8
    in  u : u8
    out o : bool
    out b : bool
    let q = Q<3, 3> { x: x, a: a, u: u }
    o = q.o
    b = q.b
}
";
    let want = [
        ("p".to_string(), "i16".to_string()),
        ("n".to_string(), "integer literal".to_string()),
        ("q".to_string(), "u9".to_string()),
    ];
    assert_eq!(let_types(src, "Q_3_3"), want);
    assert_eq!(error_codes(src), Vec::<String>::new());
    // #471: `Q<-1, 3>` yazılabilir; K yine literaldir. `u[K]` K = -1 için
    // sabit indeksin sınır denetimi E2006 verir (değer denetlenir, tip
    // değil; ADR-0147 Sınırlar).
    let src = src.replace("Q<3, 3>", "Q<-1, 3>");
    assert_eq!(let_types(&src, "Q_n1_3"), want);
    assert_eq!(error_codes(&src), vec!["E2006"]);
}
