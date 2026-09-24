//! İfadeler ve tip referansları (name-resolution.md §3): ağaç yürüyüşü,
//! struct literali alanları (E1008) ve prev() kullanım kuralı (E5017,
//! ADR-0040).

use volt_ast::{
    ArrayLitKind, Expr, ExprKind, FieldInit, GenericArg, Idx, ItemKind, Name, Path, TypeRef,
    TypeRefKind,
};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan};

use super::def::{BuiltinKind, DefId, DefKind};
use super::prelude::is_widened_int_type;
use super::scope::ScopeId;
use super::suggest::{closest_match, did_you_mean};
use super::Resolver;

impl Resolver<'_> {
    pub(super) fn resolve_type(&mut self, ty_idx: Idx<TypeRef>, scope: ScopeId) {
        let ty = &self.ast.types[ty_idx];
        match &ty.kind {
            TypeRefKind::Bits(e) | TypeRefKind::UIntN(e) | TypeRefKind::SIntN(e) => {
                self.resolve_expr(*e, scope)
            }
            TypeRefKind::Array { elem, len } => {
                self.resolve_type(*elem, scope);
                self.resolve_expr(*len, scope);
            }
            TypeRefKind::Tuple(items) => {
                for &t in items.clone().iter() {
                    self.resolve_type(t, scope);
                }
            }
            TypeRefKind::Path { path, args } => {
                // u65..uN / i65..iN genişletilmiş tipleri parser Path olarak
                // taşır (widened types) — bunlar isim değil, yerleşik ailedir.
                if path.segments.len() == 1 && is_widened_int_type(&path.segments[0].text) {
                    return;
                }
                let def = self.resolve_path(&path.clone(), scope);
                self.type_resolutions.insert(ty_idx, def);
                for arg in args {
                    match arg {
                        GenericArg::Type(t) => self.resolve_type(*t, scope),
                        GenericArg::Const(e) => self.resolve_expr(*e, scope),
                    }
                }
            }
            // Yerleşik tipler (bool, clock, uN, bits, Trit, reset) isim değildir.
            _ => {}
        }
    }

    pub(super) fn resolve_expr(&mut self, expr_idx: Idx<Expr>, scope: ScopeId) {
        let expr = &self.ast.exprs[expr_idx];
        match &expr.kind {
            ExprKind::Path(path) => {
                let def = self.resolve_path(&path.clone(), scope);
                self.resolutions.insert(expr_idx, def);
            }
            ExprKind::Binary { lhs, rhs, .. } => self.resolve_exprs(&[*lhs, *rhs], scope),
            ExprKind::Unary { operand, .. } => self.resolve_expr(*operand, scope),
            ExprKind::Index { base, index } => self.resolve_exprs(&[*base, *index], scope),
            ExprKind::Range { base, hi, lo } => self.resolve_exprs(&[*base, *hi, *lo], scope),
            ExprKind::PartSelect {
                base, start, width, ..
            } => self.resolve_exprs(&[*base, *start, *width], scope),
            ExprKind::Field { base, field } => self.resolve_field(*base, &field.clone(), scope),
            ExprKind::Call { callee, args } => {
                self.resolve_call(expr_idx, *callee, &args.clone(), scope);
            }
            ExprKind::Cast { expr: inner, ty } => {
                self.resolve_expr(*inner, scope);
                self.resolve_type(*ty, scope);
            }
            ExprKind::If {
                cond,
                then_expr,
                else_expr,
            } => self.resolve_exprs(&[*cond, *then_expr, *else_expr], scope),
            ExprKind::Match { scrutinee, arms } => {
                self.resolve_expr(*scrutinee, scope);
                for arm in arms {
                    self.resolve_arm(arm, scope);
                }
            }
            ExprKind::StructLit { path, fields } => {
                self.resolve_struct_lit(expr_idx, &path.clone(), fields, scope);
            }
            ExprKind::ArrayLit(ArrayLitKind::List(items)) | ExprKind::TupleLit(items) => {
                self.resolve_exprs(&items.clone(), scope);
            }
            ExprKind::ArrayLit(ArrayLitKind::Repeat { value, count }) => {
                self.resolve_exprs(&[*value, *count], scope);
            }
            ExprKind::IntLit { .. }
            | ExprKind::BoolLit(_)
            | ExprKind::StringLit(_)
            | ExprKind::Todo { .. }
            | ExprKind::Concat(_)
            | ExprKind::Error => {}
        }
    }

    fn resolve_call(
        &mut self,
        call: Idx<Expr>,
        callee: Idx<Expr>,
        args: &[Idx<Expr>],
        scope: ScopeId,
    ) {
        self.resolve_expr(callee, scope);
        self.resolve_exprs(args, scope);
        self.check_prev_call(callee, args, call);
    }

    /// Alt ifadeleri verilen (kaynak) sırayla çözer.
    fn resolve_exprs(&mut self, exprs: &[Idx<Expr>], scope: ScopeId) {
        for &e in exprs {
            self.resolve_expr(e, scope);
        }
    }

    /// `taban.alan` — taban bir instance'a çözülüyorsa (uart.busy) port
    /// adı doğrulanır.
    fn resolve_field(&mut self, base: Idx<Expr>, field: &Name, scope: ScopeId) {
        if let ExprKind::Path(p) = &self.ast.exprs[base].kind {
            if let [seg] = p.segments.as_slice() {
                let span = volt_span::Span {
                    end: field.span.end,
                    ..seg.span
                };
                if self.whole_bundle_field(&seg.text.clone(), field, span, scope) {
                    return;
                }
            }
        }
        self.resolve_expr(base, scope);
        if let Some(&base_def) = self.resolutions.get(&base) {
            if let Some(&target) = self.instance_module.get(&base_def) {
                self.check_port_exists(target, field);
            } else if let Some(&prim) = self.instance_builtin.get(&base_def) {
                self.check_builtin_field(prim, field);
            }
        }
    }

    /// `req.data` — düzleştirilmiş bir Handshake/bundle portunun alt
    /// alanının TAMAMI değer olarak (ADR-0077 Karar 1): parser yalnız
    /// yaprakları porta açar (`req_data_a`), ara düğüm bir sinyal
    /// değildir. Taban adı kapsamda yoksa ve bir düzleştirme kaynağının
    /// yolu `alan.` ile başlıyorsa E0003 verilir (önceden yanıltıcı
    /// `E1001 undefined name`); `true` → çözümleme atlanır.
    pub(super) fn whole_bundle_field(
        &mut self,
        base: &str,
        field: &Name,
        span: volt_span::Span,
        scope: ScopeId,
    ) -> bool {
        if self.lookup_visible(base, scope).is_some() {
            return false;
        }
        let prefix = format!("{}.", field.text);
        let Some(origin) = self
            .bundle_origins
            .iter()
            .find(|o| o.port.text == base && o.path.starts_with(&prefix))
        else {
            return false;
        };
        let example = format!("{base}.{}", origin.path);
        let whole = format!("{base}.{}", field.text);
        let what = if origin.bundle == "Handshake" {
            lstr!(en: "the whole Handshake payload '{whole}' as a value; access its fields ('{example}')";
                  tr: "Handshake payload'ının tamamı ('{whole}') değer olarak; alanlarına erişin ('{example}')")
        } else {
            lstr!(en: "the whole bundle field '{whole}' as a value; access its fields ('{example}')";
                  tr: "bundle alanının tamamı ('{whole}') değer olarak; alanlarına erişin ('{example}')")
        };
        self.diagnostics.push(Diagnostic::error(
            ErrorCode::E0003,
            lstr!(en: "not supported yet: {what}"; tr: "henüz desteklenmiyor: {what}"),
            LabeledSpan::primary(span, String::new()),
            lstr!(
                en: "this is valid Volt but has no SystemVerilog mapping yet; express it with supported constructs (see volt explain E0003)";
                tr: "bu geçerli Volt ama henüz SystemVerilog eşlemesi yok; desteklenen yapılarla yazın (bkz. volt explain E0003)"
            ),
        ));
        true
    }

    fn resolve_struct_lit(
        &mut self,
        expr_idx: Idx<Expr>,
        path: &Path,
        fields: &[FieldInit],
        scope: ScopeId,
    ) {
        let def = self.resolve_path(path, scope);
        self.resolutions.insert(expr_idx, def);
        self.check_struct_fields(def, fields);
        for f in fields.iter() {
            match f.value {
                Some(e) => self.resolve_expr(e, scope),
                // `Foo { x }` kısayolu — yerel isim alan adıyla aynı.
                None => {
                    let _ = self.resolve_simple(&f.name.clone(), scope, true);
                }
            }
        }
    }

    fn check_struct_fields(&mut self, def: DefId, fields: &[FieldInit]) {
        if self.def(def).kind != DefKind::Struct {
            return;
        }
        let Some(&item_idx) = self.item_of_def.get(&def) else {
            return;
        };
        let ItemKind::Struct(s) = &self.ast.items_arena[item_idx].kind else {
            return;
        };
        let known: Vec<String> = s.fields.iter().map(|f| f.name.text.clone()).collect();
        let struct_name = s.name.text.clone();
        let mut diags = Vec::new();
        for f in fields {
            if !known.contains(&f.name.text) {
                diags.push(Diagnostic::error(
                    ErrorCode::E1008,
                    lstr!(en: "struct '{}' has no field '{}'", struct_name, f.name.text;
                          tr: "'{}' yapısında '{}' alanı yok", struct_name, f.name.text),
                    LabeledSpan::primary(
                        f.name.span,
                        lstr!(en: "unknown field"; tr: "bilinmeyen alan"),
                    ),
                    did_you_mean(
                        closest_match(&f.name.text, &known).as_ref(),
                        lstr!(en: "available fields: {}", known.join(", ");
                              tr: "mevcut alanlar: {}", known.join(", ")),
                    ),
                ));
            }
        }
        self.diagnostics.extend(diags);
    }

    /// E5017: prev(x[, N]) yalnız kontrat ifadesinde ve (sinyal) ya da
    /// (sinyal, pozitif literal) argümanlarıyla geçerlidir. Beş parça:
    /// kod, konum, açıklama, öneri (RTL'de reg), ADR referansı (explain).
    fn check_prev_call(&mut self, callee: Idx<Expr>, args: &[Idx<Expr>], call: Idx<Expr>) {
        let Some(&def) = self.resolutions.get(&callee) else {
            return;
        };
        if self.def(def).kind != DefKind::Builtin(BuiltinKind::Prev) {
            return;
        }
        let span = self.ast.exprs[call].span;
        if !self.in_contract {
            self.diagnostics.push(Diagnostic::error(
                ErrorCode::E5017,
                lstr!(en: "prev() can only be used inside contracts";
                      tr: "prev() yalnızca kontratlarda kullanılabilir"),
                LabeledSpan::primary(
                    span,
                    lstr!(en: "this is RTL, not a contract"; tr: "burası kontrat değil, RTL"),
                ),
                lstr!(en: "use a register for a past value in RTL: reg x_r : T = 0; on clk {{ x_r <= x }}";
                      tr: "RTL'de geçmiş değer için reg kullanın: reg x_r : T = 0; on clk {{ x_r <= x }}"),
            ));
            return;
        }
        let depth_ok = match args.get(1) {
            None => true,
            Some(&n) => {
                matches!(self.ast.exprs[n].kind, ExprKind::IntLit { value, .. } if value >= 1)
            }
        };
        if args.is_empty() || args.len() > 2 || !depth_ok {
            self.diagnostics.push(Diagnostic::error(
                ErrorCode::E5017,
                lstr!(en: "prev() takes a signal and an optional positive cycle count";
                      tr: "prev() bir sinyal ve isteğe bağlı pozitif döngü sayısı alır"),
                LabeledSpan::primary(
                    span,
                    lstr!(en: "expected prev(x) or prev(x, N) with N >= 1";
                          tr: "prev(x) ya da N >= 1 ile prev(x, N) bekleniyor"),
                ),
                lstr!(en: "write it as prev(x) or prev(x, 2)";
                      tr: "prev(x) ya da prev(x, 2) biçiminde yazın"),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{codes, resolved};

    #[test]
    fn every_path_expression_gets_a_resolution() {
        let r = resolved("module M { in a : u8 in b : u8 out y : u8 y = a + b }");
        let (a, _) = r.def_by_name("a").expect("a");
        let (b, _) = r.def_by_name("b").expect("b");
        let mut targets: Vec<_> = r.resolutions.values().copied().collect();
        targets.sort();
        assert_eq!(targets, [a, b]);
    }

    #[test]
    fn operands_are_resolved_left_to_right() {
        let r = resolved("module M { out y : u8 y = sol + sag }");
        let spans: Vec<u32> = r
            .diagnostics
            .iter()
            .filter(|d| d.code.as_str() == "E1001")
            .map(|d| d.primary_span().expect("birincil").span.start)
            .collect();
        assert_eq!(spans.len(), 2);
        assert!(spans[0] < spans[1]);
    }

    #[test]
    fn prev_outside_a_contract_is_e5017() {
        let c = codes("module M { in a : u8 out y : u8 y = prev(a) }");
        assert!(c.contains(&"E5017"), "{c:?}");
    }

    #[test]
    fn widened_int_type_is_not_looked_up_as_a_name() {
        let c = codes("module M { in a : u100 out y : u100 y = a }");
        assert!(!c.contains(&"E1001"), "{c:?}");
    }
}
