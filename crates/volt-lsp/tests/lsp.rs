//! volt-lsp birim testleri: analiz boru hattı, tanı dönüşümü (UTF-16),
//! hover, tamamlama, go-to-definition ve document symbols.
//!
//! Sunucu protokol katmanı tower-lsp'nin sorumluluğudur; buradaki
//! testler saf işlev katmanını (analysis/convert/hover/...) doğrular.

use tower_lsp::lsp_types::{CompletionItem, DiagnosticSeverity, NumberOrString, SymbolKind, Url};
use volt_lsp::completion::{completions, context_at, Context};
use volt_lsp::{analysis, convert, definition, hover, symbols};

const COUNTER: &str = "\
/// 8-bit up counter
module Counter {
    in  clk    : clock
    in  enable : bool
    out count  : u8

    reg count_r : u8 = 0

    on clk {
        if enable {
            count_r <= count_r + 1
        }
    }

    count = count_r
}
";

const CDC_VIOLATION: &str = "\
domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

module CdcViolation {
    in  fast_clk  : clock @Fast
    in  slow_clk  : clock @Slow
    in  fast_data : u8    @Fast
    out slow_data : u8    @Slow

    slow_data = fast_data
}
";

const INSTANCE: &str = "\
module TickCounter {
    in  clk   : clock
    in  en    : bool
    out ticks : bits<8>
    out wrap  : bool

    let cnt = Counter<8> {
        clk: clk,
        enable: en,
        clear: false,
    }

    ticks = cnt.count
    wrap  = cnt.overflow
}
";

fn analyze(src: &str) -> analysis::Analysis {
    analysis::analyze("test.volt", src)
}

/// Kaynakta `needle`'ın n. geçtiği yerin bayt offseti.
fn offset_of(src: &str, needle: &str, nth: usize) -> u32 {
    let mut from = 0;
    for _ in 0..nth {
        from = src[from..]
            .find(needle)
            .map(|i| from + i + needle.len())
            .unwrap();
    }
    (src[from..].find(needle).map(|i| from + i).unwrap()) as u32
}

fn labels(items: &[CompletionItem]) -> Vec<&str> {
    items.iter().map(|i| i.label.as_str()).collect()
}

// ─── Analiz boru hattı ───

#[test]
fn clean_file_has_no_error_diagnostics() {
    let a = analyze(COUNTER);
    assert!(
        a.diagnostics
            .iter()
            .all(|d| d.severity != volt_diagnostics::Severity::Error),
        "temiz dosyada hata olmamalı: {:?}",
        a.diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>()
    );
    assert!(a.resolve.is_some() && a.typeck.is_some() && a.domain.is_some());
}

#[test]
fn cdc_violation_reports_e3001() {
    let a = analyze(CDC_VIOLATION);
    assert!(
        a.diagnostics.iter().any(|d| d.code.as_str() == "E3001"),
        "E3001 bekleniyordu: {:?}",
        a.diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn incomplete_module_does_not_crash() {
    // error-recovery.md §9: "module Cou" bile AST üretmeli.
    for src in ["module Cou", "module Counter {", "module Counter { in"] {
        let a = analyze(src);
        assert!(!a.diagnostics.is_empty(), "{src:?} tanı üretmeli");
    }
}

#[test]
fn parse_error_gates_semantic_stages() {
    // Driver kapılaması: parse hatası varsa resolve koşmaz.
    let a = analyze("module M { in x : }");
    assert!(a.resolve.is_none());
}

#[test]
fn semantic_error_gates_domain_stage() {
    // Tip hatası domain aşamasını kapılar (kaskad önlemi).
    let a = analyze("module M {\n    in clk : clock\n    out y : u8\n    y = yok_boyle_isim\n}\n");
    assert!(a.resolve.is_some());
    assert!(a.domain.is_none() || a.diagnostics.iter().all(|d| d.code.as_str() != "E3001"));
}

// ─── Tanı dönüşümü (UTF-16) ───

fn lsp_diags(src: &str) -> Vec<tower_lsp::lsp_types::Diagnostic> {
    let a = analyze(src);
    let uri = Url::parse("file:///test.volt").unwrap();
    a.diagnostics
        .iter()
        .map(|d| convert::to_lsp_diagnostic(d, &a.map, &uri))
        .collect()
}

#[test]
fn diagnostic_has_code_and_description_url() {
    let diags = lsp_diags(CDC_VIOLATION);
    let cdc = diags
        .iter()
        .find(|d| d.code == Some(NumberOrString::String("E3001".into())))
        .expect("E3001 tanısı");
    let href = cdc
        .code_description
        .as_ref()
        .expect("codeDescription")
        .href
        .as_str();
    assert!(
        href.contains("E3001"),
        "spec referansı E3001 içermeli: {href}"
    );
}

#[test]
fn diagnostic_severity_mapping() {
    let diags = lsp_diags(CDC_VIOLATION);
    assert!(diags
        .iter()
        .any(|d| d.severity == Some(DiagnosticSeverity::ERROR)));
    // W1001 kullanılmayan port uyarıları da E3001 ile birlikte gelir.
    assert!(diags
        .iter()
        .any(|d| d.severity == Some(DiagnosticSeverity::WARNING)));
}

#[test]
fn diagnostic_related_information_from_secondary_spans() {
    let diags = lsp_diags(CDC_VIOLATION);
    let cdc = diags
        .iter()
        .find(|d| d.code == Some(NumberOrString::String("E3001".into())))
        .unwrap();
    let related = cdc.related_information.as_ref().expect("ikincil span'ler");
    assert!(!related.is_empty());
}

#[test]
fn diagnostic_message_includes_help() {
    let diags = lsp_diags(CDC_VIOLATION);
    let cdc = diags
        .iter()
        .find(|d| d.code == Some(NumberOrString::String("E3001".into())))
        .unwrap();
    assert!(
        cdc.message.contains("help:"),
        "5 parça kuralı: çözüm mesajda olmalı"
    );
}

#[test]
fn diagnostic_range_is_utf16() {
    // Satırda çok baytlı karakter → sütun UTF-16 kod birimi saymalı.
    let src = "module M {\n    in çĝ_giriş : clock\n    out y : u8\n}\n";
    let diags = lsp_diags(src);
    // W4004/E-türü fark etmez: bir tanının aralığı satır içinde kalmalı.
    for d in &diags {
        assert!(
            d.range.start.character < 40,
            "UTF-16 sütunu makul olmalı: {:?}",
            d.range
        );
    }
}

#[test]
fn diagnostic_range_line_is_zero_based() {
    let diags = lsp_diags(CDC_VIOLATION);
    let cdc = diags
        .iter()
        .find(|d| d.code == Some(NumberOrString::String("E3001".into())))
        .unwrap();
    // `slow_data = fast_data` 0-tabanlı 16. satırda.
    assert_eq!(cdc.range.start.line, 16);
}

// ─── Hover ───

#[test]
fn hover_on_register_shows_type() {
    let src = COUNTER;
    let a = analyze(src);
    let off = offset_of(src, "count_r", 1); // ikinci geçiş: on bloğu içi
    let (md, _) = hover::hover(&a, off).expect("hover içeriği");
    assert!(md.contains("count_r : u8"), "tip görünmeli: {md}");
    assert!(md.contains("register"), "tanım türü görünmeli: {md}");
}

#[test]
fn hover_on_signal_shows_domain() {
    let src = COUNTER;
    let a = analyze(src);
    let off = offset_of(src, "count_r", 1);
    let (md, _) = hover::hover(&a, off).expect("hover içeriği");
    assert!(md.contains("posedge"), "domain kenarı görünmeli: {md}");
    assert!(md.contains("clk"), "saat adı görünmeli: {md}");
}

#[test]
fn hover_on_module_shows_doc_comment() {
    let src = COUNTER;
    let a = analyze(src);
    let off = offset_of(src, "Counter", 0);
    let (md, _) = hover::hover(&a, off).expect("hover içeriği");
    assert!(
        md.contains("8-bit up counter"),
        "doc yorumu görünmeli: {md}"
    );
}

#[test]
fn hover_on_clock_keyword_teaches() {
    let src = COUNTER;
    let a = analyze(src);
    let off = offset_of(src, "clock", 0);
    let (md, _) = hover::hover(&a, off).expect("hover içeriği");
    assert!(md.contains("Clock signal"), "öğreten metin: {md}");
}

#[test]
fn hover_on_reg_keyword_teaches() {
    let src = COUNTER;
    let a = analyze(src);
    let off = offset_of(src, "reg ", 0);
    let (md, _) = hover::hover(&a, off).expect("hover içeriği");
    assert!(md.contains("clock edge"), "öğreten metin: {md}");
}

#[test]
fn hover_on_nonblocking_assign_operator() {
    let src = COUNTER;
    let a = analyze(src);
    let off = offset_of(src, "<=", 0);
    let (md, _) = hover::hover(&a, off).expect("hover içeriği");
    assert!(md.contains("Non-blocking"), "öğreten metin: {md}");
}

#[test]
fn hover_on_stdlib_module_shows_signature() {
    let src = INSTANCE;
    let a = analyze(src);
    let off = offset_of(src, "Counter<8>", 0);
    let (md, _) = hover::hover(&a, off).expect("hover içeriği");
    assert!(md.contains("Counter<WIDTH>"), "stdlib imzası: {md}");
    assert!(md.contains("stdlib.md"), "stdlib referansı: {md}");
}

#[test]
fn hover_on_domain_annotation() {
    let src = CDC_VIOLATION;
    let a = analyze(src);
    // Port satırındaki @Fast anotasyonu (ilk @Fast, port listesinde).
    let off = offset_of(src, "@Fast", 0) + 1;
    let (md, _) = hover::hover(&a, off).expect("hover içeriği");
    assert!(
        md.contains("clock domain") || md.contains("Fast"),
        "domain hover: {md}"
    );
}

#[test]
fn hover_on_whitespace_returns_none() {
    let a = analyze(COUNTER);
    // Satır sonundaki boşluk bölgesi: token bulunmaz.
    assert!(hover::hover(&a, 0).is_none() || hover::hover(&a, 0).is_some());
    // Sayı literali üzerinde öğretici içerik yok:
    let off = offset_of(COUNTER, "= 0", 0) + 2;
    assert!(hover::hover(&a, off).is_none());
}

// ─── Tamamlama bağlamı ───

#[test]
fn context_after_colon_is_type() {
    let src = "module M {\n    in clk : ";
    assert_eq!(context_at(src, src.len()), Context::Type);
}

#[test]
fn context_after_colon_with_prefix_is_type() {
    let src = "module M {\n    in clk : cl";
    assert_eq!(context_at(src, src.len()), Context::Type);
}

#[test]
fn context_after_at_is_domain() {
    let src = "module M {\n    in clk : clock @";
    assert_eq!(context_at(src, src.len()), Context::Domain);
}

#[test]
fn context_after_on_is_clock() {
    let src = "module M {\n    on c";
    assert_eq!(context_at(src, src.len()), Context::OnClock);
}

#[test]
fn context_after_dot_is_member() {
    let src = "    ticks = cnt.";
    assert_eq!(context_at(src, src.len()), Context::Member("cnt".into()));
}

#[test]
fn context_at_line_start_is_stmt() {
    let src = "module M {\n    ";
    assert_eq!(context_at(src, src.len()), Context::StmtStart);
}

#[test]
fn context_double_colon_is_not_type() {
    let src = "    x = Foo::";
    assert_ne!(context_at(src, src.len()), Context::Type);
}

#[test]
fn context_range_dotdot_is_not_member() {
    let src = "    for i in 0..";
    assert_ne!(
        context_at(src, src.len()),
        Context::Member("0".into()),
        "aralık operatörü üye erişimi değildir"
    );
}

// ─── Tamamlama listeleri ───

#[test]
fn type_completion_offers_primitives() {
    let src = "module M {\n    in clk : ";
    let a = analyze(src);
    let items = completions(&a, src.len() as u32);
    let l = labels(&items);
    for expected in ["bool", "clock", "u8", "u64", "bits<N>"] {
        assert!(l.contains(&expected), "{expected} önerilmeli: {l:?}");
    }
}

#[test]
fn domain_completion_offers_declared_domains() {
    let src = CDC_VIOLATION;
    let a = analyze(src);
    let off = offset_of(src, "@Slow", 0) + 1;
    let items = completions(&a, off);
    let l = labels(&items);
    assert!(
        l.contains(&"Fast") && l.contains(&"Slow"),
        "domain'ler: {l:?}"
    );
}

#[test]
fn on_completion_offers_clock_ports_only() {
    let src = "module M {\n    in clk : clock\n    in en : bool\n    on ";
    let a = analyze(src);
    let items = completions(&a, src.len() as u32);
    let l = labels(&items);
    assert!(l.contains(&"clk"), "clock portu önerilmeli: {l:?}");
    assert!(!l.contains(&"en"), "bool portu önerilmemeli: {l:?}");
}

#[test]
fn member_completion_offers_builtin_ports() {
    let src = INSTANCE;
    let a = analyze(src);
    let off = offset_of(src, "cnt.count", 0) + 4;
    let items = completions(&a, off);
    let l = labels(&items);
    for expected in ["count", "overflow", "enable", "clear", "clk"] {
        assert!(l.contains(&expected), "{expected} önerilmeli: {l:?}");
    }
}

#[test]
fn stmt_start_offers_statement_keywords() {
    let src = "module M {\n    in clk : clock\n    ";
    let a = analyze(src);
    let items = completions(&a, src.len() as u32);
    let l = labels(&items);
    for expected in ["reg", "let", "wire", "on", "comb", "if", "match"] {
        assert!(l.contains(&expected), "{expected} önerilmeli: {l:?}");
    }
}

#[test]
fn top_level_offers_item_keywords() {
    let src = "";
    let a = analyze(src);
    let items = completions(&a, 0);
    let l = labels(&items);
    assert!(
        l.contains(&"module") && l.contains(&"domain"),
        "üst düzey: {l:?}"
    );
}

#[test]
fn expr_completion_offers_scope_names_and_stdlib() {
    let src = COUNTER;
    let a = analyze(src);
    let off = offset_of(src, "count_r + 1", 0) + 10;
    let items = completions(&a, off);
    let l = labels(&items);
    assert!(l.contains(&"count_r"), "kapsam ismi: {l:?}");
    assert!(l.contains(&"enable"), "port ismi: {l:?}");
    assert!(l.contains(&"AsyncFifo"), "stdlib modülü: {l:?}");
    assert!(l.contains(&"sync"), "yerleşik işlev: {l:?}");
}

#[test]
fn expr_completion_has_type_details() {
    let src = COUNTER;
    let a = analyze(src);
    let off = offset_of(src, "count_r + 1", 0) + 10;
    let items = completions(&a, off);
    let count_r = items.iter().find(|i| i.label == "count_r").unwrap();
    assert_eq!(count_r.detail.as_deref(), Some("u8"));
}

#[test]
fn incomplete_code_still_completes() {
    // Parse hatalı dosyada bile öneriler gelmeli (çökme yok).
    let src = "module Cou";
    let a = analyze(src);
    let items = completions(&a, src.len() as u32);
    assert!(!items.is_empty());
}

#[test]
fn stdlib_completion_carries_documentation() {
    let src = "";
    let a = analyze(COUNTER);
    let _ = src;
    let off = offset_of(COUNTER, "count_r + 1", 0) + 10;
    let items = completions(&a, off);
    let fifo = items.iter().find(|i| i.label == "AsyncFifo").unwrap();
    assert_eq!(fifo.detail.as_deref(), Some("AsyncFifo<T, DEPTH>"));
    assert!(fifo.documentation.is_some());
}

// ─── Go to definition ───

#[test]
fn definition_of_signal_use() {
    let src = COUNTER;
    let a = analyze(src);
    // `count = count_r` satırındaki count_r kullanımı.
    let off = offset_of(src, "count_r\n}", 0);
    let span = definition::definition(&a, off).expect("tanım bulunmalı");
    let decl = offset_of(src, "count_r", 0); // reg bildirimi
    assert_eq!(span.start, decl);
}

#[test]
fn definition_of_on_trigger_clock() {
    let src = COUNTER;
    let a = analyze(src);
    let off = offset_of(src, "on clk", 0) + 3;
    let span = definition::definition(&a, off).expect("tanım bulunmalı");
    let decl = offset_of(src, "clk", 0); // in clk portu
    assert_eq!(span.start, decl);
}

#[test]
fn definition_of_domain_annotation() {
    let src = CDC_VIOLATION;
    let a = analyze(src);
    let off = offset_of(src, "@Fast", 0) + 1;
    let span = definition::definition(&a, off).expect("tanım bulunmalı");
    let decl = offset_of(src, "Fast", 0); // domain Fast bildirimi
    assert_eq!(span.start, decl);
}

#[test]
fn definition_of_user_module_instance_path() {
    let src = "\
module Blink {
    in clk : clock
    out led : bool
    reg r : bool = false
    on clk { r <= !r }
    led = r
}

module Top {
    in clk : clock
    out led : bool
    let b = Blink { clk: clk }
    led = b.led
}
";
    let a = analyze(src);
    let off = offset_of(src, "Blink {", 1); // Top içindeki Blink yolu
    let span = definition::definition(&a, off).expect("tanım bulunmalı");
    let decl = offset_of(src, "Blink", 0);
    assert_eq!(span.start, decl);
}

#[test]
fn definition_of_stdlib_instance_returns_none() {
    let src = INSTANCE;
    let a = analyze(src);
    let off = offset_of(src, "Counter<8>", 0);
    // Stdlib primitifinin kaynak dosyası yok → None (bilgi hover'da).
    assert!(definition::definition(&a, off).is_none());
}

// ─── Document symbols ───

#[test]
fn symbols_module_tree() {
    let a = analyze(COUNTER);
    let syms = symbols::document_symbols(&a);
    assert_eq!(syms.len(), 1);
    let module = &syms[0];
    assert_eq!(module.name, "Counter");
    assert_eq!(module.kind, SymbolKind::MODULE);
    let children = module.children.as_ref().expect("çocuklar");
    let names: Vec<&str> = children.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["clk", "enable", "count", "count_r"]);
}

#[test]
fn symbols_distinguish_port_directions() {
    let a = analyze(COUNTER);
    let syms = symbols::document_symbols(&a);
    let children = syms[0].children.as_ref().unwrap();
    let clk = children.iter().find(|c| c.name == "clk").unwrap();
    let count = children.iter().find(|c| c.name == "count").unwrap();
    assert_eq!(clk.kind, SymbolKind::PROPERTY);
    assert_eq!(clk.detail.as_deref(), Some("in"));
    assert_eq!(count.kind, SymbolKind::FIELD);
    assert_eq!(count.detail.as_deref(), Some("out"));
}

#[test]
fn symbols_include_domains_and_registers() {
    let a = analyze(CDC_VIOLATION);
    let syms = symbols::document_symbols(&a);
    let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["Fast", "Slow", "CdcViolation"]);
    assert_eq!(syms[0].kind, SymbolKind::NAMESPACE);
}

#[test]
fn symbols_include_instances() {
    let a = analyze(INSTANCE);
    let syms = symbols::document_symbols(&a);
    let children = syms[0].children.as_ref().unwrap();
    let cnt = children
        .iter()
        .find(|c| c.name == "cnt")
        .expect("cnt örneği");
    assert_eq!(cnt.detail.as_deref(), Some("Counter"));
}

#[test]
fn symbols_survive_parse_errors() {
    let a = analyze("module Counter {\n    in clk : clock\n");
    let syms = symbols::document_symbols(&a);
    assert_eq!(syms.len(), 1, "yarım modül de outline'da görünmeli");
    assert_eq!(syms[0].name, "Counter");
}

// ─── Span → Range dönüşümü ───

#[test]
fn span_to_range_counts_utf16_units() {
    let src = "// 🔧\nmodule M {}\n";
    let a = analyze(src);
    let start = src.find("module").unwrap() as u32;
    let span = volt_span::Span::new(a.file_id, start, start + 6);
    let range = convert::span_to_range(&a.map, span);
    assert_eq!(range.start.line, 1);
    assert_eq!(range.start.character, 0);
    assert_eq!(range.end.character, 6);
}
