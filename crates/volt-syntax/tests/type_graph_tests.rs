//! Tip çizgesi denetimi (ADR-0069): özyineli tipler E4009, generic
//! `struct port` E0003.
//!
//! Issue #21'in iki bulgusu — sade özyineli struct ve generic struct port —
//! ve aynı sınıfın taraması (karşılıklı, dizi/demet, enum payload'ı, tip
//! takma adı, generic, Handshake payload'ı, bundle üzerinden) eskiden
//! `volt check`'ten tanısız geçiyordu.

use std::time::{Duration, Instant};

use volt_ast::{ItemKind, ModuleDecl};
use volt_diagnostics::Severity;
use volt_span::FileId;
use volt_syntax::parser::{parse, parse_unit, ParseResult};

fn p(src: &str) -> ParseResult {
    parse(FileId(0), src)
}

fn count(res: &ParseResult, code: &str) -> usize {
    res.error_codes().iter().filter(|c| **c == code).count()
}

fn module<'a>(res: &'a ParseResult, name: &str) -> &'a ModuleDecl {
    res.ast
        .items
        .iter()
        .find_map(|&i| match &res.ast.items_arena[i].kind {
            ItemKind::Module(m) if m.name.text == name => Some(m),
            _ => None,
        })
        .expect("modül bulunmalı")
}

fn port_names(res: &ParseResult, name: &str) -> Vec<String> {
    module(res, name)
        .ports
        .iter()
        .map(|p| p.name.text.clone())
        .collect()
}

/// E4009 tanısının notları (döngü yolu ilk nottur).
fn cycle_notes(res: &ParseResult) -> Vec<String> {
    res.diagnostics
        .iter()
        .filter(|d| d.code.as_str() == "E4009")
        .map(|d| d.notes[0].text.clone())
        .collect()
}

// ═══ Fixture'lar: her `//~^ ERROR <KOD>` bir üst satırda birincil konumlu
// tam bir tanıya karşılık gelir, işaretsiz hata yoktur ═══════════════════

fn line_of(src: &str, offset: u32) -> usize {
    src[..offset as usize].matches('\n').count() + 1
}

fn expected_errors(src: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (i, l) in src.lines().enumerate() {
        if let Some(code) = l.trim().strip_prefix("//~^ ERROR ") {
            out.push((i, code.trim().to_string()));
        }
    }
    out
}

fn assert_fixture_exact(file: &str) {
    let path = format!("{}/../../tests/ui/fail/{file}", env!("CARGO_MANIFEST_DIR"));
    let src = std::fs::read_to_string(&path).expect("fixture okunmalı");
    let res = p(&src);
    let mut actual: Vec<(usize, String)> = res
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| {
            let primary = d.spans.iter().find(|s| s.primary).expect("birincil konum");
            (
                line_of(&src, primary.span.start),
                d.code.as_str().to_string(),
            )
        })
        .collect();
    actual.sort();
    let expected = expected_errors(&src);
    assert!(!expected.is_empty(), "{file}: anotasyon yok");
    assert_eq!(actual, expected, "{file}: (satır, kod) eşleşmeli");
}

#[test]
fn fixture_74_recursive_struct() {
    assert_fixture_exact("74_recursive_struct.volt");
}

#[test]
fn fixture_75_recursive_struct_mutual() {
    assert_fixture_exact("75_recursive_struct_mutual.volt");
}

#[test]
fn fixture_76_recursive_type_through_array_and_tuple() {
    assert_fixture_exact("76_recursive_type_container.volt");
}

#[test]
fn fixture_77_recursive_enum_payloads_and_base_type() {
    assert_fixture_exact("77_recursive_enum.volt");
}

#[test]
fn fixture_78_recursive_type_aliases() {
    assert_fixture_exact("78_recursive_type_alias.volt");
}

#[test]
fn fixture_79_recursive_generic_structs() {
    assert_fixture_exact("79_recursive_generic_struct.volt");
}

#[test]
fn fixture_80_handshake_payload_recursive_through_array() {
    assert_fixture_exact("80_recursive_handshake_payload.volt");
}

#[test]
fn fixture_81_struct_port_recursive_through_plain_struct_and_array() {
    assert_fixture_exact("81_recursive_bundle_via_plain.volt");
}

#[test]
fn fixture_82_generic_struct_port_is_e0003() {
    assert_fixture_exact("82_generic_struct_port.volt");
}

#[test]
fn fixture_72_recursive_struct_port_is_still_e4009() {
    // ADR-0067 fixture'ı: anotasyonu kapatan alanda, birincil konum struct
    // adında — kod ve sayı değişmedi.
    let path = format!(
        "{}/../../tests/ui/fail/72_recursive_struct_port.volt",
        env!("CARGO_MANIFEST_DIR")
    );
    let res = p(&std::fs::read_to_string(path).expect("fixture"));
    assert_eq!(res.error_codes(), vec!["E4009"]);
}

// ═══ Tanı biçimi: döngüdeki her tip, kapatan üye, deterministik yol ═══

#[test]
fn mutual_cycle_reports_every_type_with_its_path_in_declaration_order() {
    let res =
        p("struct A { b : B }\nstruct B { c : C }\nstruct C { a : A }\nmodule M { in x : u8 }");
    assert_eq!(count(&res, "E4009"), 3, "{:?}", res.error_codes());
    assert_eq!(
        cycle_notes(&res),
        vec![
            "cycle: A.b → B.c → C.a → A",
            "cycle: B.c → C.a → A.b → B",
            "cycle: C.a → A.b → B.c → C",
        ]
    );
    let d = &res.diagnostics[0];
    assert!(
        d.message.contains("struct 'A' contains itself (field 'b'"),
        "{}",
        d.message
    );
    assert_eq!(d.spans.len(), 2, "birincil ad + ikincil kapatan alan");
    assert!(d.help.is_some());
}

#[test]
fn diagnostics_are_identical_across_runs() {
    let src = "type T = U\ntype U = [T; 2]\nenum E { A, V(S) }\nstruct S { e : E, t : T }\nmodule M { in x : u8 }";
    let first: Vec<String> = p(src)
        .diagnostics
        .iter()
        .map(|d| format!("{d:?}"))
        .collect();
    for _ in 0..5 {
        let again: Vec<String> = p(src)
            .diagnostics
            .iter()
            .map(|d| format!("{d:?}"))
            .collect();
        assert_eq!(again, first);
    }
}

#[test]
fn a_type_that_only_refers_to_a_cycle_is_not_reported() {
    // Outer döngüye ULAŞIR ama döngüde değildir: tanı yok (kaskad yok).
    let res = p("struct P { f : P }\nstruct Outer { p : P }\nmodule M { in x : Outer }");
    assert_eq!(count(&res, "E4009"), 1);
    assert!(res.diagnostics[0].message.contains("'P'"));
}

#[test]
fn unused_recursive_type_is_still_an_error() {
    let res = p("struct P { d : u8, f : P }\nmodule M { in a : u8\n out y : u8\n y = a }");
    assert_eq!(res.error_codes(), vec!["E4009"]);
}

// ═══ Kesinlik: sonlu tipler reddedilmez ════════════════════════════

#[test]
fn finite_types_are_not_e4009() {
    let res = p(concat!(
        "struct Inner { a : u8 }\n",
        "struct Outer { x : Inner, y : Inner }\n", // elmas
        "struct Pair<T> { l : T, r : T }\n",
        "type Nested = Pair<Pair<Outer>>\n", // generic iç içe
        "struct Tag<T> { v : u8 }\n",
        "struct Node { t : Tag<Node> }\n", // Tag T'yi taşımaz
        "enum Cmd { Nop, Load(Inner) }\n",
        "type W = [Outer; 4]\n",
        "module M { in x : u8 }",
    ));
    assert!(res.diagnostics.is_empty(), "{:?}", res.error_codes());
}

#[test]
fn generic_argument_is_an_edge_only_when_the_parameter_is_stored() {
    let stored = p("struct Box<T> { v : T }\nstruct P { b : Box<P> }\nmodule M { in x : u8 }");
    assert_eq!(count(&stored, "E4009"), 1, "{:?}", stored.error_codes());
    let nested =
        p("struct Box<T> { v : [T; 2] }\nstruct P { b : Box<(u8, P)> }\nmodule M { in x : u8 }");
    assert_eq!(count(&nested, "E4009"), 1, "{:?}", nested.error_codes());
    let phantom = p("struct Box<T> { v : u8 }\nstruct P { b : Box<P> }\nmodule M { in x : u8 }");
    assert!(
        phantom.diagnostics.is_empty(),
        "{:?}",
        phantom.error_codes()
    );
}

#[test]
fn a_generic_parameter_shadows_a_type_of_the_same_name() {
    // `T` burada parametredir, üst düzeydeki `struct T` değil.
    let res = p("struct T { w : W<u8> }\nstruct W<T> { x : T }\nmodule M { in x : u8 }");
    assert!(res.diagnostics.is_empty(), "{:?}", res.error_codes());
}

// ═══ Açılım: döngüye ulaşan tipler açılmaz ════════════════════════

#[test]
fn handshake_payload_recursive_through_array_stays_opaque() {
    let res = p("struct P { d : u8, f : [P; 2] }\nmodule M {\n    in clk : clock\n    in x : Handshake<P>\n}");
    assert_eq!(count(&res, "E4009"), 1, "{:?}", res.error_codes());
    assert_eq!(
        port_names(&res, "M"),
        vec!["clk", "x_data", "x_valid", "x_ready"]
    );
}

#[test]
fn struct_port_recursive_through_plain_struct_is_not_flattened() {
    let res = p("struct port A { out p : P }\nstruct P { a : A }\nmodule M { in x : A }");
    assert_eq!(count(&res, "E4009"), 2, "{:?}", res.error_codes());
    assert_eq!(port_names(&res, "M"), vec!["x"]);
}

#[test]
fn struct_port_recursive_through_array_field_is_not_flattened() {
    let res = p("struct port S { out d : u8\n out x : [S; 2] }\nmodule M { in s : S }");
    assert_eq!(count(&res, "E4009"), 1, "{:?}", res.error_codes());
    assert_eq!(port_names(&res, "M"), vec!["s"]);
}

// ═══ Generic struct port (E0003) ═══════════════════════════════════

#[test]
fn generic_struct_port_is_e0003_once_even_when_unused() {
    let used =
        p("struct port G<T> { out d : T }\nmodule M { in g : G<u8>\n out o : u8\n o = g.d }");
    assert_eq!(used.error_codes(), vec!["E0003"]);
    assert!(used.diagnostics[0]
        .message
        .contains("generic struct ports are not supported yet"));
    let unused = p("struct port G<T> { out d : T }\nmodule M { in a : u8 }");
    assert_eq!(unused.error_codes(), vec!["E0003"]);
    let konst = p("struct port B<const W: u32> { out d : bits<W> }\nmodule M { in a : u8 }");
    assert_eq!(konst.error_codes(), vec!["E0003"]);
}

#[test]
fn generic_plain_struct_is_not_e0003() {
    // Yalnız `struct port` açılır; sade generic struct bu ADR'nin konusu değil.
    let res = p("struct G<T> { d : T }\nmodule M { in a : u8 }");
    assert!(res.diagnostics.is_empty(), "{:?}", res.error_codes());
}

// ═══ Çoklu dosya ve ölçek ══════════════════════════════════════════

#[test]
fn cycle_across_files_of_one_unit_is_found() {
    let a = "struct A { b : B }\n";
    let b = "struct B { a : A }\nmodule M { in x : u8 }\n";
    let res = parse_unit(&[(FileId(0), a), (FileId(1), b)]);
    assert_eq!(count(&res, "E4009"), 2, "{:?}", res.error_codes());
}

fn ring(n: usize, port: bool) -> String {
    let (kw, dir) = if port {
        ("struct port", "out ")
    } else {
        ("struct", "")
    };
    let mut s = String::new();
    for i in 0..n {
        s.push_str(&format!("{kw} S{i} {{ {dir}n : S{} }}\n", (i + 1) % n));
    }
    s.push_str("module M { in a : S0 }\n");
    s
}

#[test]
fn large_cycle_is_linear_and_reports_component_size() {
    // ADR-0067'nin find_cycles'ı halkada süper-doğrusaldı (2000 struct
    // port: 0,79 s release); Tarjan + sınırlı yol notu doğrusal.
    for port in [false, true] {
        let src = ring(3000, port);
        let t = Instant::now();
        let res = p(&src);
        assert!(t.elapsed() < Duration::from_secs(3), "{:?}", t.elapsed());
        assert_eq!(count(&res, "E4009"), 3000);
        assert_eq!(cycle_notes(&res)[0], "cycle: 3000 types");
        assert_eq!(port_names(&res, "M"), vec!["a"]);
    }
}

#[test]
fn small_cycle_note_shows_the_full_path() {
    let res = p(&ring(64, false));
    let notes = cycle_notes(&res);
    assert!(
        notes[0].starts_with("cycle: S0.n → S1.n → "),
        "{}",
        notes[0]
    );
    assert!(notes[0].ends_with("S63.n → S0"), "{}", notes[0]);
}

#[test]
fn long_acyclic_chain_does_not_overflow_the_stack() {
    let mut src = String::new();
    for i in 0..20_000 {
        src.push_str(&format!("struct S{i} {{ n : S{} }}\n", i + 1));
    }
    src.push_str("struct S20000 { d : u8 }\nmodule M { in x : u8 }\n");
    let res = p(&src);
    assert!(res.diagnostics.is_empty(), "{:?}", &res.error_codes()[..3]);
}

#[test]
fn struct_port_that_only_reaches_a_cycle_is_not_flattened() {
    // Wrap döngüde değil (tanı yok) ama döngüye ULAŞIR: açılırsa `w_r`
    // anlamsız bir yaprak olurdu. Port olduğu gibi kalır (ADR-0067 kuralı).
    let res = p(
        "struct port Req { in r : Req }\nstruct port Wrap { in r : Req }\nmodule M { in w : Wrap }",
    );
    assert_eq!(count(&res, "E4009"), 1, "{:?}", res.error_codes());
    assert_eq!(port_names(&res, "M"), vec!["w"]);
}
