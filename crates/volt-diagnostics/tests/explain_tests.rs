//! `volt explain` açıklama tabanı testleri (cli-contract.md §9).
//!
//! Kapsam: 89 kodun iki dilde de tam açıklaması, §9 bölüm yapısı,
//! genişliğe göre sarma, renk, --list gruplaması ve kod önerisi.

use volt_diagnostics::explain::{
    category_name, explanation, render_explanation, render_list, suggest, DEFAULT_WIDTH,
};
use volt_diagnostics::{ErrorCode, Lang};

/// Spec'teki toplam kod sayısı — kod eklenince bilinçli olarak güncellenir.
const CODE_COUNT: usize = 89;

#[test]
fn all_codes_present_89_of_89() {
    assert_eq!(
        ErrorCode::ALL.len(),
        CODE_COUNT,
        "kod tablosu değişti — açıklamaları ve bu sabiti güncelle"
    );
}

#[test]
fn every_code_has_complete_en_explanation() {
    for &code in ErrorCode::ALL {
        let exp = explanation(Lang::En, code);
        assert!(!exp.title.trim().is_empty(), "{}: title boş", code.as_str());
        assert!(
            !exp.summary.trim().is_empty(),
            "{}: summary boş",
            code.as_str()
        );
        assert!(!exp.why.trim().is_empty(), "{}: why boş", code.as_str());
        assert!(
            !exp.example.trim().is_empty(),
            "{}: example boş",
            code.as_str()
        );
        assert!(!exp.fix.trim().is_empty(), "{}: fix boş", code.as_str());
    }
}

#[test]
fn every_code_has_complete_tr_explanation() {
    for &code in ErrorCode::ALL {
        let exp = explanation(Lang::Tr, code);
        assert!(!exp.title.trim().is_empty(), "{}: title boş", code.as_str());
        assert!(
            !exp.summary.trim().is_empty(),
            "{}: summary boş",
            code.as_str()
        );
        assert!(!exp.why.trim().is_empty(), "{}: why boş", code.as_str());
        assert!(
            !exp.example.trim().is_empty(),
            "{}: example boş",
            code.as_str()
        );
        assert!(!exp.fix.trim().is_empty(), "{}: fix boş", code.as_str());
    }
}

#[test]
fn every_rendered_explanation_has_at_least_5_lines() {
    for &code in ErrorCode::ALL {
        for lang in [Lang::En, Lang::Tr] {
            let text = render_explanation(code, lang, DEFAULT_WIDTH, false);
            let lines = text.lines().filter(|l| !l.trim().is_empty()).count();
            assert!(
                lines >= 5,
                "{} ({}): {} dolu satır < 5",
                code.as_str(),
                lang.as_str(),
                lines
            );
        }
    }
}

#[test]
fn render_e3001_en_follows_spec_section_order() {
    let text = render_explanation(ErrorCode::E3001, Lang::En, 80, false);
    assert!(
        text.starts_with("E3001: "),
        "başlık ilk satır olmalı: {text}"
    );
    let idx = |needle: &str| {
        text.find(needle)
            .unwrap_or_else(|| panic!("'{needle}' yok"))
    };
    assert!(idx("WHY THIS IS A PROBLEM") < idx("EXAMPLE"));
    assert!(idx("EXAMPLE") < idx("SOLUTION"));
    assert!(idx("SOLUTION") < idx("NOTE"));
    assert!(idx("NOTE") < idx("FOR MORE"));
    assert!(text.contains("https://volthdl.org/errors/E3001"));
    assert!(text.contains("https://volthdl.org/guide/cdc"));
    assert!(text.contains("sync(data, slow_clk)"));
}

#[test]
fn render_e3001_tr_follows_spec_section_order() {
    let text = render_explanation(ErrorCode::E3001, Lang::Tr, 80, false);
    assert!(text.starts_with("E3001: Saat Alanı"), "başlık: {text}");
    let idx = |needle: &str| {
        text.find(needle)
            .unwrap_or_else(|| panic!("'{needle}' yok"))
    };
    assert!(idx("NEDEN SORUN") < idx("ÖRNEK"));
    assert!(idx("ÖRNEK") < idx("ÇÖZÜM"));
    assert!(idx("ÇÖZÜM") < idx("NOT"));
    assert!(idx("NOT") < idx("DAHA FAZLA"));
    assert!(text.contains("https://volthdl.org/errors/E3001"));
    // Kod parçacıkları çevrilmez (GLOSSARY.md §0).
    assert!(text.contains("sync(data, slow_clk)"));
}

#[test]
fn prose_wraps_to_requested_width() {
    let width = 50;
    let text = render_explanation(ErrorCode::E3001, Lang::En, width, false);
    for line in text.lines().skip(1) {
        // Kod blokları ve linkler ("  " girintili) sarılmaz; düzyazı sarılır.
        if line.starts_with("  ") {
            continue;
        }
        assert!(
            line.chars().count() <= width,
            "satır {width} sütunu aşıyor: {line:?}"
        );
    }
}

#[test]
fn width_is_clamped_to_minimum() {
    // 10 sütun istemek MIN_WIDTH'e (40) yuvarlanır — panik yok, taşma yok.
    let text = render_explanation(ErrorCode::E2001, Lang::En, 10, false);
    for line in text.lines().skip(1) {
        if line.starts_with("  ") {
            continue;
        }
        assert!(line.chars().count() <= 40, "satır: {line:?}");
    }
}

#[test]
fn code_blocks_are_indented_and_never_wrapped() {
    for lang in [Lang::En, Lang::Tr] {
        let exp = explanation(lang, ErrorCode::E3001);
        let text = render_explanation(ErrorCode::E3001, lang, 40, false);
        for code_line in exp.example.lines().filter(|l| !l.is_empty()) {
            let indented = format!("  {code_line}");
            assert!(
                text.contains(&indented),
                "{}: kod satırı girintili ve bölünmemiş olmalı: {code_line:?}",
                lang.as_str()
            );
        }
    }
}

#[test]
fn color_flag_paints_title_plain_output_has_no_ansi() {
    let colored = render_explanation(ErrorCode::E3001, Lang::En, 80, true);
    assert!(colored.starts_with("\x1b[1;36m"), "başlık boyanmalı");
    assert!(colored.contains("\x1b[0m"));
    let plain = render_explanation(ErrorCode::E3001, Lang::En, 80, false);
    assert!(!plain.contains('\x1b'), "renksiz çıktıda ANSI olmamalı");
}

#[test]
fn list_contains_every_code_with_categories_en() {
    let list = render_list(Lang::En);
    for &code in ErrorCode::ALL {
        assert!(
            list.contains(code.as_str()),
            "listede {} yok",
            code.as_str()
        );
    }
    for cat in [
        "Syntax",
        "Name resolution",
        "Clock/reset domains",
        "Warnings",
    ] {
        assert!(list.contains(cat), "kategori başlığı yok: {cat}");
    }
}

#[test]
fn list_uses_turkish_categories_in_tr() {
    let list = render_list(Lang::Tr);
    for cat in [
        "Sözdizimi",
        "İsim çözümleme",
        "Saat/sıfırlama alanları",
        "Uyarılar",
    ] {
        assert!(list.contains(cat), "kategori başlığı yok: {cat}");
    }
    assert!(!list.contains("Warnings"), "TR listede EN başlık olmamalı");
}

#[test]
fn category_ranges_match_code_table() {
    assert_eq!(category_name(Lang::En, ErrorCode::E0001), "Syntax");
    assert_eq!(category_name(Lang::En, ErrorCode::E2005), "Type inference");
    assert_eq!(
        category_name(Lang::En, ErrorCode::E2020),
        "Constant evaluation"
    );
    assert_eq!(
        category_name(Lang::En, ErrorCode::E5004),
        "Behavioral contracts"
    );
    assert_eq!(
        category_name(Lang::Tr, ErrorCode::E5004),
        "Davranışsal kontratlar"
    );
    assert_eq!(
        category_name(Lang::En, ErrorCode::E6001),
        "Budget and timing contracts"
    );
    assert_eq!(category_name(Lang::En, ErrorCode::W0010), "Warnings");
    assert_eq!(
        category_name(Lang::Tr, ErrorCode::E3001),
        "Saat/sıfırlama alanları"
    );
    assert_eq!(
        category_name(Lang::Tr, ErrorCode::E9001),
        "Release disiplini"
    );
}

#[test]
fn parse_is_case_insensitive_and_rejects_unknown() {
    assert_eq!(ErrorCode::parse("e3001"), Some(ErrorCode::E3001));
    assert_eq!(ErrorCode::parse(" W2013 "), Some(ErrorCode::W2013));
    assert_eq!(ErrorCode::parse("E9999"), None);
    assert_eq!(ErrorCode::parse(""), None);
}

#[test]
fn suggest_finds_close_code() {
    // "E1000" bir harf farkla E1001'e en yakın (tablo sırasında ilk aday).
    assert_eq!(suggest("E1000"), Some(ErrorCode::E1001));
    assert_eq!(suggest("e2001"), Some(ErrorCode::E2001));
}

#[test]
fn suggest_returns_none_for_distant_input() {
    assert_eq!(suggest("XYZ"), None);
    assert_eq!(suggest("HATA42"), None);
}

#[test]
fn v1_codes_carry_a_note_in_both_languages() {
    use ErrorCode::*;
    for code in [E3006, E3007, E3008, E3009, E4003, E4004] {
        for lang in [Lang::En, Lang::Tr] {
            assert!(
                explanation(lang, code).note.is_some(),
                "{} ({}): V1 notu eksik",
                code.as_str(),
                lang.as_str()
            );
        }
    }
}
