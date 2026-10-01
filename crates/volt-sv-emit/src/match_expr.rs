//! `match` ifadesi ve blok içi `let` (ADR-0083).
//!
//! Karar 10 — yere göre biçim: match ifadesi bir atamanın ya da `let`'in
//! **tüm** sağ tarafıysa (kök konum) deyim biçimine (ADR-0032) iner, hedef
//! her kolda tekrarlanır (`case (s) k: y = v; … endcase`); başka her
//! konumda (operand, koşul, indeks, port bağlaması, kontrat, fn ikame
//! kipi) `if` ifadesinin genellemesi olan üçlü zincirdir
//! (`(s == k0) ? v0 : (s == k1) ? v1 : v_son`). İki biçim aynı donanımdır
//! (§5.4); geçersiz kodları son kol alır (Karar 4: `default` ↔ zincirin
//! son `else`'i).
//!
//! Karar 11 — blok `let`'i içinde bulunduğu `always_ff`/`always_comb`
//! sürecinin yerel değişkenidir: süreç başında ayrı bildirim (ilk değer
//! yok), bildirim noktasında blocking atama; yerel içeren süreç adlı
//! bloktur (`begin : on_<k>` / `comb_<k>`). Modül düzeyi bir SV adıyla ya
//! da aynı süreçteki başka bir yerelle çakışan ad `<ad>_2`, `<ad>_3`…
//! olur (SV adla, Volt `DefId` ile çözer — §5.6 ad yakalaması).

use volt_ast::{EnumDecl, Expr, ExprKind, Idx, LetDecl, MatchArm, MatchArmBody, PatternKind};
use volt_ast::{MAX_DEPTH, MAX_EXPANSION_NODES};
use volt_diagnostics::{lstr, ErrorCode};
use volt_span::Span;

use crate::expr::{PREC_TERNARY, PREC_UNARY};
use crate::{Emitter, Sig};

/// SV `==` önceliği (`expr::sv_prec`): sınananın sol operand konumu.
const PREC_EQ: u8 = 6;

/// Kolun SV karşılığı: `labels` `None` ise `default` (joker ya da
/// Karar 4'ün son adlı kolu).
struct ExprArm {
    labels: Option<Vec<String>>,
    body: Idx<Expr>,
}

struct ExprMatchPlan {
    scrut: String,
    arms: Vec<ExprArm>,
    /// Son adlı kol `default` olduysa yorum: `State_Done (and invalid codes)`.
    default_comment: Option<String>,
}

/// Blok `let`'inin süreç içi yereli.
pub(crate) struct ProcLocal {
    /// Bildirimin ad span'i ve adı (aynı `let` `for` iterasyonlarında aynı
    /// yerel; struct indirgemesinin yaprakları aynı span'i paylaşır).
    key: (Span, String),
    pub(crate) sv: String,
    pub(crate) sig: Sig,
    /// Tipsiz yerelin esnek aralık alt ucu (ADR-0025); somut tipte
    /// `sig.width`.
    pub(crate) lo: u32,
    /// Okunan en geniş bit sayısı (ADR-0025): `lo < sig.width` iken
    /// taşma biti hiç okunmazsa bildirim UNUSEDSIGNAL'dan susturulur.
    pub(crate) read: u32,
    pub(crate) enum_name: Option<String>,
}

/// Açık sürecin yerel durumu: bildirimler ve kapsam yığını.
#[derive(Default)]
pub(crate) struct ProcScope {
    pub(crate) locals: Vec<ProcLocal>,
    /// İçten dışa: (Volt adı, `locals` indeksi).
    scopes: Vec<Vec<(String, usize)>>,
}

impl<'a> Emitter<'a> {
    // ═══ Kök konum ════════════════════════════════════════════════

    /// `e` bir match ifadesiyse (sınanan, kollar).
    pub(crate) fn as_match(&self, e: Idx<Expr>) -> Option<(Idx<Expr>, &'a [MatchArm])> {
        match &self.ast.exprs[e].kind {
            ExprKind::Match { scrutinee, arms } => Some((*scrutinee, arms.as_slice())),
            _ => None,
        }
    }

    /// Kök konum (Karar 10.1): `case (s) k: <hedef> <op> v; … endcase`.
    /// `None`: tanı üretildi, satır yazılmaz.
    pub(crate) fn emit_match_case(
        &mut self,
        target: &str,
        op: &str,
        sig: Option<Sig>,
        match_expr: Idx<Expr>,
        indent: usize,
    ) -> Vec<String> {
        let (scrutinee, arms) = self.as_match(match_expr).expect("çağıran match denetler");
        let ind = " ".repeat(indent);
        let Some(plan) = self.plan_match_expr(scrutinee, arms) else {
            return Vec::new();
        };
        let mut lines = vec![format!("{ind}case ({})", plan.scrut)];
        for arm in &plan.arms {
            let value = self.emit_assigned(arm.body, sig);
            match &arm.labels {
                Some(labels) => {
                    lines.push(format!(
                        "{ind}    {}: {target} {op} {value};",
                        labels.join(", ")
                    ));
                }
                None => {
                    let comment = plan
                        .default_comment
                        .as_ref()
                        .map_or(String::new(), |c| format!(" // {c}"));
                    lines.push(format!("{ind}    default: {target} {op} {value};{comment}"));
                }
            }
        }
        lines.push(format!("{ind}endcase"));
        lines
    }

    // ═══ İç konum ═════════════════════════════════════════════════

    /// İç konum (Karar 10.2): üçlü zincir. Yükseklik (E0018) ve sınanan
    /// kopyası (E2027) yalnız en dıştaki iç match'te denetlenir.
    pub(crate) fn emit_match_ternary(&mut self, idx: Idx<Expr>, ctx: Option<Sig>) -> (String, u8) {
        let (scrutinee, arms) = self.as_match(idx).expect("çağıran match denetler");
        if self.ternary_depth == 0 && !self.check_ternary_limits(idx) {
            return ("1'b0".to_string(), crate::expr::PREC_ATOM);
        }
        let Some(plan) = self.plan_match_expr(scrutinee, arms) else {
            return ("1'b0".to_string(), crate::expr::PREC_ATOM);
        };
        self.ternary_depth += 1;
        let mut parts = Vec::with_capacity(plan.arms.len());
        let mut last = String::new();
        for arm in &plan.arms {
            let value = self.emit_operand(arm.body, ctx, PREC_UNARY, false);
            match &arm.labels {
                Some(labels) => {
                    let cond = labels
                        .iter()
                        .map(|l| format!("{} == {l}", plan.scrut))
                        .collect::<Vec<_>>()
                        .join(" || ");
                    parts.push(format!("({cond}) ? {value}"));
                }
                None => last = value,
            }
        }
        self.ternary_depth -= 1;
        if parts.is_empty() {
            return (last, PREC_UNARY);
        }
        parts.push(last);
        (parts.join(" : "), PREC_TERNARY)
    }

    /// E0018 / E2027 (Karar 10.3–10.4). `false`: tanı üretildi.
    fn check_ternary_limits(&mut self, idx: Idx<Expr>) -> bool {
        let span = self.ast.exprs[idx].span;
        let height = self.ternary_height(idx, 0);
        if height > MAX_DEPTH as usize {
            self.error(
                ErrorCode::E0018,
                lstr!(
                    en: "this 'match' expression becomes a conditional chain nested deeper than {MAX_DEPTH} levels";
                    tr: "bu 'match' ifadesi {MAX_DEPTH} seviyeden derin bir koşullu zincire iniyor"
                ),
                span,
                &lstr!(
                    en: "give the match its own let (let v = match ...): as the whole right-hand side it becomes a 'case' of any size (ADR-0083)";
                    tr: "match'i kendi let'ine verin (let v = match ...): tüm sağ taraf olarak her boyutta bir 'case' olur (ADR-0083)"
                ),
            );
            return false;
        }
        if self.ternary_cost(idx, 0) > MAX_EXPANSION_NODES {
            self.error(
                ErrorCode::E2027,
                lstr!(
                    en: "this 'match' expression repeats its scrutinee beyond the AST node budget ({MAX_EXPANSION_NODES} nodes)";
                    tr: "bu 'match' ifadesi sınananını AST düğüm bütçesinin ({MAX_EXPANSION_NODES} düğüm) ötesinde tekrarlıyor"
                ),
                span,
                &lstr!(
                    en: "bind the scrutinee to a let first, or give the match its own let so it becomes a 'case' (ADR-0083)";
                    tr: "sınananı önce bir let'e bağlayın ya da match'i kendi let'ine verin ki 'case' olsun (ADR-0083)"
                ),
            );
            return false;
        }
        true
    }

    /// Üretilecek SV ifadesinin yüksekliği: iç match kol sayısı kadar
    /// derin bir zincirdir.
    fn ternary_height(&self, e: Idx<Expr>, depth: usize) -> usize {
        if depth > MAX_DEPTH as usize {
            return depth;
        }
        let kind = &self.ast.exprs[e].kind;
        let children = volt_ast::visit::expr_children(kind);
        let below = children
            .iter()
            .map(|&c| self.ternary_height(c, depth + 1))
            .max()
            .unwrap_or(depth);
        match kind {
            ExprKind::Match { arms, .. } => below + arms.len().saturating_sub(1),
            _ => below.max(depth + 1),
        }
    }

    /// Üretilecek SV ifadesinin düğüm sayısı: sınanan her desen
    /// literali için bir kez yazılır.
    fn ternary_cost(&self, e: Idx<Expr>, depth: usize) -> usize {
        if depth > MAX_DEPTH as usize {
            return MAX_EXPANSION_NODES + 1;
        }
        let kind = &self.ast.exprs[e].kind;
        match kind {
            ExprKind::Match { scrutinee, arms } => {
                let s = self.ternary_cost(*scrutinee, depth + 1);
                let mut total: usize = 1;
                for arm in arms {
                    let lits = self.pattern_literals(arm.pattern);
                    total = total.saturating_add(s.saturating_mul(lits.max(1)));
                    for c in arm.guard.into_iter().chain(match arm.body {
                        MatchArmBody::Expr(b) => Some(b),
                        MatchArmBody::Block(_) => None,
                    }) {
                        total = total.saturating_add(self.ternary_cost(c, depth + 1));
                    }
                }
                total
            }
            _ => volt_ast::visit::expr_children(kind)
                .iter()
                .fold(1usize, |acc, &c| {
                    acc.saturating_add(self.ternary_cost(c, depth + 1))
                }),
        }
    }

    fn pattern_literals(&self, pat: Idx<volt_ast::Pattern>) -> usize {
        match &self.ast.patterns[pat].kind {
            PatternKind::Or(alts) => alts.iter().map(|&a| self.pattern_literals(a)).sum(),
            PatternKind::Wildcard => 0,
            _ => 1,
        }
    }

    // ═══ Ortak kol planı ══════════════════════════════════════════

    /// Deyim `match`'iyle aynı kurallar: erişilemez kol atlanır (ADR-0075),
    /// kapsayıcı `_`'sız enum match'inde son adlı kol `default` olur
    /// (ADR-0074 Karar 4); ilk joker koldan sonrası erişilemez.
    fn plan_match_expr(
        &mut self,
        scrutinee: Idx<Expr>,
        arms: &'a [MatchArm],
    ) -> Option<ExprMatchPlan> {
        let scrut_sig = self.width_of(scrutinee);
        let scrut_enum: Option<&'a EnumDecl> = self.enum_of_expr(scrutinee);
        let scrut = self.emit_prec(scrutinee, scrut_sig, PREC_EQ, false);
        let enum_plan = scrut_enum.map(|d| self.enum_match_plan(arms, d));
        let value_skip = match enum_plan {
            Some(_) => Vec::new(),
            None => volt_ast::match_cover::unreachable_value_arms(self.ast, arms, &mut |e| {
                self.eval_const(e)
            }),
        };
        let mut out = Vec::with_capacity(arms.len());
        let mut default_comment = None;
        let mut ok = true;
        for (i, arm) in arms.iter().enumerate() {
            if enum_plan.as_ref().is_some_and(|p| p.skip[i]) || value_skip.get(i) == Some(&true) {
                continue;
            }
            let MatchArmBody::Expr(body) = arm.body else {
                continue; // gramer ifade kolunda blok gövdesi vermez (E0001)
            };
            if arm.guard.is_some() {
                self.future(
                    arm.span,
                    &lstr!(
                        en: "'match' arm guards ('if' after a pattern)";
                        tr: "'match' kolu muhafızları (desenden sonra 'if')"
                    ),
                );
                ok = false;
                continue;
            }
            if let Some(p) = enum_plan.as_ref().filter(|p| p.default_arm == Some(i)) {
                let invalid = if p.has_invalid_codes {
                    " (and invalid codes)"
                } else {
                    ""
                };
                default_comment = Some(format!("{}{invalid}", p.default_names));
                out.push(ExprArm { labels: None, body });
                break;
            }
            match self.expr_arm_labels(arm.pattern, scrut_sig, scrut_enum) {
                Some(Some(labels)) => out.push(ExprArm {
                    labels: Some(labels),
                    body,
                }),
                Some(None) => {
                    out.push(ExprArm { labels: None, body });
                    break;
                }
                None => ok = false,
            }
        }
        if !ok {
            return None;
        }
        // Kapsayıcılık tip denetiminde (E0014); yine de zincirin bir sonu
        // olmalı — son kol varsayılan olur.
        if let Some(last) = out.last_mut().filter(|a| a.labels.is_some()) {
            last.labels = None;
        }
        Some(ExprMatchPlan {
            scrut,
            arms: out,
            default_comment,
        })
    }

    /// Kolun etiketleri; `Some(None)` joker (varsayılan), `None` tanı
    /// üretildi.
    fn expr_arm_labels(
        &mut self,
        pat: Idx<volt_ast::Pattern>,
        scrut_sig: Option<Sig>,
        scrut_enum: Option<&'a EnumDecl>,
    ) -> Option<Option<Vec<String>>> {
        let ast = self.ast;
        match &ast.patterns[pat].kind {
            PatternKind::Wildcard => Some(None),
            PatternKind::Or(alts) => {
                if alts
                    .iter()
                    .any(|&a| matches!(ast.patterns[a].kind, PatternKind::Wildcard))
                {
                    return Some(None);
                }
                let mut labels = Vec::with_capacity(alts.len());
                for &a in alts {
                    labels.extend(self.expr_arm_labels(a, scrut_sig, scrut_enum)??);
                }
                Some(Some(labels))
            }
            _ => self
                .match_arm_label(pat, scrut_sig, scrut_enum)
                .map(|l| Some(vec![l])),
        }
    }

    // ═══ Blok `let`'i ═════════════════════════════════════════════

    /// Süreç başında boş kapsam; `emit_block` her blok için bir kat açar.
    pub(crate) fn begin_process(&mut self) -> ProcScope {
        std::mem::take(&mut self.proc)
    }

    /// Süreci kapat: yerel bildirim satırları ve adlı blok etiketi
    /// (`None`: yerel yok, blok adsız kalır — çıktı bugünküyle aynı).
    pub(crate) fn end_process(
        &mut self,
        outer: ProcScope,
        kind: &str,
        index: usize,
        indent: usize,
    ) -> Option<(String, Vec<String>)> {
        let done = std::mem::replace(&mut self.proc, outer);
        if done.locals.is_empty() {
            return None;
        }
        let ind = " ".repeat(indent);
        let mut decls: Vec<String> = done
            .locals
            .iter()
            .map(|l| {
                let line = format!("{ind}{} {};", l.sig.decl_type(), l.sv);
                if l.lo < l.sig.width && l.read < l.sig.width {
                    crate::let_width::silence_unused_carry(&ind, &l.sv, &line)
                } else {
                    line
                }
            })
            .collect();
        // `always_comb`'da dal içi yerel her yolda atanmazsa Yosys bunu
        // mandal sayar (ERROR) ve Verilator LATCH uyarır; süreç başındaki
        // sıfır hiç okunmaz — `let` yalnız bildiriminden sonra görünür
        // (ADR-0083 Aşama 2 ölçümü). `always_ff`'te gerekmez.
        if kind == "comb" {
            decls.extend(
                done.locals
                    .iter()
                    .map(|l| format!("{ind}{} = {}'d0;", l.sv, l.sig.width)),
            );
        }
        let mut label = format!("{kind}_{index}");
        let taken: Vec<String> = done.locals.iter().map(|l| l.sv.clone()).collect();
        let mut n = 2;
        while self.module_name_taken(&label) || taken.contains(&label) {
            label = format!("{kind}_{index}_{n}");
            n += 1;
        }
        Some((label, decls))
    }

    pub(crate) fn push_local_scope(&mut self) {
        self.proc.scopes.push(Vec::new());
    }

    pub(crate) fn pop_local_scope(&mut self) {
        self.proc.scopes.pop();
    }

    /// Volt adının bu noktada görünen süreç yereli (içten dışa).
    pub(crate) fn local(&self, name: &str) -> Option<&ProcLocal> {
        self.local_index(name).map(|i| &self.proc.locals[i])
    }

    pub(crate) fn local_index(&self, name: &str) -> Option<usize> {
        self.proc
            .scopes
            .iter()
            .rev()
            .flat_map(|s| s.iter().rev())
            .find(|(n, _)| n == name)
            .map(|&(_, i)| i)
    }

    /// `let t = v` blok satırları: değer, ad kapsama girmeden ÖNCE
    /// üretilir (`let a = a + 1` dıştaki `a`'yı okur).
    pub(crate) fn emit_block_let(&mut self, decl: &'a LetDecl, indent: usize) -> Vec<String> {
        let ind = " ".repeat(indent);
        let span = decl.name.span;
        if self.proc.scopes.is_empty() {
            // Süreç dışı blok (ör. modül `for` gövdesi parser'da açıldı);
            // savunma: yerel kurulamaz.
            self.future(
                span,
                &lstr!(
                    en: "'let {}' outside an 'on' or 'comb' block", decl.name.text;
                    tr: "'on' ya da 'comb' bloğu dışında 'let {}'", decl.name.text
                ),
            );
            return Vec::new();
        }
        // Tipsiz yerel: doğal genişlik (type-inference.md §3.3, ADR-0025).
        let sig = match decl.ty {
            Some(t) => self.sig_of_typeref(t, span).map(|s| (s, s.width)),
            None => self.untyped_let_sig(decl.value),
        };
        let Some((sig, lo)) = sig else {
            if decl.ty.is_none() {
                self.error(
                    ErrorCode::E2005,
                    lstr!(
                        en: "cannot determine the width of '{}'", decl.name.text;
                        tr: "'{}' genişliği belirlenemiyor", decl.name.text
                    ),
                    span,
                    &lstr!(
                        en: "write an explicit type on the let binding: let x : u8 = ...";
                        tr: "let bağlamasına açık tip yazın: let x : u8 = ..."
                    ),
                );
            }
            return Vec::new();
        };
        let enum_name = match decl.ty {
            Some(t) => self.enum_of_type(t).map(|d| d.name.text.clone()),
            None => self.enum_of_expr(decl.value).map(|d| d.name.text.clone()),
        };
        let key = (span, decl.name.text.clone());
        let index = match self.proc.locals.iter().position(|l| l.key == key) {
            Some(i) => i,
            None => {
                let sv = self.fresh_local_name(&decl.name.text);
                self.proc.locals.push(ProcLocal {
                    key,
                    sv,
                    sig,
                    lo,
                    read: 0,
                    enum_name,
                });
                self.proc.locals.len() - 1
            }
        };
        let sv = self.proc.locals[index].sv.clone();
        let lines = if self.as_match(decl.value).is_some() {
            self.emit_match_case(&sv, "=", Some(sig), decl.value, indent)
        } else {
            let value = self.emit_assigned(decl.value, Some(sig));
            vec![format!("{ind}{sv} = {value};")]
        };
        if let Some(top) = self.proc.scopes.last_mut() {
            top.push((decl.name.text.clone(), index));
        }
        lines
    }

    /// `name` ya da `name_2`, `name_3`…: modül düzeyi SV adlarıyla ve
    /// süreçteki yerellerle çakışmayan ilk ad.
    fn fresh_local_name(&self, name: &str) -> String {
        let clash =
            |n: &str| self.module_name_taken(n) || self.proc.locals.iter().any(|l| l.sv == n);
        if !clash(name) {
            return name.to_string();
        }
        let mut k = 2;
        loop {
            let candidate = format!("{name}_{k}");
            if !clash(&candidate) {
                return candidate;
            }
            k += 1;
        }
    }

    /// Modül düzeyinde bu SV adı var mı: port, reg, tel, modül `let`'i,
    /// fn açılım telleri (sembol tablosu), örnek adı ve çıkış telleri
    /// (`<örnek>_<port>`), enum `localparam`'ı (`<Enum>_<Varyant>`).
    pub(crate) fn module_name_taken(&self, name: &str) -> bool {
        if self.symbols.contains_key(name)
            || self.builtin_insts.contains_key(name)
            || self.user_insts.contains_key(name)
        {
            return true;
        }
        // Örnek çıkış telleri `<örnek>_<port>`.
        let user_output = self.user_insts.iter().any(|(i, u)| {
            name.strip_prefix(i.as_str())
                .and_then(|r| r.strip_prefix('_'))
                .is_some_and(|port| u.outputs.iter().any(|(o, _)| o == port))
        });
        let builtin_port = self.builtin_insts.iter().any(|(i, b)| {
            name.strip_prefix(i.as_str())
                .and_then(|r| r.strip_prefix('_'))
                .is_some_and(|port| b.prim.port(port).is_some())
        });
        if user_output || builtin_port {
            return true;
        }
        self.ast
            .items
            .iter()
            .any(|&i| match &self.ast.items_arena[i].kind {
                volt_ast::ItemKind::Enum(e) => e
                    .variants
                    .iter()
                    .any(|v| format!("{}_{}", e.name.text, v.name.text) == name),
                _ => false,
            })
    }
}
