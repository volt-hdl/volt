//! Kısaltılmış alan ve port yazımında öneri düzenlemeleri (#204).
//!
//! Ayrıştırıcı `S { x }` ve `Inst { x }` yazımını `x: x`'e açar; değer,
//! adın aralığında tek parçalı bir yoldur ([`SourceFile::shorthands`]).
//! Değeri düzenleyen bir öneri (genişlik daraltma `[7:0] as u8`,
//! senkronizör adı, yazım düzeltmesi) o aralığa yazılır; olduğu gibi
//! uygulanırsa alan adını da değiştirir: `S { a[7:0] as u8 }` sözdizimi
//! hatasıdır, `And2 { b_sync }` başka bir porta bağlar. Tanı kapısı bu
//! düzenlemeleri uzun biçime genişletir: `S { a: a[7:0] as u8 }`.
//!
//! Alan ya da port adının kendisini düzelten öneri (E1008 literal alanı,
//! E1009 port adı) adı değiştirir, değeri korur: `Pair { hii }` →
//! `Pair { hi: hii }`.

use volt_ast::{Name, SourceFile};
use volt_diagnostics::{Diagnostic, Edit, EditKind, Suggestion};
use volt_span::Span;

/// Adı düzelten kodlar: düzenleme alan/port adına yöneliktir.
const NAME_FIX_CODES: &[&str] = &["E1008", "E1009"];

/// Kısayol adına düşen düzenlemeleri uzun biçime genişletir. İkinci kez
/// uygulanınca bir şey değiştirmez (genişletilmiş düzenleme zaten
/// `ad: …` / `…: ad` biçimindedir).
pub(crate) fn expand_shorthand_edits(ast: &SourceFile, diagnostics: &mut [Diagnostic]) {
    if ast.shorthands.is_empty() {
        return;
    }
    for d in diagnostics {
        let name_fix = NAME_FIX_CODES.contains(&d.code.as_str());
        for s in &mut d.suggestions {
            expand(s, &ast.shorthands, name_fix);
        }
    }
}

/// `inner`, `outer`'ın içinde mi (aynı dosya; açılım bağlamı `ctx`
/// yok sayılır: klonlanan kısayolun aralığı kaynağınkidir).
fn within(inner: Span, outer: Span) -> bool {
    inner.file == outer.file && outer.start <= inner.start && inner.end <= outer.end
}

fn expand(s: &mut Suggestion, names: &[Name], name_fix: bool) {
    // Düzenlemelerin sırası korunur (ilki birincil düzenlemedir):
    // genişletilen, grubunun ilk düzenlemesinin yerine girer.
    let mut out: Vec<Option<Edit>> = Vec::with_capacity(s.edits.len());
    // (kısayol, `out`'taki yeri, ona düşen düzenlemeler).
    let mut groups: Vec<(&Name, usize, Vec<Edit>)> = Vec::new();
    for e in std::mem::take(&mut s.edits) {
        let hit = (e.kind == EditKind::Replace)
            .then(|| names.iter().find(|n| within(e.span, n.span)))
            .flatten();
        match hit {
            Some(n) if !already_expanded(&e, n) => {
                match groups.iter_mut().find(|(g, _, _)| g.span == n.span) {
                    Some((_, _, edits)) => edits.push(e),
                    None => {
                        out.push(None);
                        groups.push((n, out.len() - 1, vec![e]));
                    }
                }
            }
            _ => out.push(Some(e)),
        }
    }
    for (n, slot, mut edits) in groups {
        // Sağdan sola: önceki konumlar kaymaz.
        edits.sort_by_key(|e| std::cmp::Reverse((e.span.start, e.span.end)));
        let mut text = n.text.clone();
        for e in &edits {
            let a = (e.span.start - n.span.start) as usize;
            let b = (e.span.end - n.span.start) as usize;
            text.replace_range(a..b, &e.text);
        }
        let text = if name_fix {
            format!("{text}: {}", n.text)
        } else {
            format!("{}: {text}", n.text)
        };
        out[slot] = Some(Edit {
            span: n.span,
            text,
            kind: EditKind::Replace,
        });
    }
    s.edits = out.into_iter().flatten().collect();
}

/// Bu kapıdan önce geçmiş düzenleme: kısayolun bütün aralığını uzun
/// biçimle değiştirir.
fn already_expanded(e: &Edit, n: &Name) -> bool {
    e.span.start == n.span.start
        && e.span.end == n.span.end
        && (e.text.starts_with(&format!("{}: ", n.text))
            || e.text.ends_with(&format!(": {}", n.text)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use volt_diagnostics::Applicability;
    use volt_span::FileId;

    fn sp(start: u32, end: u32) -> Span {
        Span::new(FileId(0), start, end)
    }

    fn name(text: &str, start: u32) -> Name {
        Name {
            text: text.to_string(),
            span: sp(start, start + text.len() as u32),
        }
    }

    fn edits(s: &Suggestion) -> Vec<(u32, u32, String)> {
        s.edits
            .iter()
            .map(|e| (e.span.start, e.span.end, e.text.clone()))
            .collect()
    }

    #[test]
    fn an_insertion_after_the_name_becomes_the_long_form() {
        let names = [name("a", 10)];
        let mut s = Suggestion::replace(sp(11, 11), "[7:0] as u8", Applicability::MaybeIncorrect);
        expand(&mut s, &names, false);
        assert_eq!(edits(&s), [(10, 11, "a: a[7:0] as u8".to_string())]);
    }

    #[test]
    fn a_wrapping_pair_becomes_one_long_form_edit() {
        let names = [name("a", 10)];
        let mut s = Suggestion::replace(sp(10, 10), "(", Applicability::MaybeIncorrect)
            .and_replace(sp(11, 11), " as u8)");
        expand(&mut s, &names, false);
        assert_eq!(edits(&s), [(10, 11, "a: (a as u8)".to_string())]);
    }

    #[test]
    fn a_name_fix_renames_the_field_and_keeps_the_value() {
        let names = [name("hii", 20)];
        let mut s = Suggestion::replace(sp(20, 23), "hi", Applicability::MaybeIncorrect);
        expand(&mut s, &names, true);
        assert_eq!(edits(&s), [(20, 23, "hi: hii".to_string())]);
    }

    #[test]
    fn edits_elsewhere_and_line_edits_are_kept_in_order() {
        let names = [name("b", 30)];
        let mut s = Suggestion::line_above(
            sp(30, 31),
            "let b_s = sync(b, clk)",
            Applicability::MaybeIncorrect,
        )
        .and_replace(sp(5, 6), "x")
        .and_replace(sp(30, 31), "b_s");
        expand(&mut s, &names, false);
        assert_eq!(s.edits[0].kind, EditKind::LineAbove);
        assert_eq!(
            edits(&s)[1..],
            [(5, 6, "x".to_string()), (30, 31, "b: b_s".to_string())]
        );
    }

    #[test]
    fn expansion_is_idempotent() {
        let names = [name("a", 10)];
        let mut s = Suggestion::replace(sp(11, 11), "[7:0] as u8", Applicability::MaybeIncorrect);
        expand(&mut s, &names, false);
        let once = edits(&s);
        expand(&mut s, &names, false);
        assert_eq!(edits(&s), once);
        let mut r = Suggestion::replace(sp(10, 11), "hi", Applicability::MaybeIncorrect);
        let names = [name("a", 10)];
        expand(&mut r, &names, true);
        let once = edits(&r);
        expand(&mut r, &names, true);
        assert_eq!(edits(&r), once);
    }

    #[test]
    fn a_name_in_another_file_is_not_touched() {
        let names = [name("a", 10)];
        let other = Span::new(FileId(1), 11, 11);
        let mut s = Suggestion::replace(other, "[7:0] as u8", Applicability::MaybeIncorrect);
        expand(&mut s, &names, false);
        assert_eq!(s.edits[0].text, "[7:0] as u8");
    }
}
