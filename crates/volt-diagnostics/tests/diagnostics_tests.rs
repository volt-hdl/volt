//! volt-diagnostics birim testleri.
//!
//! JSON şeması referansı: docs/spec/cli-contract.md §5.

use volt_diagnostics::{
    messages, render_human, render_short, to_json_value, Applicability, Diagnostic, ErrorCode,
    LabeledSpan, Lang, NoteKind, Severity, Suggestion,
};
use volt_span::{FileId, SourceMap, Span};

fn sample_map() -> (SourceMap, FileId) {
    let mut map = SourceMap::new();
    let id = map.add_file("design.volt", "module M {\n    result = data\n}\n");
    (map, id)
}

/// cli-contract.md'deki E3001 örneğine benzer tam donanımlı tanı.
fn sample_diagnostic(file: FileId) -> Diagnostic {
    let data_span = Span::new(file, 24, 28); // "data"
    let result_span = Span::new(file, 15, 21); // "result"
    Diagnostic::error(
        ErrorCode::E3001,
        "iki farklı saat alanı doğrudan bağlanamaz",
        LabeledSpan::primary(data_span, "'data' → fast_clk alanında"),
        "result = sync(data, slow_clk)",
    )
    .with_secondary(result_span, "'result' → slow_clk alanında")
    .with_note(NoteKind::Reason, "sinyal kararsız bir anda yakalanabilir")
    .with_suggestion(Suggestion {
        span: data_span,
        replacement: "sync(data, slow_clk)".to_string(),
        applicability: Applicability::MachineApplicable,
    })
}

// ═══ ErrorCode ════════════════════════════════════════════════════

#[test]
fn code_as_str() {
    assert_eq!(ErrorCode::E3001.as_str(), "E3001");
    assert_eq!(ErrorCode::W1001.as_str(), "W1001");
}

#[test]
fn code_display_includes_description() {
    // Varsayılan dil EN — Display açıklaması İngilizce.
    let text = ErrorCode::E0001.to_string();
    assert!(text.starts_with("E0001:"));
    assert!(text.contains("Unexpected token"));
}

#[test]
fn code_warning_prefix_detection() {
    assert!(ErrorCode::W1001.is_warning());
    assert!(!ErrorCode::E3001.is_warning());
}

#[test]
fn all_codes_have_nonempty_description() {
    for code in ErrorCode::ALL {
        assert!(
            !code.description().trim().is_empty(),
            "{} açıklamasız",
            code.as_str()
        );
    }
}

#[test]
fn all_codes_cover_expected_ranges() {
    let strs: Vec<&str> = ErrorCode::ALL.iter().map(|c| c.as_str()).collect();
    for expected in [
        "E0001", "E0013", "E1010", "E2029", "E3012", "E4004", "E6001", "E7002", "E9002", "W0010",
        "W4002",
    ] {
        assert!(strs.contains(&expected), "{expected} eksik");
    }
    assert!(
        ErrorCode::ALL.len() >= 80,
        "kod sayısı: {}",
        ErrorCode::ALL.len()
    );
}

#[test]
fn code_explain_url_format() {
    assert_eq!(
        ErrorCode::E3001.explain_url(),
        "https://volthdl.org/errors/E3001"
    );
}

// ═══ 5 parça kuralı ═══════════════════════════════════════════════

#[test]
fn constructor_produces_valid_diagnostic() {
    let (_, file) = sample_map();
    assert_eq!(sample_diagnostic(file).validate(), Ok(()));
}

#[test]
fn missing_help_fails_validation() {
    let (_, file) = sample_map();
    let mut diag = sample_diagnostic(file);
    diag.help = None;
    let err = diag.validate().unwrap_err();
    assert!(err.contains("5 parça"), "hata metni: {err}");
}

#[test]
fn empty_help_fails_validation() {
    let (_, file) = sample_map();
    let mut diag = sample_diagnostic(file);
    diag.help = Some("   ".to_string());
    assert!(diag.validate().is_err());
}

#[test]
fn missing_primary_span_fails_validation() {
    let (_, file) = sample_map();
    let mut diag = sample_diagnostic(file);
    diag.spans.retain(|s| !s.primary);
    assert!(diag.validate().is_err());
}

#[test]
fn empty_message_fails_validation() {
    let (_, file) = sample_map();
    let mut diag = sample_diagnostic(file);
    diag.message = String::new();
    assert!(diag.validate().is_err());
}

// ═══ JSON şeması (cli-contract.md §5) ═════════════════════════════

#[test]
fn json_top_level_field_names_match_contract() {
    let (map, file) = sample_map();
    let value = to_json_value(&sample_diagnostic(file), &map);
    let obj = value.as_object().unwrap();
    for key in [
        "code",
        "severity",
        "message",
        "spans",
        "notes",
        "help",
        "suggestions",
        "explain_url",
    ] {
        assert!(obj.contains_key(key), "'{key}' alanı eksik");
    }
}

#[test]
fn json_span_field_names_match_contract() {
    let (map, file) = sample_map();
    let value = to_json_value(&sample_diagnostic(file), &map);
    let span = &value["spans"][0];
    let obj = span.as_object().unwrap();
    for key in ["file", "start", "end", "label", "primary"] {
        assert!(obj.contains_key(key), "'{key}' alanı eksik");
    }
    for key in ["line", "col", "byte"] {
        assert!(span["start"].as_object().unwrap().contains_key(key));
        assert!(span["end"].as_object().unwrap().contains_key(key));
    }
}

#[test]
fn json_values_match_contract_example_shape() {
    let (map, file) = sample_map();
    let value = to_json_value(&sample_diagnostic(file), &map);
    assert_eq!(value["code"], "E3001");
    assert_eq!(value["severity"], "error");
    assert_eq!(value["spans"][0]["primary"], true);
    assert_eq!(value["spans"][1]["primary"], false);
    assert_eq!(value["spans"][0]["file"], "design.volt");
    assert_eq!(value["notes"][0]["kind"], "reason");
    assert_eq!(value["help"], "result = sync(data, slow_clk)");
    assert_eq!(value["explain_url"], "https://volthdl.org/errors/E3001");
}

#[test]
fn json_line_col_are_one_based() {
    let (map, file) = sample_map();
    let value = to_json_value(&sample_diagnostic(file), &map);
    // "data" 2. satırda, "    result = data" → byte 24, sütun 14
    assert_eq!(value["spans"][0]["start"]["line"], 2);
    assert_eq!(value["spans"][0]["start"]["col"], 14);
    assert_eq!(value["spans"][0]["start"]["byte"], 24);
}

#[test]
fn json_suggestion_fields_match_contract() {
    let (map, file) = sample_map();
    let value = to_json_value(&sample_diagnostic(file), &map);
    let sug = &value["suggestions"][0];
    let obj = sug.as_object().unwrap();
    for key in ["span", "replacement", "applicability"] {
        assert!(obj.contains_key(key), "'{key}' alanı eksik");
    }
    assert_eq!(sug["applicability"], "machine-applicable");
    // Suggestion span'inde label/primary yok (şemada da yok)
    let span_obj = sug["span"].as_object().unwrap();
    assert!(!span_obj.contains_key("label"));
    assert!(!span_obj.contains_key("primary"));
}

#[test]
fn json_applicability_strings() {
    assert_eq!(
        Applicability::MachineApplicable.as_str(),
        "machine-applicable"
    );
    assert_eq!(Applicability::MaybeIncorrect.as_str(), "maybe-incorrect");
    assert_eq!(Applicability::HasPlaceholders.as_str(), "has-placeholders");
    assert_eq!(Applicability::Unspecified.as_str(), "unspecified");
}

#[test]
fn json_severity_strings() {
    assert_eq!(Severity::Error.as_str(), "error");
    assert_eq!(Severity::Warning.as_str(), "warning");
    assert_eq!(Severity::Note.as_str(), "note");
}

// ═══ İnsan ve short çıktı ═════════════════════════════════════════

#[test]
fn human_output_has_contract_elements() {
    let (map, file) = sample_map();
    let text = render_human(&sample_diagnostic(file), &map);
    assert!(text.contains("error[E3001]"), "çıktı: {text}");
    assert!(text.contains("iki farklı saat alanı doğrudan bağlanamaz"));
    assert!(text.contains("design.volt"));
    // Şablon anahtarları aktif dilden gelir — varsayılan EN (GLOSSARY §7).
    assert!(text.contains("reason: sinyal kararsız bir anda yakalanabilir"));
    assert!(text.contains("help: result = sync(data, slow_clk)"));
    assert!(text.contains("for more: volt explain E3001"));
}

// ═══ Yerelleştirme (messages/{en,tr}.rs) ══════════════════════════

#[test]
fn en_and_tr_cover_the_same_code_set() {
    // İki dil de ErrorCode::ALL'daki her kod için boş olmayan açıklama
    // taşımalı (match'ler joker kolsuz — derleyici de zorlar).
    for &code in ErrorCode::ALL {
        let en = messages::message(Lang::En, code);
        let tr = messages::message(Lang::Tr, code);
        assert!(!en.trim().is_empty(), "{}: EN açıklama boş", code.as_str());
        assert!(!tr.trim().is_empty(), "{}: TR açıklama boş", code.as_str());
    }
}

#[test]
fn template_keys_follow_glossary() {
    let en = messages::keys(Lang::En);
    assert_eq!(en.reason, "reason:");
    assert_eq!(en.help, "help:");
    assert_eq!(en.note, "note:");
    assert_eq!(en.for_more, "for more:");
    let tr = messages::keys(Lang::Tr);
    assert_eq!(tr.reason, "neden:");
    assert_eq!(tr.help, "çözüm:");
    assert_eq!(tr.note, "not:");
    assert_eq!(tr.for_more, "daha fazla:");
}

#[test]
fn lang_parse_accepts_en_tr_only() {
    assert_eq!(Lang::parse("en"), Some(Lang::En));
    assert_eq!(Lang::parse("TR"), Some(Lang::Tr));
    assert_eq!(Lang::parse("de"), None);
    assert_eq!(Lang::parse(""), None);
}

#[test]
fn short_output_single_line_format() {
    let (map, file) = sample_map();
    let line = render_short(&sample_diagnostic(file), &map);
    assert_eq!(
        line,
        "design.volt:2:14: error[E3001]: iki farklı saat alanı doğrudan bağlanamaz"
    );
}

#[test]
fn short_output_for_warning() {
    let (map, file) = sample_map();
    let diag = Diagnostic::warning(
        ErrorCode::W1001,
        "kullanılmayan sinyal: 'temp'",
        LabeledSpan::primary(Span::new(file, 15, 21), ""),
        "'_temp' olarak yeniden adlandırın",
    );
    let line = render_short(&diag, &map);
    assert!(
        line.starts_with("design.volt:2:5: warning[W1001]:"),
        "çıktı: {line}"
    );
}
