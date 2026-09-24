//! Birim varyantlı enum'ların SV eşlemesi (ADR-0074 Karar 5, seçenek B).
//!
//! Enum tipli sinyal düz `logic [W-1:0]`'dır; varyant `<Enum>_<Varyant>`
//! adlı modül yerel `localparam` olur. Modül yalnız gövdesinde (ve
//! SVA'sında) ADI GEÇEN varyantları bildirir — Verilator `-Wall`
//! UNUSEDPARAM; enum başına tek satırlık kodlama yorumu tabloyu yine
//! eksiksiz gösterir. Paket + `typedef enum` (seçenek A) reddedildi:
//! Yosys paketi modülden önce okumazsa kırılır (ölçüm ADR-0074).
//!
//! Kodlama kuralı `volt_ast::enum_layout`'tadır (HIR tanılarıyla ortak);
//! geçersiz kodlama HIR'da E2030/E2010 alır, burada sessizce atlanır.

use volt_ast::enum_layout::{self, EnumLayout};
use volt_ast::{EnumDecl, Expr, ExprKind, Idx, ItemKind, Path, TypeRef};
use volt_diagnostics::{lstr, ErrorCode};
use volt_span::Span;

use crate::{Emitter, Sig};

impl<'a> Emitter<'a> {
    /// Geçerli kodlama tablosu (generic/payload'lı/geçersiz enum'da `None`).
    pub(crate) fn enum_layout(&self, decl: &EnumDecl) -> Option<EnumLayout> {
        enum_layout::valid_layout(self.ast, decl, &mut |e| self.eval_const(e))
    }

    /// Enum tipli sinyalin imzası; desteklenmeyen enum E0003.
    pub(crate) fn enum_sig(&mut self, decl: &EnumDecl, span: Span) -> Option<Sig> {
        let name = decl.name.text.clone();
        if !decl.generics.is_empty() {
            self.future(
                span,
                &lstr!(
                    en: "generic enum '{name}' as a signal type";
                    tr: "sinyal tipi olarak generic enum '{name}'"
                ),
            );
            return None;
        }
        if let Some(v) = enum_layout::payload_variant(decl) {
            let variant = v.name.text.clone();
            self.future(
                span,
                &lstr!(
                    en: "enum '{name}' with data-carrying variants ('{name}::{variant}')";
                    tr: "veri taşıyan varyantlı enum '{name}' ('{name}::{variant}')"
                ),
            );
            return None;
        }
        // Geçersiz kodlama HIR'da tanılandı (E2030/E2010/E2021).
        let layout = self.enum_layout(decl)?;
        Some(Sig {
            width: layout.width,
            signed: false,
        })
    }

    /// Tipin adlandırdığı enum (takma adlar izlenir).
    pub(crate) fn enum_of_type(&self, ty: Idx<TypeRef>) -> Option<&'a EnumDecl> {
        enum_layout::enum_of_type(self.ast, ty)
    }

    /// Enum tipli sinyal bildirimini kaydeder (`// State` yorumu, match
    /// sınananı, kod dönüşümü).
    pub(crate) fn note_enum_signal(&mut self, name: &str, ty: Idx<TypeRef>) {
        if let Some(decl) = self.enum_of_type(ty) {
            self.enum_sigs
                .insert(name.to_string(), decl.name.text.clone());
        }
    }

    /// Bildirim satırı sonuna eklenecek `  // State` yorumu (İ5).
    pub(crate) fn enum_comment(&self, name: &str) -> String {
        self.enum_sigs
            .get(name)
            .map_or_else(String::new, |e| format!("  // {e}"))
    }

    /// `State::Idle` yolu (son iki segment) → (enum, varyant sırası).
    pub(crate) fn enum_variant_of_path(&self, path: &Path) -> Option<(&'a EnumDecl, usize)> {
        let n = path.segments.len();
        if n < 2 {
            return None;
        }
        let decl = enum_layout::enum_named(self.ast, &path.segments[n - 2].text)?;
        let idx = decl
            .variants
            .iter()
            .position(|v| v.name.text == path.segments[n - 1].text)?;
        Some((decl, idx))
    }

    /// Varyant yolunun SV adı `<Enum>_<Varyant>`; kullanım kaydedilir
    /// (modül başına `localparam`).
    pub(crate) fn emit_enum_variant(&mut self, path: &Path) -> Option<String> {
        let (decl, idx) = self.enum_variant_of_path(path)?;
        let key = (decl.name.text.clone(), decl.variants[idx].name.text.clone());
        let name = format!("{}_{}", key.0, key.1);
        if !self.enum_used.contains(&key) {
            self.enum_used.push(key);
        }
        Some(name)
    }

    /// Varyant yolunun imzası.
    pub(crate) fn enum_variant_sig(&self, path: &Path) -> Option<Sig> {
        let (decl, _) = self.enum_variant_of_path(path)?;
        let layout = self.enum_layout(decl)?;
        Some(Sig {
            width: layout.width,
            signed: false,
        })
    }

    /// İfadenin enum tipi (kaba çıkarım): enum sinyali, varyant yolu,
    /// enum const'u, `prev(x)`, örnek çıkışı, koşullu ifade.
    pub(crate) fn enum_of_expr(&self, idx: Idx<Expr>) -> Option<&'a EnumDecl> {
        let ast = self.ast;
        match &ast.exprs[idx].kind {
            ExprKind::Path(p) if p.segments.len() == 1 => {
                let name = &p.segments[0].text;
                if let Some(e) = self.enum_sigs.get(name) {
                    return enum_layout::enum_named(ast, e);
                }
                if self.symbols.contains_key(name) {
                    return None;
                }
                let &(ty, _) = self.consts.get(name)?;
                self.enum_of_type(ty)
            }
            ExprKind::Path(p) => self.enum_variant_of_path(p).map(|(d, _)| d),
            ExprKind::Call { callee, args } if self.is_prev_call(*callee) => {
                self.enum_of_expr(*args.first()?)
            }
            ExprKind::If {
                then_expr,
                else_expr,
                ..
            } => self
                .enum_of_expr(*then_expr)
                .or_else(|| self.enum_of_expr(*else_expr)),
            ExprKind::Field { base, field } => {
                let inst = crate::path_single(ast, *base)?;
                let module = &self.user_insts.get(inst)?.module;
                let target = ast
                    .items
                    .iter()
                    .find_map(|&i| match &ast.items_arena[i].kind {
                        ItemKind::Module(m) if &m.name.text == module => Some(m),
                        _ => None,
                    })?;
                let port = target.ports.iter().find(|p| p.name.text == field.text)?;
                self.enum_of_type(port.ty)
            }
            _ => None,
        }
    }

    /// Modülün kullandığı varyantların `localparam`'ları (enum bildirim
    /// sırası, varyant bildirim sırası — determinizm). Aynı adlı modül
    /// sinyali varsa E1003 (SV ad çakışması).
    pub(crate) fn enum_localparams(
        &mut self,
        used: &[(String, String)],
        clash_span: Span,
    ) -> Option<String> {
        if used.is_empty() {
            return None;
        }
        let ast = self.ast;
        let mut lines = Vec::new();
        for &item in &ast.items {
            let ItemKind::Enum(decl) = &ast.items_arena[item].kind else {
                continue;
            };
            let names: Vec<(usize, String)> = decl
                .variants
                .iter()
                .enumerate()
                .filter(|(_, v)| {
                    used.iter()
                        .any(|(e, n)| *e == decl.name.text && *n == v.name.text)
                })
                .map(|(i, v)| (i, format!("{}_{}", decl.name.text, v.name.text)))
                .collect();
            if names.is_empty() {
                continue;
            }
            let Some(layout) = self.enum_layout(decl) else {
                continue;
            };
            let table: Vec<String> = decl
                .variants
                .iter()
                .zip(&layout.values)
                .map(|(v, code)| format!("{} = {code}", v.name.text))
                .collect();
            lines.push(format!(
                "    // enum {} : {}",
                decl.name.text,
                table.join(", ")
            ));
            let sig = Sig {
                width: layout.width,
                signed: false,
            };
            let pad = names.iter().map(|(_, n)| n.len()).max().unwrap_or(0);
            for (i, name) in &names {
                if self.symbols.contains_key(name)
                    || self.user_insts.contains_key(name)
                    || self.builtin_insts.contains_key(name)
                {
                    self.error(
                        ErrorCode::E1003,
                        lstr!(
                            en: "the SystemVerilog name '{name}' of an enum variant clashes with a signal of this module";
                            tr: "bir enum varyantının SystemVerilog adı '{name}' bu modülün bir sinyaliyle çakışıyor"
                        ),
                        clash_span,
                        &lstr!(
                            en: "rename the signal '{name}' — enum variants map to '<Enum>_<Variant>' localparams (ADR-0074)";
                            tr: "'{name}' sinyalini yeniden adlandırın — enum varyantları '<Enum>_<Varyant>' localparam'larına iner (ADR-0074)"
                        ),
                    );
                }
                lines.push(format!(
                    "    localparam {} {name:<pad$} = {}'d{};",
                    sig.decl_type(),
                    layout.width,
                    layout.values[*i]
                ));
            }
        }
        (!lines.is_empty()).then(|| lines.join("\n"))
    }
}

/// Enum `match`'inin `case` planı (ADR-0074 Karar 4).
pub(crate) struct EnumMatchPlan {
    /// Kol atlanır: bütün varyantları önceki kollarca kapsanmış (W2014).
    pub(crate) skip: Vec<bool>,
    /// Kapsayıcı, `_`'sız match'te `default:` olan son adlı kol.
    pub(crate) default_arm: Option<usize>,
    /// O kolun varyantlarının SV adları (yorum için).
    pub(crate) default_names: String,
    /// Kodlamada hiçbir varyanta ait olmayan kod var mı; yoğun (`2^n`
    /// varyantlı) enum'da `default` yalnız son kolun varyantlarıdır.
    pub(crate) has_invalid_codes: bool,
}

impl<'a> Emitter<'a> {
    /// Kolların kapsadığı varyantlar: erişilemez kollar ve kapsayıcı
    /// match'in `default` kolu. Muhafızlı kollar kapsamaya sayılmaz
    /// (E0003); joker ya da bağlama deseni sonrasında plan yapılmaz.
    pub(crate) fn enum_match_plan(
        &self,
        m: &volt_ast::MatchStmt,
        decl: &EnumDecl,
    ) -> EnumMatchPlan {
        let mut skip = vec![false; m.arms.len()];
        let mut covered: Vec<usize> = Vec::new();
        let mut wildcard = false;
        let mut last_named = None;
        for (i, arm) in m.arms.iter().enumerate() {
            if arm.guard.is_some() {
                continue;
            }
            let (set, wild) = self.pattern_variants(arm.pattern, decl);
            if !wildcard && !wild && !set.is_empty() && set.iter().all(|v| covered.contains(v)) {
                skip[i] = true;
                continue;
            }
            wildcard |= wild;
            if !wild {
                last_named = Some((i, set.clone()));
            }
            for v in set {
                if !covered.contains(&v) {
                    covered.push(v);
                }
            }
        }
        let exhaustive = !wildcard && covered.len() == decl.variants.len();
        let (default_arm, default_names) = match last_named {
            Some((i, set)) if exhaustive => {
                let names: Vec<String> = set
                    .iter()
                    .map(|&v| format!("{}_{}", decl.name.text, decl.variants[v].name.text))
                    .collect();
                (Some(i), names.join(", "))
            }
            _ => (None, String::new()),
        };
        let has_invalid_codes = self.enum_layout(decl).is_none_or(|l| !l.is_dense());
        EnumMatchPlan {
            skip,
            default_arm,
            default_names,
            has_invalid_codes,
        }
    }

    /// Desenin adlandırdığı varyant sıraları ve joker olup olmadığı.
    fn pattern_variants(&self, pat: Idx<volt_ast::Pattern>, decl: &EnumDecl) -> (Vec<usize>, bool) {
        use volt_ast::PatternKind;
        match &self.ast.patterns[pat].kind {
            PatternKind::Path { path, args: None } => match self.enum_variant_of_path(path) {
                Some((d, i)) if d.name.text == decl.name.text => (vec![i], false),
                _ => (Vec::new(), true),
            },
            PatternKind::Or(alts) => {
                let mut set = Vec::new();
                let mut wild = false;
                for &a in alts {
                    let (s, w) = self.pattern_variants(a, decl);
                    set.extend(s);
                    wild |= w;
                }
                (set, wild)
            }
            _ => (Vec::new(), true),
        }
    }
}
