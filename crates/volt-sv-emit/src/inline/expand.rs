//! Çağrı yerinde açılım (ADR-0081 Karar 12.2, 12.3).
//!
//! **Tel kipi:** çağrı örneği `<fn>_<k>`; fn'in her canlı `let`'i
//! `<fn>_<k>_<let>`, yerine doğrudan yazılamayan argüman
//! `<fn>_<k>_<param>` teli olur; sonuç teli `<fn>_<k>` (çağrı bir modül
//! `let`inin ya da atamasının tüm sağ tarafıysa yazılmaz). Teller modül
//! gövdesine, çağrıyı içeren deyimden hemen önce, bağımlılık sırasıyla
//! `let` deyimi olarak eklenir; emitter onları sıradan modül `let`'leri
//! gibi `wire` bildirir.
//!
//! **İkame kipi:** tel üretilmez; parametreler argüman ifadesiyle,
//! `let`'ler değerleriyle yer değiştirir. Genişlik korunur: parametre
//! tipinden farklı (ya da bilinmeyen) tipli argüman ve bağlama duyarlı
//! sonuç SV boyut dönüşümüyle (`W'(e)`) sarılır — Volt'ta argüman
//! parametre tipine `check` edilir (ADR-0041), SV'de ifadenin genişliği
//! çevresinden gelirdi.

use std::collections::{HashMap, HashSet};

use volt_ast::visit::{expr_children, map_children};
use volt_ast::{
    Expr, ExprKind, FieldInit, Idx, LetDecl, Name, SourceFile, Stmt, StmtKind, TypeRef, MAX_DEPTH,
    MAX_EXPANSION_NODES,
};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use super::scope::{Binding, FnInfo};
use super::types::{canon, Canon, DeclTypes};
use super::{CallHeader, InlineNotes};

/// Açılım kipi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    Wire,
    Subst,
}

/// Çağrının içinde bulunduğu bağlam.
#[derive(Debug, Clone, Default)]
pub(super) struct Ctx {
    /// `comb`, blok içi `for`, kontrat, reg başlangıcı: yalnız ikame.
    pub(super) subst_only: bool,
    /// Blok yereli adlar (`let`, döngü değişkeni, desen bağlaması): bu
    /// adlara başvuran argümanlı çağrı modül düzeyi tel kuramaz.
    pub(super) locals: Vec<String>,
}

impl Ctx {
    pub(super) fn module() -> Self {
        Ctx::default()
    }

    pub(super) fn subst() -> Self {
        Ctx {
            subst_only: true,
            locals: Vec::new(),
        }
    }
}

/// Açılmış gövdenin parametre ve `let` şablonları (çıktı AST'sinde).
struct Env {
    params: Vec<Option<Idx<Expr>>>,
    lets: Vec<Option<Idx<Expr>>>,
}

pub(super) struct Expander<'a> {
    pub(super) src: &'a SourceFile,
    pub(super) out: SourceFile,
    pub(super) fns: &'a HashMap<String, FnInfo<'a>>,
    pub(super) skip: &'a HashSet<String>,
    pub(super) notes: InlineNotes,
    pub(super) diags: Vec<Diagnostic>,
    budget: usize,
    budget_reported: bool,
    // ── modül bağlamı ──
    pub(super) module: String,
    /// Modülde kullanıcının bildirdiği adlar + birim const'ları (E1003).
    pub(super) module_names: HashSet<String>,
    pub(super) decls: DeclTypes,
    counters: HashMap<String, usize>,
    /// Sıradaki deyimden önce eklenecek teller.
    pub(super) pending: Vec<Idx<Stmt>>,
    /// Açılmakta olan fn'ler (döngü savunması — E4013 HIR'da).
    stack: Vec<String>,
    /// Çağrı başına henüz yazılmamış başlık yorumu (iç içe çağrılar).
    headers: Vec<Option<CallHeader>>,
    /// Birimin const adları (hijyen: gövdedeki const, çağıranın aynı adlı
    /// sinyaline bağlanmamalı).
    consts: HashSet<String>,
    /// Üretilen teller (bütçe aşımında geri almak için).
    wire_log: Vec<String>,
}

impl<'a> Expander<'a> {
    pub(super) fn new(
        src: &'a SourceFile,
        fns: &'a HashMap<String, FnInfo<'a>>,
        skip: &'a HashSet<String>,
        consts: HashSet<String>,
    ) -> Self {
        Expander {
            src,
            out: src.clone(),
            fns,
            skip,
            notes: InlineNotes::default(),
            diags: Vec::new(),
            budget: 0,
            budget_reported: false,
            module: String::new(),
            module_names: HashSet::new(),
            decls: DeclTypes::default(),
            counters: HashMap::new(),
            pending: Vec::new(),
            stack: Vec::new(),
            headers: Vec::new(),
            consts,
            wire_log: Vec::new(),
        }
    }

    pub(super) fn enter_module(&mut self, name: &str, names: HashSet<String>, decls: DeclTypes) {
        self.module = name.to_string();
        self.module_names = names;
        self.decls = decls;
        self.counters.clear();
        self.pending.clear();
        self.wire_log.clear();
    }

    // ═══ Çağıran ifadesinin gezilmesi ═════════════════════════════

    /// `e` ağacındaki kullanıcı fn çağrılarını yerinde açar.
    pub(super) fn rewrite(&mut self, e: Idx<Expr>, ctx: &Ctx) {
        let kind = self.out.exprs[e].kind.clone();
        if let ExprKind::Call { callee, args } = &kind {
            if let Some(name) = self.user_fn(*callee, ctx) {
                let mode = if ctx.subst_only || self.mentions_local(args, ctx) {
                    Mode::Subst
                } else {
                    Mode::Wire
                };
                let repl = self.expand_call(e, &name, args, mode, ctx, None, None);
                self.replace(e, repl);
                return;
            }
        }
        for c in expr_children(&kind) {
            self.rewrite(c, ctx);
        }
    }

    /// Çağrılan bir kullanıcı fn'i mi — çağıranın aynı adlı sinyali ya
    /// da blok yereli fn'i gölgeler (W1002).
    pub(super) fn user_fn(&self, callee: Idx<Expr>, ctx: &Ctx) -> Option<String> {
        let ExprKind::Path(p) = &self.out.exprs[callee].kind else {
            return None;
        };
        let [seg] = p.segments.as_slice() else {
            return None;
        };
        let name = &seg.text;
        (self.fns.contains_key(name)
            && !self.module_names.contains(name)
            && !ctx.locals.contains(name))
        .then(|| name.clone())
    }

    fn mentions_local(&self, args: &[Idx<Expr>], ctx: &Ctx) -> bool {
        if ctx.locals.is_empty() {
            return false;
        }
        let mut stack: Vec<Idx<Expr>> = args.to_vec();
        while let Some(e) = stack.pop() {
            if let ExprKind::Path(p) = &self.out.exprs[e].kind {
                if p.segments
                    .first()
                    .is_some_and(|s| ctx.locals.contains(&s.text))
                {
                    return true;
                }
            }
            stack.extend(expr_children(&self.out.exprs[e].kind));
        }
        false
    }

    /// Çağrı düğümünü açılımın kökünün bir kopyasıyla değiştirir (ebeveyn
    /// bağlantıları geçerli kalır).
    pub(super) fn replace(&mut self, e: Idx<Expr>, repl: Idx<Expr>) {
        if e == repl {
            return;
        }
        let node = self.out.exprs[repl].clone();
        self.out.exprs[e] = node;
        if self.notes.sized_casts.contains(&repl) {
            self.notes.sized_casts.insert(e);
        }
        if self.notes.global_paths.contains(&repl) {
            self.notes.global_paths.insert(e);
        }
    }

    // ═══ Çağrının açılması ════════════════════════════════════════

    /// `whole`: çağrı bir modül `let`inin ya da atamasının tüm sağ tarafı
    /// ve hedefin tipi dönüş tipiyle aynı — sonuç teli yazılmaz.
    /// `arg_spans`: başlıkta gösterilecek argüman konumları (iç çağrıda
    /// gövdedeki yazım).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn expand_call(
        &mut self,
        call: Idx<Expr>,
        name: &str,
        args: &[Idx<Expr>],
        mode: Mode,
        ctx: &Ctx,
        whole: Option<bool>,
        arg_spans: Option<Vec<Span>>,
    ) -> Idx<Expr> {
        let call_span = self.out.exprs[call].span;
        let fns = self.fns;
        let info = &fns[name];
        let src = self.src;
        // Geçersiz gövde (tanımda tanılandı), özyineleme (E4013), bütçe
        // (E2027) ya da arity (E2003): açılmaz, tanı zaten var.
        if self.skip.contains(name)
            || self.stack.iter().any(|s| s == name)
            || self.budget_reported
            || args.len() != info.decl.params.len()
        {
            return self.error_node(call_span);
        }
        let Some(tail) = info.tail(src) else {
            return self.error_node(call_span);
        };
        // Bütçe aşılırsa bu çağrının bıraktığı teller geri alınır.
        let pending_mark = self.pending.len();
        let wires_mark = self.wire_log.len();
        let k = match mode {
            Mode::Wire => {
                let c = self.counters.entry(name.to_string()).or_default();
                *c += 1;
                *c - 1
            }
            Mode::Subst => 0,
        };
        // Argümanlar çağıranın bağlamındadır (iç çağrıları önce açılır).
        for &a in args {
            self.rewrite(a, ctx);
        }
        let header = CallHeader {
            fn_name: name.to_string(),
            call: call_span,
            // İç çağrının argümanları gövdede yazıldığı gibi gösterilir.
            args: arg_spans
                .unwrap_or_else(|| args.iter().map(|&a| self.out.exprs[a].span).collect()),
        };
        self.headers.push((mode == Mode::Wire).then_some(header));
        self.stack.push(name.to_string());
        let (live_lets, live_params) = info.liveness(src);
        let mut env = Env {
            params: vec![None; args.len()],
            lets: vec![None; info.lets.len()],
        };
        let mut failed = false;
        for (i, &a) in args.iter().enumerate() {
            if !live_params[i] {
                continue;
            }
            match self.bind_param(info, i, a, mode, k, ctx, call_span) {
                Some(v) => env.params[i] = Some(v),
                None => failed = true,
            }
        }
        if !failed {
            for (j, l) in info.lets.iter().enumerate() {
                if !live_lets[j] {
                    continue;
                }
                let v = self.inst(info, l.value, &env, mode);
                env.lets[j] = Some(match mode {
                    Mode::Wire => {
                        let wire = format!("{name}_{k}_{}", l.name.text);
                        self.add_wire(&wire, l.ty, v, call_span, info)
                    }
                    Mode::Subst => v,
                });
            }
        }
        let result = if failed {
            self.error_node(call_span)
        } else {
            let v = self.inst(info, tail, &env, mode);
            match mode {
                Mode::Wire if whole == Some(true) => v,
                Mode::Wire => {
                    let wire = format!("{name}_{k}");
                    self.add_wire(&wire, info.decl.return_ty, v, call_span, info)
                }
                Mode::Subst => self.subst_result(v, info, call_span),
            }
        };
        self.stack.pop();
        self.headers.pop();
        // Bütçe bu açılım sırasında aşıldıysa kısmi ağaç atılır: yarım
        // açılım ne doğru ne de emit edilmeye değer (tanı zaten var).
        if self.budget_reported {
            self.pending.truncate(pending_mark);
            for name in self.wire_log.drain(wires_mark..) {
                self.notes.headers.remove(&(self.module.clone(), name));
            }
            return self.error_node(call_span);
        }
        result
    }

    /// Parametre şablonu: argümanın kendisi, tipine dönüştürülmüş literal,
    /// tel (tel kipi) ya da boyut dönüşümü (ikame kipi).
    #[allow(clippy::too_many_arguments)]
    fn bind_param(
        &mut self,
        info: &FnInfo<'a>,
        i: usize,
        arg: Idx<Expr>,
        mode: Mode,
        k: usize,
        ctx: &Ctx,
        call_span: Span,
    ) -> Option<Idx<Expr>> {
        let p = &info.decl.params[i];
        let pty = canon(self.src, p.ty);
        let int_like = pty.as_ref().is_some_and(Canon::is_int_like);
        let arg_kind = &self.out.exprs[arg].kind;
        // Dizi elemanı paketlenmiş vektörde `a[W*i +: W]` olur; SV bir
        // parça seçimi yeniden seçemez: gövde bitlerini seçiyorsa doğrudan
        // yazılmaz.
        let simple = is_path_like(&self.out, arg)
            && !(info.param_select_base[i] && matches!(arg_kind, ExprKind::Index { .. }));
        // Dizi parametre argümanın adıyla yer değiştirir (Karar 4; yalın
        // ad olmayan dizi argümanı HIR'da E0003).
        if matches!(pty, Some(Canon::Array(..))) || matches!(arg_kind, ExprKind::BoolLit(_)) {
            return Some(arg);
        }
        // bool/enum/struct/Trit parametresine tip denetleyicisi yalnız aynı
        // tipi geçirir: yol argüman doğrudan yazılır.
        if simple && !int_like {
            return Some(arg);
        }
        if simple {
            let arg_ty = self.decls.type_of(&self.out, arg, &ctx.locals);
            if arg_ty.and_then(|t| canon(&self.out, t)) == pty {
                return Some(arg);
            }
        }
        if int_like && matches!(arg_kind, ExprKind::IntLit { .. }) && !info.param_select_base[i] {
            return Some(self.cast(arg, p.ty, false));
        }
        match mode {
            Mode::Wire => {
                let wire = format!("{}_{k}_{}", info.name, p.name.text);
                Some(self.add_wire(&wire, Some(p.ty), arg, call_span, info))
            }
            Mode::Subst => {
                let is_struct = matches!(&pty, Some(Canon::Named(n)) if self.is_struct(n));
                if (is_struct && !simple) || (int_like && info.param_select_base[i]) {
                    self.err_subst_arg(arg, &p.name.text, &info.name, is_struct);
                    return None;
                }
                if int_like {
                    Some(self.cast(arg, p.ty, true))
                } else {
                    Some(arg)
                }
            }
        }
    }

    /// İkame kipinde sonucun genişliği dönüş tipine sabitlenir: çevresi
    /// geniş bir SV bağlamı `a + b`'yi taşırmadan hesaplardı.
    fn subst_result(&mut self, v: Idx<Expr>, info: &FnInfo<'a>, call_span: Span) -> Idx<Expr> {
        // Yükseklik çağıranın yazdığı çağrıda denetlenir (tanı çağrı
        // yerinde; iç çağrıların ağaçları onun alt ağacıdır).
        if self.stack.len() == 1 && height(&self.out, v) > MAX_DEPTH as usize {
            self.err_too_deep(call_span, &info.name);
            return self.error_node(call_span);
        }
        let Some(ret) = info.decl.return_ty else {
            return v;
        };
        let self_determined = matches!(
            self.out.exprs[v].kind,
            ExprKind::Path(_)
                | ExprKind::Field { .. }
                | ExprKind::IntLit { .. }
                | ExprKind::BoolLit(_)
                | ExprKind::Index { .. }
                | ExprKind::Range { .. }
                | ExprKind::PartSelect { .. }
                | ExprKind::Cast { .. }
        );
        if self_determined || !canon(self.src, ret).is_some_and(|c| c.is_int_like()) {
            return v;
        }
        self.cast(v, ret, true)
    }

    // ═══ Gövdenin örneklenmesi ════════════════════════════════════

    /// fn gövdesindeki `fe` ifadesinin (kaynak AST) açılmış kopyası
    /// (çıktı AST'si).
    fn inst(&mut self, info: &FnInfo<'a>, fe: Idx<Expr>, env: &Env, mode: Mode) -> Idx<Expr> {
        let src = self.src;
        let node = &src.exprs[fe];
        if self.budget_reported {
            return self.error_node(node.span);
        }
        match &node.kind {
            ExprKind::Path(p) if p.segments.len() == 1 => match info.bindings.get(&fe) {
                Some(Binding::Param(i)) => match env.params[*i] {
                    Some(t) => self.clone_tree(t),
                    None => self.error_node(node.span),
                },
                Some(Binding::Let(j)) => match env.lets[*j] {
                    Some(t) => self.clone_tree(t),
                    None => self.error_node(node.span),
                },
                _ => {
                    let n = self.alloc(node.clone());
                    if self.consts.contains(&p.segments[0].text) {
                        self.notes.global_paths.insert(n);
                    }
                    n
                }
            },
            ExprKind::Call { callee, args } if self.body_fn(info, *callee).is_some() => {
                let name = self.body_fn(info, *callee).unwrap_or_default();
                let new_args: Vec<Idx<Expr>> = args
                    .iter()
                    .map(|&a| self.inst(info, a, env, mode))
                    .collect();
                let callee_n = self.alloc(src.exprs[*callee].clone());
                let call_n = self.alloc(Expr {
                    span: node.span,
                    kind: ExprKind::Call {
                        callee: callee_n,
                        args: new_args.clone(),
                    },
                });
                let ctx = Ctx {
                    subst_only: mode == Mode::Subst,
                    locals: Vec::new(),
                };
                let spans = args.iter().map(|&a| src.exprs[a].span).collect();
                self.expand_call(call_n, &name, &new_args, mode, &ctx, None, Some(spans))
            }
            ExprKind::StructLit { path, fields } => {
                let mut out_fields = Vec::with_capacity(fields.len());
                for (i, f) in fields.iter().enumerate() {
                    let value = match f.value {
                        Some(v) => self.inst(info, v, env, mode),
                        // Kısa alan (`P { a }`) açılımda yol olur: çağıranın
                        // aynı adlı sinyaline bağlanmasın.
                        None => match info.shorthand.get(&(fe, i)) {
                            Some(Binding::Param(pi)) => match env.params[*pi] {
                                Some(t) => self.clone_tree(t),
                                None => self.error_node(f.span),
                            },
                            Some(Binding::Let(j)) => match env.lets[*j] {
                                Some(t) => self.clone_tree(t),
                                None => self.error_node(f.span),
                            },
                            _ => self.alloc(Expr {
                                span: f.name.span,
                                kind: ExprKind::Path(volt_ast::Path {
                                    span: f.name.span,
                                    segments: vec![f.name.clone()],
                                }),
                            }),
                        },
                    };
                    out_fields.push(FieldInit {
                        span: f.span,
                        name: f.name.clone(),
                        value: Some(value),
                    });
                }
                self.alloc(Expr {
                    span: node.span,
                    kind: ExprKind::StructLit {
                        path: path.clone(),
                        fields: out_fields,
                    },
                })
            }
            kind => {
                let kind = map_children(kind, |c| self.inst(info, c, env, mode));
                self.alloc(Expr {
                    span: node.span,
                    kind,
                })
            }
        }
    }

    /// Gövdedeki çağrının hedefi bir kullanıcı fn'i mi (fn kapsamında:
    /// parametre ya da `let` gölgelemiyorsa).
    fn body_fn(&self, info: &FnInfo<'a>, callee: Idx<Expr>) -> Option<String> {
        let ExprKind::Path(p) = &self.src.exprs[callee].kind else {
            return None;
        };
        let [seg] = p.segments.as_slice() else {
            return None;
        };
        let global = matches!(info.bindings.get(&callee), None | Some(Binding::Global));
        (global && self.fns.contains_key(&seg.text)).then(|| seg.text.clone())
    }

    /// Çıktı AST'sindeki bir şablonun taze kopyası.
    fn clone_tree(&mut self, e: Idx<Expr>) -> Idx<Expr> {
        if self.budget_reported {
            return self.error_node(self.out.exprs[e].span);
        }
        let node = self.out.exprs[e].clone();
        let kind = map_children(&node.kind, |c| self.clone_tree(c));
        let n = self.alloc(Expr {
            span: node.span,
            kind,
        });
        if self.notes.sized_casts.contains(&e) {
            self.notes.sized_casts.insert(n);
        }
        if self.notes.global_paths.contains(&e) {
            self.notes.global_paths.insert(n);
        }
        n
    }

    // ═══ Düğüm ve tel üretimi ═════════════════════════════════════

    fn alloc(&mut self, e: Expr) -> Idx<Expr> {
        self.charge(e.span);
        self.out.exprs.alloc(e)
    }

    /// Savunma bütçesi (Karar 11): HIR aynı sınırla E2027'yi önceden
    /// verir; emitter doğrudan çağrıldığında da açılım sınırlı kalır.
    fn charge(&mut self, span: Span) {
        self.budget += 1;
        if self.budget > MAX_EXPANSION_NODES && !self.budget_reported {
            self.budget_reported = true;
            let name = self.stack.last().cloned().unwrap_or_default();
            self.diags.push(
                Diagnostic::error(
                    ErrorCode::E2027,
                    lstr!(en: "inlining the call to '{name}' exceeded the AST node budget ({MAX_EXPANSION_NODES} nodes per compilation unit)";
                          tr: "'{name}' çağrısını açmak AST düğüm bütçesini aştı (derleme birimi başına {MAX_EXPANSION_NODES} düğüm)"),
                    LabeledSpan::primary(
                        span,
                        lstr!(en: "expansion budget exceeded here"; tr: "açılım bütçesi burada aşıldı"),
                    ),
                    lstr!(en: "a function that calls another function twice doubles at every level; share the result in a let, or restructure the computation";
                          tr: "başka bir fonksiyonu iki kez çağıran fonksiyon her seviyede ikiye katlanır; sonucu bir let'te paylaşın ya da hesabı yeniden yapılandırın"),
                ),
            );
        }
    }

    fn error_node(&mut self, span: Span) -> Idx<Expr> {
        self.out.exprs.alloc(Expr {
            span,
            kind: ExprKind::Error,
        })
    }

    fn cast(&mut self, e: Idx<Expr>, ty: Idx<TypeRef>, sized: bool) -> Idx<Expr> {
        let span = self.out.exprs[e].span;
        let n = self.alloc(Expr {
            span,
            kind: ExprKind::Cast { expr: e, ty },
        });
        if sized {
            self.notes.sized_casts.insert(n);
        }
        n
    }

    /// Modül düzeyi `let` teli; yol ifadesini döndürür.
    fn add_wire(
        &mut self,
        name: &str,
        ty: Option<Idx<TypeRef>>,
        value: Idx<Expr>,
        call_span: Span,
        info: &FnInfo<'a>,
    ) -> Idx<Expr> {
        if self.module_names.contains(name) {
            self.err_name_clash(name, call_span, &info.name);
        }
        self.module_names.insert(name.to_string());
        self.wire_log.push(name.to_string());
        if let Some(t) = ty {
            self.decls.types.insert(name.to_string(), t);
        }
        let ident = Name {
            text: name.to_string(),
            span: call_span,
        };
        let stmt = self.out.stmts.alloc(Stmt {
            span: call_span,
            attrs: Vec::new(),
            kind: StmtKind::Let(LetDecl {
                name: ident.clone(),
                ty,
                value,
            }),
        });
        self.charge(call_span);
        self.pending.push(stmt);
        if let Some(h) = self.headers.last_mut().and_then(Option::take) {
            self.notes
                .headers
                .insert((self.module.clone(), name.to_string()), h);
        }
        self.alloc(Expr {
            span: call_span,
            kind: ExprKind::Path(volt_ast::Path {
                span: call_span,
                segments: vec![ident],
            }),
        })
    }

    fn is_struct(&self, name: &str) -> bool {
        self.src.items.iter().any(|&i| {
            matches!(&self.src.items_arena[i].kind,
                volt_ast::ItemKind::Struct(s) if !s.is_port && s.name.text == name)
        })
    }

    // ═══ Tanılar ══════════════════════════════════════════════════

    /// E1003 — üretilen tel adı birimdeki bir adla çakışıyor.
    fn err_name_clash(&mut self, name: &str, call: Span, fn_name: &str) {
        self.diags.push(
            Diagnostic::error(
                ErrorCode::E1003,
                lstr!(en: "inlining '{fn_name}' generates the signal '{name}', which is already declared in module '{}'", self.module;
                      tr: "'{fn_name}' açılımı '{name}' sinyalini üretiyor; bu ad '{}' modülünde zaten bildirilmiş", self.module),
                LabeledSpan::primary(
                    call,
                    lstr!(en: "this call generates '{name}'"; tr: "bu çağrı '{name}' üretiyor"),
                ),
                lstr!(en: "rename the signal '{name}' in the module; generated names follow <fn>_<k>[_<let>|_<param>]";
                      tr: "modüldeki '{name}' sinyalini yeniden adlandırın; üretilen adlar <fn>_<k>[_<let>|_<param>] kalıbındadır"),
            )
            .with_note(
                NoteKind::Note,
                lstr!(en: "each call of a function is inlined with its own wires in the calling module (ADR-0081)";
                      tr: "fonksiyonun her çağrısı çağıran modülde kendi telleriyle açılır (ADR-0081)"),
            ),
        );
    }

    /// E0003 — ikame kipinde tel kurulamayan argüman.
    fn err_subst_arg(&mut self, arg: Idx<Expr>, param: &str, fn_name: &str, is_struct: bool) {
        let span = self.out.exprs[arg].span;
        let what = if is_struct {
            lstr!(en: "a struct argument that is not a name for '{param}' of '{fn_name}' in a comb block, a block-level for or a contract";
                  tr: "comb bloğunda, blok içi for'da ya da kontratta '{fn_name}' fonksiyonunun '{param}' parametresine ad olmayan struct argümanı")
        } else {
            lstr!(en: "an argument for '{param}' of '{fn_name}' that is not a signal name of the parameter's type, while the function selects bits of '{param}', in a comb block, a block-level for or a contract";
                  tr: "comb bloğunda, blok içi for'da ya da kontratta '{fn_name}' fonksiyonunun bitlerini seçtiği '{param}' parametresine, parametre tipinde bir sinyal adı olmayan argüman")
        };
        self.diags.push(Diagnostic::error(
            ErrorCode::E0003,
            lstr!(en: "not supported yet: {what}"; tr: "henüz desteklenmiyor: {what}"),
            LabeledSpan::primary(
                span,
                lstr!(en: "no wire can be created here"; tr: "burada tel kurulamaz"),
            ),
            lstr!(en: "bind the argument to a module-level let of the parameter's type and pass its name (ADR-0081)";
                  tr: "argümanı parametrenin tipinde modül düzeyi bir let'e bağlayıp adını geçirin (ADR-0081)"),
        ));
    }

    /// E0018 — ikame kipinde açılmış ifade çok derin.
    fn err_too_deep(&mut self, call: Span, fn_name: &str) {
        self.diags.push(Diagnostic::error(
            ErrorCode::E0018,
            lstr!(en: "inlining '{fn_name}' here produces an expression nested deeper than {MAX_DEPTH} levels";
                  tr: "'{fn_name}' burada açılınca {MAX_DEPTH} seviyeden derin bir ifade oluşuyor"),
            LabeledSpan::primary(
                call,
                lstr!(en: "expanded expression too deep"; tr: "açılmış ifade çok derin"),
            ),
            lstr!(en: "call the function from a module-level let (wires keep each call shallow), or split the nested calls (ADR-0080, ADR-0081)";
                  tr: "fonksiyonu modül düzeyi bir let'ten çağırın (teller her çağrıyı sığ tutar) ya da iç içe çağrıları bölün (ADR-0080, ADR-0081)"),
        ));
    }
}

/// Yol, alan zinciri ya da dizi elemanı (`x`, `s.a`, `u.q`, `arr[i]`):
/// genişliği kendi tipi olan, SV'de seçilebilen ifade.
fn is_path_like(ast: &SourceFile, e: Idx<Expr>) -> bool {
    match &ast.exprs[e].kind {
        ExprKind::Path(_) => true,
        ExprKind::Field { base, .. } | ExprKind::Index { base, .. } => is_path_like(ast, *base),
        _ => false,
    }
}

/// Ağaç yüksekliği (yinelemeli).
fn height(ast: &SourceFile, root: Idx<Expr>) -> usize {
    let mut best = 0;
    let mut stack = vec![(root, 1usize)];
    while let Some((e, d)) = stack.pop() {
        best = best.max(d);
        if d > MAX_DEPTH as usize {
            break;
        }
        for c in expr_children(&ast.exprs[e].kind) {
            stack.push((c, d + 1));
        }
    }
    best
}
