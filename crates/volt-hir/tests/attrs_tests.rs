//! Uygulanmayan nitelik uyarısı testleri (ADR-0048, attrs.rs).
//!
//! Kapsam: ayrıştırılan ama hiçbir geçit tarafından yorumlanmayan
//! niteliklerin W0021 üretmesi, `@allow(unenforced)` ile susturma
//! (aynı düğüm ve öğe kapsamı), yorumlanan niteliklerin sessizliği,
//! hatalı `@allow` argümanı (E0009) ve Volt.toml `[lint]` politikası.

use volt_diagnostics::{Diagnostic, NoteKind};
use volt_hir::analyze;
use volt_hir::attrs::{check_attributes, UnenforcedLint, UNENFORCED_ATTRIBUTES};
use volt_syntax::{parse, FileId};

fn parse_clean(src: &str) -> volt_ast::SourceFile {
    let parsed = parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse hataları: {:?}",
        parsed.error_codes()
    );
    parsed.ast
}

fn diags(src: &str) -> Vec<Diagnostic> {
    analyze(&parse_clean(src)).diagnostics
}

fn count(diags: &[Diagnostic], code: &str) -> usize {
    diags.iter().filter(|d| d.code.as_str() == code).count()
}

const PASSTHROUGH: &str = "module M {\n    in  a : u8\n    out y : u8\n    y = a\n}\n";

fn with_attr(attr: &str) -> String {
    format!("{attr}\n{PASSTHROUGH}")
}

#[test]
fn version_on_module_warns_w0021() {
    let d = diags(&with_attr("@version(2)"));
    assert_eq!(count(&d, "W0021"), 1, "{d:?}");
    let w = d.iter().find(|d| d.code.as_str() == "W0021").unwrap();
    assert!(
        w.message.contains("@version"),
        "mesaj niteliği adlandırmalı: {}",
        w.message
    );
}

#[test]
fn enforced_timing_family_is_silent_since_adr_0054() {
    // ADR-0054: @timing / @false_path / @multicycle constraints.rs'te
    // yorumlanır — W0021 listesinden çıktılar (biçim hatası E0017'dir).
    let src = "@timing(clk = 25175000)\n@false_path(from = a, to = y)\n@multicycle(from = a, to = y, cycles = 2)\nmodule M {\n    in clk : clock\n    in a : u8\n    out y : u8\n    y = a\n}\n";
    let d = diags(src);
    assert_eq!(count(&d, "W0021"), 0, "{d:?}");
    for name in ["timing", "false_path", "multicycle"] {
        assert!(
            !UNENFORCED_ATTRIBUTES.contains(&name),
            "{name} listede olmamalı"
        );
    }
}

#[test]
fn budget_on_module_warns_w0021() {
    let d = diags(&with_attr("@budget(lut = 5000)"));
    assert_eq!(count(&d, "W0021"), 1, "{d:?}");
}

#[test]
fn abi_version_on_module_warns_w0021() {
    let d = diags(&with_attr("@abi_version(1)"));
    assert_eq!(count(&d, "W0021"), 1, "{d:?}");
}

#[test]
fn multicycle_on_register_statement_warns_w0021() {
    let src = "module M {\n    in clk : clock\n    in a : u8\n    out y : u8\n    @debug_trace\n    reg r : u8 = 0\n    on clk { r <= a }\n    y = r\n}\n";
    let d = diags(src);
    assert_eq!(count(&d, "W0021"), 1, "{d:?}");
}

#[test]
fn attribute_on_port_warns_w0021() {
    let src = "module M {\n    @debug_visible\n    in  a : u8\n    out y : u8\n    y = a\n}\n";
    let d = diags(src);
    assert_eq!(count(&d, "W0021"), 1, "{d:?}");
}

#[test]
fn every_unenforced_attribute_warns_once() {
    let attrs = UNENFORCED_ATTRIBUTES
        .iter()
        .map(|a| format!("@{a}"))
        .collect::<Vec<_>>()
        .join("\n");
    let d = diags(&with_attr(&attrs));
    assert_eq!(count(&d, "W0021"), UNENFORCED_ATTRIBUTES.len(), "{d:?}");
    for name in ["budget", "dft", "debug_visible", "synthesis_target"] {
        assert!(UNENFORCED_ATTRIBUTES.contains(&name), "{name} listede yok");
    }
}

#[test]
fn enforced_strict_timing_is_silent() {
    let d = diags(&with_attr("@strict_timing"));
    assert_eq!(count(&d, "W0021"), 0, "{d:?}");
}

#[test]
fn allow_unenforced_on_same_node_silences() {
    let d = diags(&with_attr("@budget(lut = 5000) @allow(unenforced)"));
    assert_eq!(count(&d, "W0021"), 0, "{d:?}");
    assert!(!d.iter().any(|d| !d.code.is_warning()), "{d:?}");
}

#[test]
fn allow_before_the_attribute_also_silences() {
    let d = diags(&with_attr("@allow(unenforced)\n@budget(lut = 5000)"));
    assert_eq!(count(&d, "W0021"), 0, "{d:?}");
}

#[test]
fn item_level_allow_covers_ports_and_body() {
    let src = "@allow(unenforced)\nmodule M {\n    in clk : clock\n    @debug_visible\n    in a : u8\n    out y : u8\n    @debug_trace\n    reg r : u8 = 0\n    on clk { r <= a }\n    y = r\n}\n";
    let d = diags(src);
    assert_eq!(count(&d, "W0021"), 0, "{d:?}");
}

#[test]
fn allow_on_port_does_not_cover_sibling_port() {
    let src = "module M {\n    @debug_visible @allow(unenforced)\n    in a : u8\n    @debug_visible\n    in b : u8\n    out y : u8\n    y = a & b\n}\n";
    let d = diags(src);
    assert_eq!(count(&d, "W0021"), 1, "{d:?}");
}

#[test]
fn bare_allow_is_silent() {
    let d = diags(&with_attr("@allow(unenforced)"));
    assert!(d.is_empty(), "{d:?}");
}

#[test]
fn allow_with_unknown_argument_is_e0009() {
    let d = diags(&with_attr("@budget(lut = 1) @allow(unenforcd)"));
    assert_eq!(count(&d, "E0009"), 1, "{d:?}");
    // Yazım hatalı allow susturmaz: W0021 görünür kalır.
    assert_eq!(count(&d, "W0021"), 1, "{d:?}");
}

#[test]
fn allow_without_argument_is_e0009() {
    let d = diags(&with_attr("@allow"));
    assert_eq!(count(&d, "E0009"), 1, "{d:?}");
}

#[test]
fn w0021_carries_reason_note_and_help() {
    let d = diags(&with_attr("@budget(lut = 5000)"));
    let w = d.iter().find(|d| d.code.as_str() == "W0021").unwrap();
    assert!(w.notes.iter().any(|n| n.kind == NoteKind::Reason), "{w:?}");
    assert!(w.notes.iter().any(|n| n.kind == NoteKind::Note), "{w:?}");
    let help = w.help.as_deref().unwrap_or("");
    assert!(
        help.contains("@allow(unenforced)"),
        "help susturmayı söylemeli: {help}"
    );
    assert!(w.validate().is_ok());
}

#[test]
fn budget_reason_mentions_e6001_and_note_lists_remaining() {
    let d = diags(&with_attr("@budget(lut = 5000)"));
    let w = d.iter().find(|d| d.code.as_str() == "W0021").unwrap();
    let reason = w
        .notes
        .iter()
        .find(|n| n.kind == NoteKind::Reason)
        .map(|n| n.text.as_str())
        .unwrap_or("");
    assert!(reason.contains("E6001"), "{reason}");
    // ADR-0054: not, hâlâ uygulanmayan nitelikleri listeler ve
    // zamanlama ailesinin artık uygulandığını söyler.
    let notes: Vec<&str> = w
        .notes
        .iter()
        .filter(|n| n.kind == NoteKind::Note)
        .map(|n| n.text.as_str())
        .collect();
    let listing = notes
        .iter()
        .find(|n| n.contains("still unenforced"))
        .expect("liste notu");
    for name in UNENFORCED_ATTRIBUTES {
        assert!(listing.contains(&format!("@{name}")), "{listing}");
    }
    assert!(listing.contains("--emit=sdc"), "{listing}");
}

#[test]
fn lint_allow_policy_silences_everything() {
    let ast = parse_clean(&with_attr("@version(1)\n@budget(lut = 1)"));
    let warn = check_attributes(&ast, UnenforcedLint::Warn);
    let allow = check_attributes(&ast, UnenforcedLint::Allow);
    assert_eq!(count(&warn, "W0021"), 2, "{warn:?}");
    assert_eq!(count(&allow, "W0021"), 0, "{allow:?}");
}

#[test]
fn lint_allow_policy_keeps_e0009() {
    let ast = parse_clean(&with_attr("@allow(bogus)"));
    let allow = check_attributes(&ast, UnenforcedLint::Allow);
    assert_eq!(count(&allow, "E0009"), 1, "{allow:?}");
}

fn ui_pass(rel: &str) -> Vec<Diagnostic> {
    let path = format!("{}/../../tests/ui/pass/{rel}", env!("CARGO_MANIFEST_DIR"));
    let src = std::fs::read_to_string(&path).expect("ui dosyası okunmalı");
    diags(&src)
}

#[test]
fn ui_pass_63_unenforced_attribute_warns_w0021_only() {
    let d = ui_pass("63_unenforced_attribute_warns.volt");
    assert_eq!(count(&d, "W0021"), 1, "{d:?}");
    assert_eq!(d.len(), 1, "yalnız W0021 beklenir: {d:?}");
}

#[test]
fn ui_pass_64_allow_unenforced_is_silent() {
    let d = ui_pass("64_allow_unenforced_silences.volt");
    assert!(d.is_empty(), "{d:?}");
}

#[test]
fn generic_module_instantiated_twice_warns_once() {
    // Monomorph klonları aynı kaynak konumunu taşır (ctx farklı):
    // kullanıcı bir @budget yazdı, bir uyarı görür.
    let src = "@budget(lut = 1)\nmodule Delay<const N: u32> {\n    in clk : clock\n    in x : u8\n    out y : u8\n    reg line : [u8; N] = [0; N]\n    on clk {\n        for i in 1..N { line[i] <= line[i - 1] }\n        line[0] <= x\n    }\n    y = line[N - 1]\n}\n\nmodule Top {\n    in clk : clock\n    in x : u8\n    out a : u8\n    out b : u8\n    let d4 = Delay<4> { clk, x }\n    let d2 = Delay<2> { clk, x }\n    a = d4.y\n    b = d2.y\n}\n";
    let d = diags(src);
    assert_eq!(count(&d, "W0021"), 1, "{d:?}");
}

#[test]
fn e0009_points_at_the_offending_extra_argument() {
    let src = with_attr("@allow(unenforced, extra)");
    let d = diags(&src);
    let e = d
        .iter()
        .find(|d| d.code.as_str() == "E0009")
        .expect("E0009");
    let span = e.primary_span().unwrap().span;
    let extra_at = src.find("extra").unwrap() as u32;
    assert_eq!(span.start, extra_at, "{e:?}");
}

#[test]
fn lint_policy_is_read_from_manifest_text() {
    assert_eq!(
        UnenforcedLint::from_manifest(
            "[package]\nname = \"p\"\n\n[lint]\nunenforced_attributes = \"allow\"  # ADR-0048\n"
        ),
        UnenforcedLint::Allow
    );
    assert_eq!(
        UnenforcedLint::from_manifest("[lint]\nunenforced_attributes = \"warn\"\n"),
        UnenforcedLint::Warn
    );
    // Bölüm dışındaki anahtar sayılmaz; tanınmayan değer warn'a düşer.
    assert_eq!(
        UnenforcedLint::from_manifest("[package]\nunenforced_attributes = \"allow\"\n"),
        UnenforcedLint::Warn
    );
    assert_eq!(
        UnenforcedLint::from_manifest("[lint]\nunenforced_attributes = \"alow\"\n"),
        UnenforcedLint::Warn
    );
}
