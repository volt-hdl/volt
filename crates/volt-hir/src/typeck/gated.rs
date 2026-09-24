//! Çözümleme hatası tip denetimini kapattığında enum kapsayıcılığı
//! (ADR-0075, ADR-0070 kapısının yedeği).
//!
//! Parser yol desenli, `_`'sız `match`'in E0014'ünü tip denetimine erteler
//! (ADR-0074 Karar 4). Birimde E1xxx varsa tip denetimi koşmaz ve ertelenen
//! tanı kaybolurdu — sayısal match'in E0014'ü ise parser'da verildiği için
//! aynı birimde görünüyordu. Bu geçit tipsiz çalışır ve yalnız KESİN
//! durumu bildirir: sınanan, bildirilen tipi bir enum olan bir tanımın
//! adıdır; muhafızsız kolların bütün yol desenleri o enum'un varyantlarına
//! çözülmüştür; joker yoktur ve eksik varyant vardır. Tanı tip denetimininkiyle
//! birebir aynıdır. Emin olunamayan her durum (çözülmemiş desen, ifade
//! sınanan, başka enum'un varyantı) sessiz kalır: çözümleme hatası
//! düzeltilince tip denetimi karar verir.

use volt_ast::{
    enum_layout, Block, BlockStmt, ElseBranch, ExprKind, Idx, IfStmt, ItemKind, MatchArmBody,
    MatchStmt, ModuleDecl, Pattern, PatternKind, SourceFile, StmtKind, TypeRef,
};
use volt_diagnostics::Diagnostic;

use super::matching::enum_not_exhaustive;
use crate::resolve::{DefId, DefKind, ResolveResult};

/// Kapı kapalıyken (çözümleme hatası) verilebilecek kesin E0014'ler.
pub fn gated_enum_exhaustiveness(ast: &SourceFile, res: &ResolveResult) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for &item in &ast.items {
        let ItemKind::Module(m) = &ast.items_arena[item].kind else {
            continue;
        };
        let walker = Walker {
            ast,
            res,
            module: m,
        };
        for &stmt in &m.body {
            match &ast.stmts[stmt].kind {
                StmtKind::On(on) => walker.block(on.body, &mut out),
                StmtKind::Comb(b) => walker.block(*b, &mut out),
                StmtKind::For(f) => walker.block(f.body, &mut out),
                _ => {}
            }
        }
    }
    out
}

struct Walker<'a> {
    ast: &'a SourceFile,
    res: &'a ResolveResult,
    module: &'a ModuleDecl,
}

impl Walker<'_> {
    fn block(&self, block: Idx<Block>, out: &mut Vec<Diagnostic>) {
        for stmt in &self.ast.blocks[block].stmts {
            match stmt {
                BlockStmt::If(i) => self.if_stmt(i, out),
                BlockStmt::For(f) => self.block(f.body, out),
                BlockStmt::Match(m) => {
                    if let Some(d) = self.check_match(m) {
                        out.push(d);
                    }
                    for arm in &m.arms {
                        if let MatchArmBody::Block(b) = arm.body {
                            self.block(b, out);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn if_stmt(&self, i: &IfStmt, out: &mut Vec<Diagnostic>) {
        self.block(i.then_block, out);
        match &i.else_branch {
            Some(ElseBranch::Block(b)) => self.block(*b, out),
            Some(ElseBranch::If(nested)) => self.if_stmt(nested, out),
            None => {}
        }
    }

    fn check_match(&self, m: &MatchStmt) -> Option<Diagnostic> {
        let decl = self.scrutinee_enum(m)?;
        let mut covered: Vec<String> = Vec::new();
        let mut any_path = false;
        for arm in m.arms.iter().filter(|a| a.guard.is_none()) {
            if !self.cover(arm.pattern, &decl.name.text, &mut covered, &mut any_path)? {
                return None; // joker/bağlama: kapsayıcı
            }
        }
        if !any_path {
            return None; // parser tanısı (literal desenli match) zaten var
        }
        let missing: Vec<String> = decl
            .variants
            .iter()
            .filter(|v| !covered.contains(&v.name.text))
            .map(|v| format!("{}::{}", decl.name.text, v.name.text))
            .collect();
        (!missing.is_empty()).then(|| enum_not_exhaustive(m.span, &decl.name.text, &missing))
    }

    /// Desenin kapsadığı varyant adları. `Some(false)`: joker (kapsayıcı);
    /// `None`: emin olunamayan desen (tanı verilmez).
    fn cover(
        &self,
        pat: Idx<Pattern>,
        enum_name: &str,
        covered: &mut Vec<String>,
        any_path: &mut bool,
    ) -> Option<bool> {
        match &self.ast.patterns[pat].kind {
            PatternKind::Wildcard | PatternKind::Binding(_) => Some(false),
            PatternKind::Or(alts) => {
                for &a in alts {
                    if !self.cover(a, enum_name, covered, any_path)? {
                        return Some(false);
                    }
                }
                Some(true)
            }
            PatternKind::Path { .. } => {
                let def = *self.res.pattern_resolutions.get(&pat)?;
                let DefKind::EnumVariant { parent } = self.res.def_kind(def) else {
                    return None;
                };
                if self.def_name(parent) != enum_name {
                    return None; // E2003 tip denetimine kalır
                }
                *any_path = true;
                covered.push(self.def_name(def).to_string());
                Some(true)
            }
            _ => None,
        }
    }

    fn def_name(&self, def: DefId) -> &str {
        &self.res.defs[def.0 as usize].name
    }

    /// Sınanan bir port/register/wire/tipli `let` adıysa ve bildirilen tipi
    /// enum ise o enum.
    fn scrutinee_enum(&self, m: &MatchStmt) -> Option<&volt_ast::EnumDecl> {
        let ExprKind::Path(_) = &self.ast.exprs[m.scrutinee].kind else {
            return None;
        };
        let def = *self.res.resolutions.get(&m.scrutinee)?;
        let span = self.res.defs[def.0 as usize].span;
        let ty = self.declared_type(span)?;
        enum_layout::enum_of_type(self.ast, ty)
    }

    fn declared_type(&self, name_span: volt_span::Span) -> Option<Idx<TypeRef>> {
        if let Some(p) = self.module.ports.iter().find(|p| p.name.span == name_span) {
            return Some(p.ty);
        }
        self.module
            .body
            .iter()
            .find_map(|&s| match &self.ast.stmts[s].kind {
                StmtKind::Reg(r) if r.name.span == name_span => r.ty,
                StmtKind::Wire(w) if w.name.span == name_span => Some(w.ty),
                StmtKind::Let(l) if l.name.span == name_span => l.ty,
                _ => None,
            })
    }
}
