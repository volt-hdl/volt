//! Monomorfizasyon (ADR-0041) uçtan uca: parse → analyze; monomorf
//! modüller isim çözümleme, const eval ve tip denetiminden geçmeli.

use volt_hir::{analyze, Ty};
use volt_span::FileId;
use volt_syntax::parse;

const GENERIC_ACC: &str = r#"
module Acc<const N: u32, const W: u32> {
    in  clk : clock
    in  x   : sint<W>
    out y   : sint<W>
    reg buf : [sint<W>; N] = [0; N]
    on clk {
        for i in 1..N { buf[i] <= buf[i - 1] }
        buf[0] <= x
    }
    y = buf[N - 1]
}
module Top {
    in  clk : clock
    in  a   : i16
    out b   : i16
    let f = Acc<4, 16> { clk, x: a }
    b = f.y
}
"#;

fn error_codes(src: &str) -> Vec<String> {
    let parsed = parse(FileId(0), src);
    let mut codes: Vec<String> = parsed
        .diagnostics
        .iter()
        .map(|d| d.code.as_str().to_string())
        .collect();
    let result = analyze(&parsed.ast);
    codes.extend(
        result
            .diagnostics
            .iter()
            .filter(|d| d.severity == volt_diagnostics::Severity::Error)
            .map(|d| d.code.as_str().to_string()),
    );
    codes
}

#[test]
fn generic_module_instantiated_with_literals_analyzes_cleanly() {
    assert!(
        error_codes(GENERIC_ACC).is_empty(),
        "{:?}",
        error_codes(GENERIC_ACC)
    );
}

#[test]
fn monomorph_port_type_resolves_to_concrete_width() {
    let parsed = parse(FileId(0), GENERIC_ACC);
    let result = analyze(&parsed.ast);
    // Acc_4_16'nın `x` portu: sint<W> → i16.
    let acc = parsed
        .ast
        .items
        .iter()
        .find_map(|&i| match &parsed.ast.items_arena[i].kind {
            volt_ast::ItemKind::Module(m) if m.name.text == "Acc_4_16" => Some(m),
            _ => None,
        })
        .expect("Acc_4_16 üretilmeli");
    let def = result
        .resolve
        .decl_spans
        .get(&acc.ports[1].name.span)
        .expect("port tanımı");
    let ty = result.typeck.def_types.get(def).expect("port tipi");
    assert_eq!(*result.typeck.types.ty(*ty), Ty::SInt { width: 16 });
}

#[test]
fn wrong_arity_is_reported_at_parse_time() {
    let src = GENERIC_ACC.replace("Acc<4, 16>", "Acc<4>");
    let parsed = parse(FileId(0), &src);
    assert_eq!(parsed.error_codes(), vec!["E2003"]);
}

#[test]
fn two_instances_with_same_arguments_share_one_monomorph_and_typecheck() {
    // BİLİNEN SINIR: FARKLI argümanlı iki monomorf aynı dosyada resolve'un
    // Span anahtarlı decl_spans/use_spans tablolarında çakışır (klonlar
    // kaynak span'lerini paylaşır) → sahte E4001. Düzeltme resolve.rs'te
    // (span → (modül, span) anahtarı); burada yalnız paylaşım sınanır.
    let src = GENERIC_ACC
        .replace(
            "    b = f.y\n",
            "    let g = Acc<4, 16> { clk, x: a }\n    b = f.y\n    c = g.y\n",
        )
        .replace(
            "    out b   : i16\n",
            "    out b   : i16\n    out c   : i16\n",
        );
    let codes = error_codes(&src);
    assert!(codes.is_empty(), "{codes:?}");
}

#[test]
fn plain_file_without_generics_is_unaffected() {
    let src = "module Counter { in clk : clock  out q : u8\n\
               reg c : u8 = 0\n on clk { c <= c + 1 }\n q = c }";
    assert!(error_codes(src).is_empty());
}

const TWO_WIDTHS: &str = r#"
module Acc<const N: u32, const W: u32> {
    in  clk : clock
    in  x   : sint<W>
    out y   : sint<W>
    reg buf : [sint<W>; N] = [0; N]
    on clk {
        for i in 1..N { buf[i] <= buf[i - 1] }
        buf[0] <= x
    }
    y = buf[N - 1]
}
module Top {
    in  clk : clock
    in  a   : i16
    out b   : i16
    out c   : i8
    let f = Acc<4, 16> { clk, x: a }
    let g = Acc<2, 8> { clk, x: a as i8 }
    b = f.y
    c = g.y
}
"#;

#[test]
fn two_monomorphs_with_different_arguments_analyze_cleanly() {
    // Klon span'leri `ctx` ile etiketlenir: decl_spans/use_spans çakışmaz,
    // sahte E4001 çıkmaz.
    let codes = error_codes(TWO_WIDTHS);
    assert!(codes.is_empty(), "{codes:?}");
}

#[test]
fn diagnostics_in_both_monomorphs_point_at_the_template_source_line() {
    // `buf[N]` her iki monomorfta sınır dışıdır (E2006); iki tanı da
    // şablonun satırını göstermeli — ctx etiketi satır/sütunu değiştirmez.
    let src = TWO_WIDTHS.replace("y = buf[N - 1]", "y = buf[N]");
    let parsed = parse(FileId(0), &src);
    assert!(parsed.diagnostics.is_empty());
    let result = analyze(&parsed.ast);
    let mut map = volt_span::SourceMap::new();
    map.add_file("t.volt", src.clone());
    let expected_line = src
        .lines()
        .position(|l| l.contains("y = buf[N]"))
        .expect("satır") as u32
        + 1;
    let lines: Vec<(u32, u16)> = result
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == "E2006")
        .filter_map(|d| d.primary_span())
        .map(|s| (map.line_col_utf8(s.span).0, s.span.ctx))
        .collect();
    assert_eq!(lines.len(), 2, "{:?}", result.diagnostics);
    assert!(lines.iter().all(|&(l, _)| l == expected_line), "{lines:?}");
    // İki monomorf farklı bağlam taşır.
    assert_ne!(lines[0].1, lines[1].1);
    assert!(lines.iter().all(|&(_, c)| c != 0));
}
