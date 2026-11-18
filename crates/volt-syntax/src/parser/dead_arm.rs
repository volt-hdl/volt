//! Parser katmanında tasarıma yazılmayan `match` kolları (sv-mapping.md
//! §19.1, W2014): kural `volt_ast::match_cover::dead_arms`'ta; burada
//! yalnız desen yaprağının parser'daki anlamı (sabit değeri, enum
//! varyantı) verilir. Donanımı anlatan parser geçişleri (FSM kontrat
//! taraması, `@mmio` yazıcı ve sahiplik taraması, `inout` sürücü taraması,
//! pipeline sınır register'ları) ölü kolun içine girmez (#331, #332).

use volt_ast::match_cover::{dead_arms, value_leaf, Leaf, LitKey};
use volt_ast::{ExprKind, Idx, MatchArm, Pattern, PatternKind, SourceFile};

use super::mono::unroll::{eval_const, visible_const, Consts};

/// Yaprağın değeri: sayı/bool ya da (enum adı, varyant sırası).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Key {
    Value(LitKey),
    Variant(String, usize),
}

/// Kol başına "tasarıma yazılmıyor".
pub(crate) fn dead(ast: &SourceFile, consts: &Consts, arms: &[MatchArm]) -> Vec<bool> {
    let cover = dead_arms(ast, arms, &mut |p| leaf(ast, consts, p));
    cover.dead.iter().map(Option::is_some).collect()
}

/// Tasarıma yazılan kollar, sırasıyla; sabitler birimin üst düzey
/// `const`'larından (kapsamında `Consts` tutmayan geçişler için).
pub(crate) fn live_arms<'m>(ast: &SourceFile, arms: &'m [MatchArm]) -> Vec<&'m MatchArm> {
    let consts = super::mono::unroll::collect_consts(ast);
    live(ast, &consts, arms).collect()
}

/// Tasarıma yazılan kollar, sırasıyla.
pub(crate) fn live<'m>(
    ast: &SourceFile,
    consts: &Consts,
    arms: &'m [MatchArm],
) -> impl Iterator<Item = &'m MatchArm> {
    arms.iter()
        .zip(dead(ast, consts, arms))
        .filter(|(_, d)| !d)
        .map(|(a, _)| a)
}

fn leaf(ast: &SourceFile, consts: &Consts, p: Idx<Pattern>) -> Leaf<Key> {
    match &ast.patterns[p].kind {
        PatternKind::Path { path, args: None } => {
            match super::auto_contract::enum_variant(ast, path) {
                Some((decl, i)) => Leaf::Key(Key::Variant(decl.name.text.clone(), i)),
                None => Leaf::Unknown,
            }
        }
        PatternKind::Literal(e) => {
            if let Some(v) = const_variant(ast, consts, *e) {
                return Leaf::Key(v);
            }
            match value_leaf(ast, p, &mut |e| eval_const(ast, consts, e, 0)) {
                Leaf::Key(k) => Leaf::Key(Key::Value(k)),
                Leaf::Unknown => Leaf::Unknown,
                Leaf::Opaque => Leaf::Opaque,
            }
        }
        PatternKind::Wildcard
        | PatternKind::Or(_)
        | PatternKind::Path { args: Some(_), .. }
        | PatternKind::Tuple(_)
        | PatternKind::Error => Leaf::Unknown,
    }
}

/// Enum tipli `const` adının varyantı (ADR-0085); zincir kuralı
/// `volt_ast::match_cover::const_variant`'ta (SV üretici aynısını okur).
fn const_variant(ast: &SourceFile, consts: &Consts, e: Idx<volt_ast::Expr>) -> Option<Key> {
    volt_ast::match_cover::const_variant(
        ast,
        e,
        &mut |p| match &ast.exprs[p].kind {
            ExprKind::Path(path) => visible_const(ast, consts, p, &path.segments[0].text),
            _ => None,
        },
        &mut |path| {
            super::auto_contract::enum_variant(ast, path)
                .map(|(decl, i)| Key::Variant(decl.name.text.clone(), i))
        },
    )
}
