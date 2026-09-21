//! Benzer ad önerisi (name-resolution.md §8): Levenshtein araması ve
//! "bilinmeyen ad" tanılarının ortak öneri metni ile fix-it'i.

use volt_diagnostics::{lstr, Applicability, Diagnostic, Suggestion};
use volt_span::Span;

/// Eşik: isim uzunluğunun üçte biri (en az 1).
pub fn closest_match(name: &str, candidates: &[String]) -> Option<String> {
    let threshold = (name.chars().count() / 3).max(1);
    candidates
        .iter()
        .filter(|c| !c.starts_with('<'))
        .map(|c| (c, levenshtein(name, c)))
        .filter(|(_, d)| *d <= threshold && *d > 0)
        .min_by_key(|(_, d)| *d)
        .map(|(c, _)| c.clone())
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// Yardım metni: öneri varsa "... mi demek istediniz?", yoksa `fallback`
/// (mevcut adların listesi ya da genel açıklama).
pub(super) fn did_you_mean(suggestion: Option<&String>, fallback: String) -> String {
    match suggestion {
        Some(s) => lstr!(en: "did you mean '{}'?", s;
                         tr: "'{}' mi demek istediniz?", s),
        None => fallback,
    }
}

/// Öneri varsa `span`'daki adı onunla değiştiren fix-it'i ekler.
pub(super) fn with_rename(diag: Diagnostic, span: Span, suggestion: Option<String>) -> Diagnostic {
    match suggestion {
        Some(replacement) => diag.with_suggestion(Suggestion {
            span,
            replacement,
            applicability: Applicability::MaybeIncorrect,
        }),
        None => diag,
    }
}

#[cfg(test)]
mod tests {
    use volt_diagnostics::{Diagnostic, ErrorCode, LabeledSpan};
    use volt_span::{FileId, Span};

    use super::{closest_match, did_you_mean, levenshtein, with_rename};

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn levenshtein_counts_single_character_edits() {
        assert_eq!(levenshtein("count", "count"), 0);
        assert_eq!(levenshtein("count", "coutn"), 2);
        assert_eq!(levenshtein("clk", "clock"), 2);
        assert_eq!(levenshtein("", "abc"), 3);
    }

    #[test]
    fn closest_match_prefers_the_first_of_equally_distant_candidates() {
        let candidates = names(&["data_b", "data_a", "data_c"]);
        assert_eq!(
            closest_match("data_x", &candidates).as_deref(),
            Some("data_b")
        );
    }

    #[test]
    fn closest_match_skips_exact_synthetic_and_distant_names() {
        assert_eq!(closest_match("clk", &names(&["clk"])), None);
        assert_eq!(closest_match("hata", &names(&["<hata>"])), None);
        assert_eq!(closest_match("clk", &names(&["reset_n"])), None);
    }

    #[test]
    fn did_you_mean_falls_back_when_there_is_no_suggestion() {
        let s = "clk".to_string();
        assert!(did_you_mean(Some(&s), "yok".into()).contains("'clk'"));
        assert_eq!(did_you_mean(None, "yok".into()), "yok");
    }

    #[test]
    fn with_rename_adds_a_fix_it_only_for_a_suggestion() {
        let span = Span::new(FileId(0), 0, 3);
        let diag =
            || Diagnostic::error(ErrorCode::E1001, "m", LabeledSpan::primary(span, "l"), "h");
        assert!(with_rename(diag(), span, None).suggestions.is_empty());
        let fixed = with_rename(diag(), span, Some("clk".into()));
        assert_eq!(fixed.suggestions[0].replacement, "clk");
    }
}
