//! Volt.toml denetimi (ADR-0099, W0025): Volt'un okumadığı anahtar ve
//! bölümler. Okuyucular satır tarayıcıdır (`Manifest::parse`,
//! `UnenforcedLint::from_manifest`, sürücünün `[ui] lang` okuması);
//! tanımadıkları her şeyi sessizce atarlar. Yanlış yazılmış bir anahtar
//! (`scr` yerine `src`) varsayılanı bırakır ve proje kastedilmeyen bir
//! dizinden derlenirdi.

use volt_diagnostics::{lstr, Applicability, Diagnostic, ErrorCode, LabeledSpan, Suggestion};
use volt_span::{FileId, Span};

/// Okunan bölümler ve anahtarları — okuyucularla birebir.
const KNOWN: &[(&str, &[&str])] = &[
    ("package", &["name", "src", "top"]),
    ("test", &["paths"]),
    ("lint", &["unenforced_attributes"]),
    ("ui", &["lang"]),
];

/// Paket yönetimi bölümleri: anahtar adı değil, özelliğin yokluğu.
const DEPENDENCY_SECTIONS: &[&str] = &["dependencies", "dev-dependencies", "build-dependencies"];

/// `text` Volt.toml'un içeriği, `file` onun kaynak haritasındaki kimliği.
pub fn check_manifest(file: FileId, text: &str) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    // Bilinmeyen bölümün anahtarları ayrıca bildirilmez.
    let mut section: Option<Option<(&str, &[&str])>> = None;
    let mut offset = 0usize;
    for raw in text.split_inclusive('\n') {
        let line_start = offset;
        offset += raw.len();
        let code = raw.split('#').next().unwrap_or("");
        let trimmed = code.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lead = code.len() - code.trim_start().len();
        let at = |from: usize, len: usize| {
            Span::new(
                file,
                (line_start + lead + from) as u32,
                (line_start + lead + from + len) as u32,
            )
        };
        if trimmed.starts_with('[') {
            let name = trimmed
                .trim_start_matches('[')
                .trim_end_matches(']')
                .trim()
                .trim_matches('"');
            let name_from = trimmed.find(name).unwrap_or(0);
            let span = at(name_from, name.len());
            section = Some(KNOWN.iter().find(|(s, _)| *s == name).copied());
            if section == Some(None) {
                out.push(unknown_section(name, span));
            }
            continue;
        }
        let Some((key, _)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim().trim_matches('"');
        let span = at(trimmed.find(key).unwrap_or(0), key.len());
        match section {
            // Bölümsüz anahtar: Volt hiçbir üst düzey anahtar okumaz.
            None => out.push(top_level_key(key, span)),
            Some(Some((section_name, keys))) if !keys.contains(&key) => {
                out.push(unknown_key(section_name, keys, key, span));
            }
            Some(_) => {}
        }
    }
    out
}

fn unknown_section(name: &str, span: Span) -> Diagnostic {
    if DEPENDENCY_SECTIONS.contains(&name) {
        return Diagnostic::warning(
            ErrorCode::W0025,
            lstr!(
                en: "[{name}] in Volt.toml: package management is not available yet";
                tr: "Volt.toml'da [{name}]: paket yönetimi henüz yok"
            ),
            LabeledSpan::primary(
                span,
                lstr!(en: "nothing in this section is fetched or used"; tr: "bu bölümdeki hiçbir şey indirilmez ya da kullanılmaz"),
            ),
            lstr!(
                en: "remove the section; share code between the files of one project with 'use' (see volt explain E1011)";
                tr: "bölümü kaldırın; bir projenin dosyaları arasında kodu 'use' ile paylaşın (bkz. volt explain E1011)"
            ),
        );
    }
    let sections: Vec<String> = KNOWN.iter().map(|(s, _)| (*s).to_string()).collect();
    let guess = closest_match(name, &sections);
    let help = match &guess {
        Some(s) => lstr!(en: "did you mean [{s}]?"; tr: "[{s}] mi demek istediniz?"),
        None => lstr!(
            en: "Volt reads [package], [test], [lint] and [ui]; remove the section";
            tr: "Volt [package], [test], [lint] ve [ui] bölümlerini okur; bölümü kaldırın"
        ),
    };
    let diag = Diagnostic::warning(
        ErrorCode::W0025,
        lstr!(en: "unknown section [{name}] in Volt.toml"; tr: "Volt.toml'da bilinmeyen bölüm [{name}]"),
        LabeledSpan::primary(
            span,
            lstr!(en: "Volt ignores this section"; tr: "Volt bu bölümü yok sayar"),
        ),
        help,
    );
    with_rename(diag, span, guess)
}

fn unknown_key(section: &str, keys: &[&str], key: &str, span: Span) -> Diagnostic {
    let known: Vec<String> = keys.iter().map(|k| (*k).to_string()).collect();
    let guess = closest_match(key, &known);
    let help = match &guess {
        Some(k) => lstr!(en: "did you mean '{k}'?"; tr: "'{k}' mi demek istediniz?"),
        None => lstr!(
            en: "Volt reads {} in [{section}]", quoted(keys);
            tr: "Volt [{section}] içinde {} okur", quoted(keys)
        ),
    };
    let diag = Diagnostic::warning(
        ErrorCode::W0025,
        lstr!(
            en: "unknown key '{key}' in [{section}] of Volt.toml";
            tr: "Volt.toml'un [{section}] bölümünde bilinmeyen anahtar '{key}'"
        ),
        LabeledSpan::primary(
            span,
            lstr!(en: "Volt ignores this key"; tr: "Volt bu anahtarı yok sayar"),
        ),
        help,
    );
    with_rename(diag, span, guess)
}

fn top_level_key(key: &str, span: Span) -> Diagnostic {
    Diagnostic::warning(
        ErrorCode::W0025,
        lstr!(
            en: "key '{key}' outside a section in Volt.toml";
            tr: "Volt.toml'da bölüm dışında '{key}' anahtarı"
        ),
        LabeledSpan::primary(
            span,
            lstr!(en: "Volt ignores this key"; tr: "Volt bu anahtarı yok sayar"),
        ),
        lstr!(
            en: "put it in its section, for example 'name' and 'src' under [package]";
            tr: "anahtarı bölümüne yazın, örneğin 'name' ve 'src' [package] altına"
        ),
    )
}

/// Yakın yazımın düzeltmesi (yalnız ad değişir).
fn with_rename(diag: Diagnostic, span: Span, guess: Option<String>) -> Diagnostic {
    match guess {
        // suggestion: w0025_manifest_key_typo, w0025_manifest_section_typo
        Some(name) => diag.with_suggestion(Suggestion::replace(
            span,
            name,
            Applicability::MaybeIncorrect,
        )),
        None => diag,
    }
}

/// En yakın bilinen ad: yer değiştirme (`scr` → `src`) tek düzenleme
/// sayılır (Damerau, en uygun dizilim); eşik kısa adlarda 1, uzunlarda 2.
fn closest_match(name: &str, candidates: &[String]) -> Option<String> {
    let limit = if name.chars().count() <= 4 { 1 } else { 2 };
    candidates
        .iter()
        .map(|c| (osa_distance(name, c), c))
        .filter(|(d, _)| *d > 0 && *d <= limit)
        .min_by_key(|(d, _)| *d)
        .map(|(_, c)| c.clone())
}

fn osa_distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut d = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for j in 0..=b.len() {
        d[0][j] = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut best = (d[i - 1][j] + 1)
                .min(d[i][j - 1] + 1)
                .min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(d[i - 2][j - 2] + 1);
            }
            d[i][j] = best;
        }
    }
    d[a.len()][b.len()]
}

fn quoted(keys: &[&str]) -> String {
    keys.iter()
        .map(|k| format!("'{k}'"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::check_manifest;
    use volt_span::FileId;

    fn codes_and_messages(text: &str) -> Vec<String> {
        check_manifest(FileId(0), text)
            .into_iter()
            .map(|d| d.message)
            .collect()
    }

    #[test]
    fn the_keys_volt_reads_are_silent() {
        let text = "# Volt\n[package]\nname = \"p\"  # yorum\nsrc = \".\"\ntop = [\"A\", \"B\"]\n\n\
                    [test]\npaths = [\"tests\"]\n[lint]\nunenforced_attributes = \"allow\"\n[ui]\nlang = \"tr\"\n";
        assert!(codes_and_messages(text).is_empty());
    }

    #[test]
    fn a_misspelt_key_is_named_with_the_closest_known_key() {
        let d = check_manifest(FileId(0), "[package]\nscr = \"rtl\"\n");
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].message, "unknown key 'scr' in [package] of Volt.toml");
        assert_eq!(d[0].help.as_deref(), Some("did you mean 'src'?"));
        let s = &d[0].suggestions[0].edits[0];
        assert_eq!((s.span.start, s.span.end), (10, 13));
        assert_eq!(s.text, "src");
    }

    #[test]
    fn transposed_letters_count_as_one_edit() {
        assert_eq!(super::osa_distance("scr", "src"), 1);
        assert_eq!(super::osa_distance("pakage", "package"), 1);
        assert_eq!(super::osa_distance("build", "ui"), 3);
    }

    #[test]
    fn dependencies_say_package_management_is_not_available() {
        let m = codes_and_messages("[package]\nname = \"p\"\n[dependencies]\nuart = \"1.0\"\n");
        assert_eq!(
            m,
            ["[dependencies] in Volt.toml: package management is not available yet"]
        );
    }

    #[test]
    fn unknown_sections_and_top_level_keys_are_reported_once() {
        let m = codes_and_messages("edition = \"2026\"\n[pakage]\nname = \"p\"\n[build]\nx = 1\n");
        assert_eq!(
            m,
            [
                "key 'edition' outside a section in Volt.toml",
                "unknown section [pakage] in Volt.toml",
                "unknown section [build] in Volt.toml",
            ]
        );
    }
}
