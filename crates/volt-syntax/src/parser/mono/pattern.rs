//! Match kolları ve desen klonu (ADR-0041) — `Cloner`'ın desen yüzü.

use volt_ast::{FieldPattern, Idx, MatchArm, MatchArmBody, Pattern, PatternArgs, PatternKind};

use super::clone::Cloner;

impl Cloner<'_> {
    /// Kolların sahipli kopyası (Idx alanları Copy, gövde türü küçük).
    pub(super) fn copy_arms(&self, arms: &[MatchArm]) -> Vec<MatchArm> {
        arms.iter()
            .map(|a| MatchArm {
                span: a.span,
                pattern: a.pattern,
                guard: a.guard,
                body: match &a.body {
                    MatchArmBody::Block(b) => MatchArmBody::Block(*b),
                    MatchArmBody::Expr(e) => MatchArmBody::Expr(*e),
                },
            })
            .collect()
    }

    pub(super) fn clone_arm(&mut self, a: &MatchArm) -> MatchArm {
        MatchArm {
            span: self.tag(a.span),
            pattern: self.clone_pattern(a.pattern),
            guard: a.guard.map(|g| self.clone_expr(g)),
            body: match &a.body {
                MatchArmBody::Block(b) => MatchArmBody::Block(self.clone_block(*b)),
                MatchArmBody::Expr(e) => MatchArmBody::Expr(self.clone_expr(*e)),
            },
        }
    }

    fn clone_pattern(&mut self, p: Idx<Pattern>) -> Idx<Pattern> {
        let span = self.tag(self.ast.patterns[p].span);
        let kind = match &self.ast.patterns[p].kind {
            PatternKind::Wildcard => PatternKind::Wildcard,
            PatternKind::Error => self.recovery(PatternKind::Error),
            PatternKind::Binding(n) => {
                let n = n.clone();
                PatternKind::Binding(self.tag_name(&n))
            }
            PatternKind::Literal(e) => {
                let e = *e;
                PatternKind::Literal(self.clone_expr(e))
            }
            PatternKind::Tuple(items) => {
                let items = items.clone();
                PatternKind::Tuple(items.iter().map(|&i| self.clone_pattern(i)).collect())
            }
            PatternKind::Or(alts) => {
                let alts = alts.clone();
                PatternKind::Or(alts.iter().map(|&i| self.clone_pattern(i)).collect())
            }
            PatternKind::Path { path, args } => {
                let path = path.clone();
                let args = self.copy_pattern_args(args.as_ref());
                let path = self.tag_path(&path);
                PatternKind::Path {
                    path,
                    args: args.map(|a| self.clone_pattern_args(a)),
                }
            }
        };
        self.ast
            .write(|ast| ast.patterns.alloc(Pattern { span, kind }))
    }

    fn copy_pattern_args(&self, args: Option<&PatternArgs>) -> Option<PatternArgs> {
        args.map(|a| match a {
            PatternArgs::Tuple(items) => PatternArgs::Tuple(items.clone()),
            PatternArgs::Struct(fields) => PatternArgs::Struct(
                fields
                    .iter()
                    .map(|f| FieldPattern {
                        span: f.span,
                        name: f.name.clone(),
                        pattern: f.pattern,
                    })
                    .collect(),
            ),
        })
    }

    fn clone_pattern_args(&mut self, args: PatternArgs) -> PatternArgs {
        match args {
            PatternArgs::Tuple(items) => {
                PatternArgs::Tuple(items.iter().map(|&i| self.clone_pattern(i)).collect())
            }
            PatternArgs::Struct(fields) => PatternArgs::Struct(
                fields
                    .into_iter()
                    .map(|f| FieldPattern {
                        span: self.tag(f.span),
                        name: self.tag_name(&f.name),
                        pattern: f.pattern.map(|p| self.clone_pattern(p)),
                    })
                    .collect(),
            ),
        }
    }
}
