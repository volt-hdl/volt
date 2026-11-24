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

use crate::scope::Res;
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
            // Ad kapsam zinciriyle çözülür (scope.rs).
            ExprKind::Path(p) if p.segments.len() == 1 => match self.resolve(idx) {
                Res::Local(i) => {
                    enum_layout::enum_named(ast, self.proc.locals[i].enum_name.as_deref()?)
                }
                Res::Module => {
                    enum_layout::enum_named(ast, self.enum_sigs.get(&p.segments[0].text)?)
                }
                Res::Const { ty, .. } => self.enum_of_type(ty),
                Res::Loop(_) | Res::Unbound => None,
            },
            ExprKind::Path(p) => self.enum_variant_of_path(p).map(|(d, _)| d),
            ExprKind::Call { callee, args } if self.is_prev_call(*callee) => {
                self.enum_of_expr(*args.first()?)
            }
            // `if` ve match ifadesi: ilk enum tipli kol (ADR-0083,
            // `value_arms`).
            kind @ (ExprKind::If { .. } | ExprKind::Match { .. }) => crate::expr::value_arms(kind)
                .unwrap_or_default()
                .into_iter()
                .find_map(|e| self.enum_of_expr(e)),
            ExprKind::Field { base, field } => {
                let inst = self.module_path(*base)?;
                let module = &self.user_insts.get(inst)?.module;
                let target = ast.items.iter().find_map(|&i| {
                    let ItemKind::Module(m) = &ast.items_arena[i].kind else {
                        return None;
                    };
                    (&m.name.text == module).then_some(m)
                })?;
                let port = target.ports.iter().find(|p| p.name.text == field.text)?;
                self.enum_of_type(port.ty)
            }
            ExprKind::IntLit { .. }
            | ExprKind::BoolLit(_)
            | ExprKind::StringLit(_)
            | ExprKind::Binary { .. }
            | ExprKind::Unary { .. }
            | ExprKind::Index { .. }
            | ExprKind::Range { .. }
            | ExprKind::PartSelect { .. }
            | ExprKind::Call { .. }
            | ExprKind::Cast { .. }
            | ExprKind::StructLit { .. }
            | ExprKind::ArrayLit(_)
            | ExprKind::TupleLit(_)
            | ExprKind::Concat(_)
            | ExprKind::Todo { .. }
            | ExprKind::Error => None,
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
                            en: "rename the signal '{name}' — enum variants map to '<Enum>_<Variant>' localparams";
                            tr: "'{name}' sinyalini yeniden adlandırın — enum varyantları '<Enum>_<Varyant>' localparam'larına iner"
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

/// Desen yaprağının emitter'daki değeri: sayı/bool ya da enum varyantının
/// sırası (desenin enum'u sınananınkiyle aynıdır; değilse HIR E2003 verdi
/// ve emit koşmaz).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ArmKey {
    Value(volt_ast::match_cover::LitKey),
    Variant(usize),
}

/// Enum `match`'inin `case` planı (ADR-0074 Karar 4).
pub(crate) struct EnumMatchPlan {
    /// Kol atlanır (W2014): ilk joker koldan sonra ya da bütün varyantları
    /// önceki kollarca kapsanmış.
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
    /// Tasarıma yazılmayan kollar ve kolların kapsadıkları (sv-mapping.md
    /// §19.1, W2014): kural `volt_ast::match_cover::dead_arms`'ta, HIR
    /// W2014'ü ve sürücü analizini aynı kuraldan verir. Burada yalnız desen
    /// yaprağının emitter'daki anlamı: varyant yolu, enum tipli `const`
    /// adının varyantı (ADR-0085), sayı/bool literali ve sabit adı.
    /// Yaprağın anlamı sınananın tipinden bağımsızdır: varyant yolu ve enum
    /// tipli sabit adı varyanttır, öteki literal ve sabit adı değerdir.
    /// Geçerli bir programda bu HIR'ın anlamıyla aynıdır (sayı üzerinde
    /// varyant ya da enum üzerinde sayı deseni E2003 alır ve emit koşmaz);
    /// modül bağlamı kurulmadan koşan denetimler (E0020) de aynı kolları
    /// görür.
    pub(crate) fn arm_cover(
        &self,
        arms: &[volt_ast::MatchArm],
    ) -> volt_ast::match_cover::ArmCover<ArmKey> {
        use volt_ast::match_cover::{dead_arms, value_leaf, Leaf};
        use volt_ast::PatternKind;
        dead_arms(self.ast, arms, &mut |p| match &self.ast.patterns[p].kind {
            PatternKind::Path { path, args: None } => match self.enum_variant_of_path(path) {
                Some((_, i)) => Leaf::Key(ArmKey::Variant(i)),
                None => Leaf::Opaque,
            },
            PatternKind::Literal(e) => {
                if let Some(i) = self.const_variant(*e) {
                    return Leaf::Key(ArmKey::Variant(i));
                }
                match value_leaf(self.ast, p, &mut |e| self.eval_const(e)) {
                    Leaf::Key(k) => Leaf::Key(ArmKey::Value(k)),
                    Leaf::Unknown => Leaf::Unknown,
                    Leaf::Opaque => Leaf::Opaque,
                }
            }
            PatternKind::Wildcard
            | PatternKind::Or(_)
            | PatternKind::Path { args: Some(_), .. }
            | PatternKind::Tuple(_)
            | PatternKind::Error => Leaf::Unknown,
        })
    }

    /// Kol başına "tasarıma yazılmıyor" ([`Self::arm_cover`]).
    pub(crate) fn dead_arms(&self, arms: &[volt_ast::MatchArm]) -> Vec<bool> {
        let cover = self.arm_cover(arms);
        (0..arms.len()).map(|i| cover.is_dead(i)).collect()
    }

    /// Enum tipli `const` adının varyant sırası; zincir kuralı
    /// `volt_ast::match_cover::const_variant`'ta (parser aynısını okur).
    fn const_variant(&self, e: Idx<Expr>) -> Option<usize> {
        volt_ast::match_cover::const_variant(
            self.ast,
            e,
            &mut |p| self.const_path(p).map(|(_, _, value)| value),
            &mut |path| self.enum_variant_of_path(path).map(|(_, i)| i),
        )
    }

    /// Enum tipli `const` adı deseninin varyantı ad zincirinden çıkmıyorsa
    /// (`if`, çağrı: değeri HIR'ın sabit değerlendiricisi bulur) E0003:
    /// hangi kolları kapsadığı burada bilinemez, HIR o kolları tasarıma
    /// yazmaz ve sürücü saymaz; yazılsalar SV aynı sinyali iki yerden
    /// sürebilirdi. Sayısal sabitin "cannot fold" kuralıyla aynı biçim.
    pub(crate) fn refuse_unresolved_enum_const(&mut self, e: Idx<Expr>) -> bool {
        let Some((name, ty, _)) = self.const_path(e) else {
            return false;
        };
        let Some(decl) = self.enum_of_type(ty) else {
            return false;
        };
        if self.const_variant(e).is_some() {
            return false;
        }
        let enum_name = &decl.name.text;
        let first = decl
            .variants
            .first()
            .map_or("<Variant>", |v| v.name.text.as_str());
        let span = self.ast.exprs[e].span;
        self.error(
            ErrorCode::E0003,
            lstr!(
                en: "not supported yet: enum constant '{name}' as a pattern, whose variant the SystemVerilog emitter cannot resolve";
                tr: "henüz desteklenmiyor: varyantını SystemVerilog üreticisinin çözemediği '{name}' enum sabiti desen olarak"
            ),
            span,
            &lstr!(
                en: "write the constant as a variant (const {name} : {enum_name} = {enum_name}::{first}) or as the name of a constant that is one (see volt explain E0003)";
                tr: "sabiti bir varyant olarak (const {name} : {enum_name} = {enum_name}::{first}) ya da varyant olan bir sabitin adı olarak yazın (bkz. volt explain E0003)"
            ),
        );
        true
    }

    /// Kolların kapsadığı varyantlar: tasarıma yazılmayan kollar ve
    /// kapsayıcı match'in `default` kolu. Muhafızlı kollar kapsamaya
    /// sayılmaz (E0003).
    pub(crate) fn enum_match_plan(
        &self,
        arms: &[volt_ast::MatchArm],
        decl: &EnumDecl,
    ) -> EnumMatchPlan {
        let cover = self.arm_cover(arms);
        let skip = (0..arms.len()).map(|i| cover.is_dead(i)).collect();
        let exhaustive = !cover.catch_all && cover.covered.len() == decl.variants.len();
        let last_named = cover
            .keys
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, k)| k.as_ref().map(|k| (i, k)));
        let (default_arm, default_names) = match last_named {
            Some((i, set)) if exhaustive => {
                let names: Vec<String> = set
                    .iter()
                    .filter_map(|k| match k {
                        ArmKey::Variant(v) => Some(format!(
                            "{}_{}",
                            decl.name.text, decl.variants[*v].name.text
                        )),
                        ArmKey::Value(_) => None,
                    })
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
}
