//! İsim çözümleme testleri (name-resolution.md §12 test vektörleri).

use volt_hir::{analyze, resolve::closest_match, DefKind};
use volt_syntax::{parse, FileId};

/// Kaynağı ayrıştırır (hatasız olmalı) ve anlamsal analizi koşturur.
fn check(src: &str) -> volt_hir::AnalysisResult {
    let parsed = parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "test kaynağı ayrışmalı: {:?}",
        parsed.error_codes()
    );
    analyze(&parsed.ast)
}

fn codes(src: &str) -> Vec<&'static str> {
    check(src).error_codes()
}

fn assert_clean_of_errors(src: &str) {
    let result = check(src);
    assert!(
        !result.has_errors(),
        "hata beklenmiyordu: {:?}",
        result.error_codes()
    );
}

// ═══ Çözülen durumlar (spec §12: ✓) ═══════════════════════════════

#[test]
fn port_reference_resolves() {
    assert_clean_of_errors("module M { in a : u8 out y : u8 y = a }");
}

#[test]
fn forward_module_reference_resolves() {
    // Öğe seviyesinde ileri referans SERBEST (spec §4).
    assert_clean_of_errors(
        "module Top { in x : u8 out y : u9 let u = Alt { a: x } y = u.s }
         module Alt { in a : u8 out s : u9 s = a + 1 }",
    );
}

#[test]
fn builtin_sync_resolves() {
    assert_clean_of_errors("module M { in clk : clock in a : u8 out y : u8 y = sync(a, clk) }");
}

#[test]
fn builtin_clog2_resolves() {
    assert_clean_of_errors(
        "const DEPTH : u32 = 64;\nmodule M { in a : bits<clog2(DEPTH)> out y : bits<6> y = a }",
    );
}

#[test]
fn all_nine_builtins_resolve() {
    // Prelude'un tamamı görünür olmalı (spec §7).
    let result = check(
        "module M { in clk : clock in a : u8 out y : u8
           y = sync(a, clk) & sync3(a, clk) & zext(a) & sext(a) & trunc(a)
             & concat(a, a) & replicate(a, 2) & popcount(a) & clog2(8)
         }",
    );
    assert!(
        !result.error_codes().contains(&"E1001"),
        "yerleşikler çözülmeli: {:?}",
        result.error_codes()
    );
}

#[test]
fn reg_wire_let_resolve() {
    assert_clean_of_errors(
        "module M { in clk : clock in a : u8 out y : u8
           wire w : u8
           reg r : u8 = 0
           let t = a + 1
           w = t
           on clk { r <= w }
           y = r
         }",
    );
}

#[test]
fn enum_variant_resolves() {
    assert_clean_of_errors(
        "enum Durum : bits<2> { Bekle = 0, Calis = 1 }
         module M { in x : u8 out y : u8
           y = match x { 0 => 1, _ => 2 }
         }
         const S : Durum = Durum::Bekle;",
    );
}

#[test]
fn enum_variant_in_match_resolves() {
    assert_clean_of_errors(
        "enum Durum : bits<2> { Bekle = 0, Calis = 1 }
         module M { in x : u8 out y : u8
           y = match x { Durum::Bekle => 0, _ => 1 }
         }",
    );
}

#[test]
fn instance_port_access_resolves() {
    assert_clean_of_errors(
        "module Adder { in a : u8 in b : u8 out s : u9 s = a + b }
         module Top { in x : u8 in y : u8 out z : u9
           let add = Adder { a: x, b: y }
           z = add.s
         }",
    );
}

#[test]
fn loop_var_resolves() {
    assert_clean_of_errors(
        "const N : u32 = 4;
         module M { in data : bits<N> in mask : bits<N> out y : bits<N>
           wire t : bits<N>
           for i in 0..N { t[i] = data[i] & mask[i] }
           y = t
         }",
    );
}

#[test]
fn match_pattern_binding_resolves() {
    assert_clean_of_errors("module M { in x : u8 out y : u8 y = match x { n => n } }");
}

#[test]
fn match_guard_sees_binding() {
    assert_clean_of_errors(
        "module M { in x : u8 out y : u8 y = match x { n if n > 4 => n, _ => 0 } }",
    );
}

#[test]
fn fn_params_and_body_resolve() {
    assert_clean_of_errors("fn parity(x: u8, y: u8) -> bool { x ^ y == 0 }");
}

#[test]
fn fn_generic_const_param_resolves() {
    assert_clean_of_errors("fn f<const N: u32>(a: bits<N>) -> bool { a[0] }");
}

#[test]
fn fn_forward_reference_from_module() {
    assert_clean_of_errors(
        "module M { in a : u8 in b : u8 out y : bool y = esit(a, b) }
         fn esit(x: u8, y: u8) -> bool { x == y }",
    );
}

#[test]
fn domain_annotation_resolves() {
    assert_clean_of_errors(
        "domain Hizli { clock = posedge }
         module M { in clk : clock @Hizli in a : u8 @Hizli out y : u8 @Hizli y = a }",
    );
}

#[test]
fn const_forward_reference_resolves() {
    // İtem seviyesi ileri referans — döngü DEĞİL (A → B, B → literal).
    assert_clean_of_errors(
        "const A : u32 = B;\nconst B : u32 = 2;\nmodule M { in x : bits<A> out y : bits<A> y = x }",
    );
}

#[test]
fn resolutions_map_records_const_kind() {
    let result = check("const W : u32 = 8;\nmodule M { in x : bits<W> out y : bits<W> y = x }");
    let (def, data) = result.resolve.def_by_name("W").expect("W tanımlı olmalı");
    assert_eq!(data.kind, DefKind::Const);
    assert_eq!(result.resolve.def_kind(def), DefKind::Const);
}

#[test]
fn def_by_name_finds_module() {
    let result = check("module Sayici { in a : u8 out y : u8 y = a }");
    let (_, data) = result.resolve.def_by_name("Sayici").expect("modül tanımlı");
    assert_eq!(data.kind, DefKind::Module);
}

// ═══ Hatalar (spec §12: ✗) ════════════════════════════════════════

#[test]
fn undefined_name_e1001() {
    let codes = codes("module M { in a : u8 out y : u8 y = a + tanimsiz_isim }");
    assert!(codes.contains(&"E1001"), "{codes:?}");
}

#[test]
fn e1001_typo_gets_suggestion() {
    // 'enabel' → 'enable' (spec §8 örneği).
    let result = check("module M { in enable : bool out y : u8 y = if enabel { 1 } else { 0 } }");
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E1001")
        .expect("E1001 bekleniyor");
    assert!(
        diag.help
            .as_deref()
            .unwrap_or("")
            .contains("did you mean 'enable'?"),
        "öneri bekleniyor: {:?}",
        diag.help
    );
    assert!(!diag.suggestions.is_empty(), "fix-it önerisi bekleniyor");
}

#[test]
fn e1001_no_suggestion_when_nothing_close() {
    let result = check("module M { in a : u8 out y : u8 y = tamamen_alakasiz_bir_isim }");
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E1001")
        .expect("E1001 bekleniyor");
    assert!(
        diag.help
            .as_deref()
            .unwrap_or("")
            .contains("not defined in any scope"),
        "genel yardım bekleniyor: {:?}",
        diag.help
    );
}

#[test]
fn use_before_decl_e1002() {
    // Modül içi SIRALI kural (spec §4).
    let codes = codes("module M { out y : u8 y = temp let temp = 42 }");
    assert!(codes.contains(&"E1002"), "{codes:?}");
    assert!(
        !codes.contains(&"E1001"),
        "E1001 değil E1002 üretmeli: {codes:?}"
    );
}

#[test]
fn reg_use_before_decl_e1002() {
    let codes =
        codes("module M { in clk : clock out y : u8 y = r reg r : u8 = 0 on clk { r <= r + 1 } }");
    assert!(codes.contains(&"E1002"), "{codes:?}");
}

#[test]
fn duplicate_same_scope_e1003() {
    let codes = codes("module M { in a : u8 out y : u8 let t = 1 let t = 2 y = t }");
    assert!(codes.contains(&"E1003"), "{codes:?}");
}

#[test]
fn duplicate_port_e1003() {
    let codes = codes("module M { in data : u8 in data : u8 out y : u8 y = data }");
    assert!(codes.contains(&"E1003"), "{codes:?}");
}

#[test]
fn port_then_let_same_name_e1003() {
    let codes = codes("module M { in data : u8 out y : u8 let data = 5 y = data }");
    assert!(codes.contains(&"E1003"), "{codes:?}");
}

#[test]
fn non_namespace_path_e1005() {
    let codes = codes("module M { in a : u8 out y : u8 y = a::b }");
    assert!(codes.contains(&"E1005"), "{codes:?}");
}

#[test]
fn cyclic_module_dependency_e1006() {
    let codes = codes(
        "module A { in x : u8 out y : u8 let b = B { p: x } y = b.q }
         module B { in p : u8 out q : u8 let a = A { x: p } q = a.y }",
    );
    assert_eq!(
        codes.iter().filter(|c| **c == "E1006").count(),
        1,
        "tam bir E1006 bekleniyor: {codes:?}"
    );
}

#[test]
fn unknown_enum_variant_e1007() {
    let codes = codes(
        "enum Durum : bits<2> { Bekle = 0, Calis = 1 }
         module M { in x : u8 out y : u8 y = match x { Durum::Kos => 0, _ => 1 } }",
    );
    assert!(codes.contains(&"E1007"), "{codes:?}");
}

#[test]
fn e1007_variant_suggestion() {
    let result = check(
        "enum Durum : bits<2> { Bekle = 0, Calis = 1 }
         module M { in x : u8 out y : u8 y = match x { Durum::Bekl => 0, _ => 1 } }",
    );
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E1007")
        .expect("E1007 bekleniyor");
    assert!(
        diag.help
            .as_deref()
            .unwrap_or("")
            .contains("did you mean 'Bekle'?"),
        "varyant önerisi bekleniyor: {:?}",
        diag.help
    );
}

#[test]
fn unknown_struct_field_e1008() {
    let codes = codes(
        "struct Nokta { x: u8, y: u8 }
         fn yap(a: u8) -> u8 { let p = Nokta { x: a, z: a } p.x }",
    );
    assert!(codes.contains(&"E1008"), "{codes:?}");
}

#[test]
fn unknown_instance_port_e1009() {
    let codes = codes(
        "module Adder { in a : u8 in b : u8 out s : u9 s = a + b }
         module Top { in x : u8 out z : u9 let add = Adder { a: x, c: x } z = add.s }",
    );
    assert!(codes.contains(&"E1009"), "{codes:?}");
}

#[test]
fn unknown_instance_port_via_field_access_e1009() {
    let codes = codes(
        "module Adder { in a : u8 in b : u8 out s : u9 s = a + b }
         module Top { in x : u8 in y : u8 out z : u9
           let add = Adder { a: x, b: y }
           z = add.toplam
         }",
    );
    assert!(codes.contains(&"E1009"), "{codes:?}");
}

#[test]
fn e1009_port_suggestion() {
    let result = check(
        "module Adder { in a : u8 in b : u8 out sum : u9 sum = a + b }
         module Top { in x : u8 in y : u8 out z : u9
           let add = Adder { a: x, b: y }
           z = add.sun
         }",
    );
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E1009")
        .expect("E1009 bekleniyor");
    assert!(
        diag.help
            .as_deref()
            .unwrap_or("")
            .contains("did you mean 'sum'?"),
        "port önerisi bekleniyor: {:?}",
        diag.help
    );
}

#[test]
fn ambiguous_import_e1010() {
    let codes = codes("use a::Ortak;\nuse b::Ortak;\nmodule M { in x : u8 out y : u8 y = x }");
    assert!(codes.contains(&"E1010"), "{codes:?}");
}

// ═══ Gölgeleme (spec §6) ══════════════════════════════════════════

#[test]
fn inner_scope_shadow_w1002() {
    let codes = codes(
        "module M { in clk : clock in data : u8 out y : u8
           reg r : u8 = 0
           on clk { let data = 1 r <= data }
           y = r
         }",
    );
    assert!(codes.contains(&"W1002"), "{codes:?}");
}

#[test]
fn builtin_shadow_w1003() {
    let codes = codes("module M { in a : u8 out y : u8 let clog2 = a y = clog2 }");
    assert!(codes.contains(&"W1003"), "{codes:?}");
}

#[test]
fn no_shadow_warning_for_disjoint_scopes() {
    // İki ayrı modülde aynı isim gölgeleme değildir.
    let result = check(
        "module M1 { in a : u8 out y : u8 y = a }
         module M2 { in a : u8 out y : u8 y = a }",
    );
    let codes = result.error_codes();
    assert!(!codes.contains(&"W1002"), "{codes:?}");
    assert!(!codes.contains(&"E1003"), "{codes:?}");
}

// ═══ Kullanım takibi (spec §9) ════════════════════════════════════

#[test]
fn unused_input_port_w1001() {
    let codes = codes("module M { in a : u8 in kullanilmayan : u8 out y : u8 y = a }");
    assert!(codes.contains(&"W1001"), "{codes:?}");
}

#[test]
fn underscore_prefix_exempt_from_w1001() {
    let codes = codes("module M { in a : u8 in _yedek : u8 out y : u8 y = a }");
    assert!(
        !codes.contains(&"W1001"),
        "'_' öneki muaf olmalı: {codes:?}"
    );
}

#[test]
fn written_never_read_register_w1004() {
    let codes = codes(
        "module M { in clk : clock in a : u8 out y : u8
           reg r : u8 = 0
           on clk { r <= a }
           y = a
         }",
    );
    assert!(codes.contains(&"W1004"), "{codes:?}");
}

#[test]
fn read_register_no_w1004() {
    let codes =
        codes("module M { in clk : clock out y : u8 reg r : u8 = 0 on clk { r <= r + 1 } y = r }");
    assert!(!codes.contains(&"W1004"), "{codes:?}");
}

#[test]
fn unused_import_w1005() {
    let codes = codes("use volt::yardimci::Donustur;\nmodule M { in a : u8 out y : u8 y = a }");
    assert!(codes.contains(&"W1005"), "{codes:?}");
}

#[test]
fn used_import_no_w1005() {
    let codes = codes("use paket::SABIT;\nmodule M { in a : u8 out y : u8 y = a + SABIT }");
    assert!(!codes.contains(&"W1005"), "{codes:?}");
    assert!(
        !codes.contains(&"E1001"),
        "import edilen isim çözülmeli: {codes:?}"
    );
}

#[test]
fn unused_domain_w3004() {
    let codes = codes(
        "domain Bos { clock = posedge }
         module M { in a : u8 out y : u8 y = a }",
    );
    assert!(codes.contains(&"W3004"), "{codes:?}");
}

#[test]
fn used_domain_no_w3004() {
    let codes = codes(
        "domain Hizli { clock = posedge }
         module M { in clk : clock @Hizli in a : u8 out y : u8 y = a }",
    );
    assert!(!codes.contains(&"W3004"), "{codes:?}");
}

#[test]
fn fully_used_module_no_warnings() {
    let result =
        check("module M { in clk : clock out y : u8 reg r : u8 = 0 on clk { r <= r + 1 } y = r }");
    assert!(
        result.diagnostics.is_empty(),
        "temiz modülde tanı olmamalı: {:?}",
        result.error_codes()
    );
}

// ═══ Prelude ve Trit (spec §7) ════════════════════════════════════

#[test]
fn trit_not_in_prelude() {
    // Prelude yalnız 9 yerleşiği içerir; Trit opt-in import (UX Anayasası).
    // Not: 'Trit' tip konumunda lexer anahtar kelimesidir; burada prelude
    // tanım kümesinin kendisi denetlenir.
    let result = check("module M { in a : u8 out y : u8 y = a }");
    assert!(
        result.resolve.defs.iter().all(|d| d.name != "Trit"),
        "Trit prelude'de olmamalı"
    );
    assert!(
        result.resolve.defs.iter().any(|d| d.name == "sync"),
        "sync prelude'de olmalı"
    );
}

#[test]
fn imported_external_name_resolves() {
    let codes =
        codes("use volt::ternary::Donusum;\nmodule M { in a : u8 out y : u8 y = a + Donusum }");
    assert!(
        !codes.contains(&"E1001"),
        "import sonrası çözülmeli: {codes:?}"
    );
    assert!(
        !codes.contains(&"W1005"),
        "kullanılan import uyarılmamalı: {codes:?}"
    );
}

// ═══ Levenshtein eşiği (spec §8) ══════════════════════════════════

#[test]
fn closest_match_within_threshold() {
    let cands = vec!["enable".to_string(), "reset".to_string()];
    // 'enabel' ↔ 'enable': mesafe 2, eşik 6/3 = 2 → önerilir.
    assert_eq!(closest_match("enabel", &cands), Some("enable".to_string()));
}

#[test]
fn closest_match_rejects_far_names() {
    let cands = vec!["clk".to_string()];
    assert_eq!(closest_match("veri_yolu", &cands), None);
}

#[test]
fn closest_match_short_name_threshold_one() {
    // Uzunluk 2 → eşik max(2/3, 1) = 1.
    let cands = vec!["ab".to_string()];
    assert_eq!(closest_match("ac", &cands), Some("ab".to_string()));
    assert_eq!(closest_match("xy", &cands), None);
}

#[test]
fn closest_match_ignores_exact_duplicates() {
    // Mesafe 0 önerilmez (isim zaten aynı — farklı bir sorun var).
    let cands = vec!["tam".to_string()];
    assert_eq!(closest_match("tam", &cands), None);
}
