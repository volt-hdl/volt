//! `volt explain` açıklama tabanı testleri (cli-contract.md §9).
//!
//! Kapsam: 120 kodun iki dilde de tam açıklaması, §9 bölüm yapısı,
//! genişliğe göre sarma, renk, --list gruplaması ve kod önerisi.

use volt_diagnostics::explain::{
    category_name, explanation, render_explanation, render_list, suggest, DEFAULT_WIDTH,
};
use volt_diagnostics::{ErrorCode, Lang};

/// Spec'teki toplam kod sayısı — kod eklenince bilinçli olarak güncellenir.
/// (E0014, ADR-0032 ile; E8501-E8506, ADR-0033 ile; E5010, ADR-0037 ile;
/// E5011-E5016, ADR-0038 ile; E3013/E4005, ADR-0039 ile; E5017, ADR-0040 ile; E0015/E4006, ADR-0044 ile; E3014, ADR-0047 ile; E4007, ADR-0050 ile;
/// E4008/W3007, ADR-0051 ile; E0016/W3008, ADR-0052 ile; E0017/W0022,
/// ADR-0054 ile; E8507-E8511, ADR-0058 ile; E8512, ADR-0059 ile; E9003/E9004,
/// ADR-0063 ile; W5001, ADR-0064 ile; W3009/W3010, ADR-0065 ile;
/// E4009/E4010, ADR-0067 ile; E1013/E8513, ADR-0078 ile; E1014, ADR-0079 ile;
/// E0018, ADR-0080 ile; E2015/E2016/E3015/E4013, ADR-0081 ile eklendi.)
const CODE_COUNT: usize = 150;

#[test]
fn all_codes_present_120_of_120() {
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
    // E3009 artık V1 değil: ADR-0052 ile uygulandı.
    for code in [E3006, E3007, E3008, E4003, E4004] {
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

#[test]
fn e3014_explanations_mention_extern_in_both_languages() {
    let en = explanation(Lang::En, ErrorCode::E3014);
    assert!(en.why.contains("extern"), "{}", en.why);
    assert!(en.example.contains("@"), "{}", en.example);
    let tr = explanation(Lang::Tr, ErrorCode::E3014);
    assert!(tr.why.contains("extern"), "{}", tr.why);
    assert!(tr.example.contains("@"), "{}", tr.example);
}

#[test]
fn w0021_explanations_describe_unenforced_attributes_in_both_languages() {
    let en = explanation(Lang::En, ErrorCode::W0021);
    assert!(en.title.contains("not yet enforced"), "{}", en.title);
    assert!(en.why.contains("@allow(unenforced)"), "{}", en.why);
    assert!(en.why.contains("unenforced_attributes"), "{}", en.why);
    // ADR-0054: örnek artık @budget — @timing uygulanıyor.
    assert!(en.example.contains("@budget"), "{}", en.example);
    assert!(
        !en.why.contains("@timing,"),
        "@timing listede olmamalı: {}",
        en.why
    );
    assert!(en.fix.contains("@allow(unenforced)"), "{}", en.fix);
    let tr = explanation(Lang::Tr, ErrorCode::W0021);
    assert!(tr.title.contains("uygulanmıyor"), "{}", tr.title);
    assert!(tr.why.contains("@allow(unenforced)"), "{}", tr.why);
    assert!(tr.example.contains("@budget"), "{}", tr.example);
    assert!(tr.fix.contains("@allow(unenforced)"), "{}", tr.fix);
}

#[test]
fn w0021_note_says_timing_family_is_enforced_since_adr_0054() {
    for lang in [Lang::En, Lang::Tr] {
        let note = explanation(lang, ErrorCode::W0021).note.unwrap();
        assert!(
            note.contains("ADR-0054") && note.contains("--emit=sdc"),
            "{note}"
        );
    }
}

#[test]
fn e0017_and_w0022_explanations_in_both_languages() {
    let en = explanation(Lang::En, ErrorCode::E0017);
    assert!(en.title.contains("timing constraint"), "{}", en.title);
    assert!(en.why.contains("@timing(clk >= 100.mhz)"), "{}", en.why);
    assert!(en.why.contains("max_delay(a, b) <= 5.ns"), "{}", en.why);
    assert!(en.example.contains("E0017"), "{}", en.example);
    assert!(en.note.unwrap().contains("_reg*"));
    let tr = explanation(Lang::Tr, ErrorCode::E0017);
    assert!(tr.title.contains("zamanlama kısıtı"), "{}", tr.title);
    assert!(tr.why.contains("@timing(clk >= 100.mhz)"), "{}", tr.why);

    let en = explanation(Lang::En, ErrorCode::W0022);
    assert!(en.title.contains("no create_clock"), "{}", en.title);
    assert!(en.summary.contains("--emit=sdc"), "{}", en.summary);
    assert!(en.fix.contains("frequency = 25_175.khz"), "{}", en.fix);
    let tr = explanation(Lang::Tr, ErrorCode::W0022);
    assert!(tr.title.contains("create_clock"), "{}", tr.title);
    assert!(tr.fix.contains("frequency = 25_175.khz"), "{}", tr.fix);
}

#[test]
fn e0018_explanation_in_both_languages() {
    let en = explanation(Lang::En, ErrorCode::E0018);
    assert!(en.title.contains("too deep"), "{}", en.title);
    assert!(en.summary.contains("256"), "{}", en.summary);
    assert!(en.why.contains("stack overflow"), "{}", en.why);
    assert!(en.why.contains("else if"), "{}", en.why);
    assert!(en.example.contains("E0018"), "{}", en.example);
    assert!(en.note.unwrap().contains("64 MB"));
    let tr = explanation(Lang::Tr, ErrorCode::E0018);
    assert!(tr.title.contains("çok derin"), "{}", tr.title);
    assert!(tr.why.contains("yığın taşması"), "{}", tr.why);
    assert!(tr.example.contains("E0018"), "{}", tr.example);
}

#[test]
fn w5001_explanation_in_both_languages() {
    let en = explanation(Lang::En, ErrorCode::W5001);
    assert!(en.title.contains("monitored in simulation"), "{}", en.title);
    assert!(en.summary.contains("ADR-0064"), "{}", en.summary);
    assert!(en.why.contains("E0003"), "{}", en.why);
    assert!(en.note.unwrap().contains("--no-contracts"));
    let tr = explanation(Lang::Tr, ErrorCode::W5001);
    assert!(tr.title.contains("simülasyonda"), "{}", tr.title);
    assert!(tr.note.unwrap().contains("--no-contracts"));
}

#[test]
fn e9003_and_e9004_explanations_in_both_languages() {
    let en = explanation(Lang::En, ErrorCode::E9003);
    assert!(en.title.contains("Register map drift"), "{}", en.title);
    assert!(en.why.contains("--against"), "{}", en.why);
    assert!(en.why.contains("regmap-hash"), "{}", en.why);
    assert!(en.example.contains("E9003"), "{}", en.example);
    assert!(en.note.unwrap().contains("--format json"));
    let tr = explanation(Lang::Tr, ErrorCode::E9003);
    assert!(tr.title.contains("ayrışması"), "{}", tr.title);
    assert!(tr.why.contains("bayat"), "{}", tr.why);

    let en = explanation(Lang::En, ErrorCode::E9004);
    assert!(en.title.contains("Not a Volt-generated"), "{}", en.title);
    assert!(en.why.contains("Generated by Volt"), "{}", en.why);
    let tr = explanation(Lang::Tr, ErrorCode::E9004);
    assert!(tr.why.contains("açıkça reddedilir"), "{}", tr.why);
    assert_eq!(
        category_name(Lang::En, ErrorCode::E9003),
        "Release discipline"
    );
}

#[test]
fn w0021_explanation_notes_the_adr_in_both_languages() {
    for lang in [Lang::En, Lang::Tr] {
        let exp = explanation(lang, ErrorCode::W0021);
        let note = exp
            .note
            .unwrap_or_else(|| panic!("{}: NOT bölümü yok", lang.as_str()));
        assert!(note.contains("ADR-0048"), "{note}");
    }
}

#[test]
fn w3003_explanation_names_async_dual_port_ram_in_both_languages() {
    // ADR-0049: rastgele erişimli veri için dördüncü alternatif.
    for lang in [Lang::En, Lang::Tr] {
        let exp = explanation(lang, ErrorCode::W3003);
        assert!(
            exp.why.contains("AsyncDualPortRam"),
            "{lang:?}: {}",
            exp.why
        );
    }
}

#[test]
fn adr_0052_explanations_describe_trust_flow_in_both_languages() {
    for lang in [Lang::En, Lang::Tr] {
        let e = explanation(lang, ErrorCode::E3009);
        assert!(e.why.contains("declassify"), "{}", e.why);
        assert!(e.why.contains("trust_level"), "{}", e.why);
        assert!(e.example.contains("@SecureCore"), "{}", e.example);
        assert!(e.note.is_some(), "E3009 K11 notu taşımalı");
        let w = explanation(lang, ErrorCode::W3008);
        assert!(w.why.contains("E3009"), "{}", w.why);
        assert!(w.example.contains("declassify"), "{}", w.example);
        let r = explanation(lang, ErrorCode::E0016);
        assert!(r.why.contains("W3008"), "{}", r.why);
        assert!(r.example.contains("E0016"), "{}", r.example);
    }
}

#[test]
fn e3009_message_no_longer_marked_v1() {
    assert!(!ErrorCode::E3009.description().contains("V1"));
    assert!(ErrorCode::E0016.description().contains("declassify"));
    assert!(ErrorCode::W3008.is_warning());
}

#[test]
fn adr_0065_rdc_explanations_point_to_the_raw_reset_port() {
    for lang in [Lang::En, Lang::Tr] {
        let e = explanation(lang, ErrorCode::E3003);
        // Eski metin bir CDC örneğiydi (dst <= sync(...)); yeni metin reset'in kendisi.
        assert!(!e.fix.contains("sync(src"), "{}", e.fix);
        assert!(e.fix.contains("reset(async, active_low)"), "{}", e.fix);
        assert!(e.why.contains("ADR-0065"), "{}", e.why);
        assert!(e.note.is_some_and(|n| n.contains("W3010")));
        for w in [ErrorCode::W3009, ErrorCode::W3010] {
            let x = explanation(lang, w);
            assert!(x.fix.contains(": reset("), "{}: {}", w.as_str(), x.fix);
            assert!(w.is_warning());
        }
        assert!(explanation(lang, ErrorCode::W3010).why.contains("ADR-0065"));
        // R5' kararı (ADR-0065): uyarı kalıcı, gerekçesi hatasız biçimi
        // olmayan hiyerarşi (E3003 R6) — "geçici" dili kalktı.
        let why = explanation(lang, ErrorCode::W3010).why;
        assert!(
            why.contains("E3003") && why.contains("AsyncDualPortRam"),
            "{why}"
        );
        assert!(
            !why.contains("temporary") && !why.contains("geçici"),
            "{why}"
        );
    }
    assert!(ErrorCode::W3009.description().contains("ADR-0065"));
}

#[test]
fn e0003_explains_the_fn_bit_select_limit_and_its_fix_in_both_languages() {
    // ADR-0081 Aşama 3: comb/blok içi for/kontratta bit seçilen parametreye
    // sinyal adı olmayan argüman E0003 verir; çözüm modül düzeyi let.
    for lang in [Lang::En, Lang::Tr] {
        let note = explanation(lang, ErrorCode::E0003)
            .note
            .expect("E0003 notu");
        assert!(note.contains("ADR-0081"), "{note}");
        assert!(
            note.contains("`comb`") && note.contains("`x[3:0]`"),
            "{note}"
        );
        assert!(note.contains("`let t = f(a ^ b)`"), "{note}");
    }
}
