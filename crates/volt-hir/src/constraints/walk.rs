//! Modül yürüyüşü (ADR-0054): saat portları ve alan frekansları,
//! asenkron gruplar, üretilen CDC köprüleri (sv-emit'in adlandırmasıyla
//! bire bir) ve kullanıcı nitelikleri. Alt modül örnekleri `örnek/`
//! önekiyle düzleştirilir; alt modülün saat portları üst modülün saat
//! adlarına bağlama üzerinden eşlenir.

use std::collections::{HashMap, HashSet};

use volt_ast::builtin::BuiltinPrim;
use volt_ast::{
    Attribute, Block, BlockStmt, ClockEdge, DomainKey, DomainValue, ElseBranch, ExprKind, Idx,
    InstanceDecl, ItemKind, MatchArmBody, ModuleDecl, Port, PortDir, ResetSync, SourceFile, Stmt,
    StmtKind, TypeRefKind,
};
use volt_diagnostics::{lstr, Diagnostic, LabeledSpan, NoteKind};
use volt_span::{FileId, Span};

use super::parse::{
    parse_false_path, parse_multicycle, parse_timing, unsupported, PathKind, TimingForm,
};
use super::{
    Bridge, ClockConstraint, ConstraintResult, Crossing, CrossingClass, FreqSource,
    ModuleConstraints, PathRule, ResetChain, Target, RESET_SYNC_STAGES,
};

/// Yerleşik CDC bileşeninin bir geçişi: kaynak ve hedef register,
/// sınıf (ADR-0065 §4.2) ve yön (`true`: hedef saatten kaynak saate —
/// AsyncFifo okuma işaretçisi, HandshakeSync ack).
type PrimCrossing = (&'static str, &'static str, CrossingClass, bool);

/// Yerleşik CDC bileşeninin geçiş tablosu: (tür adı, geçişler,
/// senkronizatör register'ları, kaynak saat portu, hedef saat portu).
type PrimSpec = (
    &'static str,
    &'static [PrimCrossing],
    &'static [&'static str],
    &'static str,
    &'static str,
);

/// Hiyerarşi derinliği sınırı — özyineleme koruması (örnek döngüsü
/// E-kodu başka geçitte; burada yalnız sonlanma garantisi).
const MAX_DEPTH: usize = 32;

const ATTR_TIMING: &str = "timing";
const ATTR_FALSE_PATH: &str = "false_path";
const ATTR_MULTICYCLE: &str = "multicycle";

/// Domain bildiriminden okunan frekans (`None`: anahtar yok) ve reset
/// varlığı (`reset = none` → `false`).
struct DomainDecl {
    freq_hz: Option<u64>,
    has_reset: bool,
    span: Span,
}

/// Bir modülün ad tabloları — çözümlemeden bağımsız, gövdeden toplanır.
struct Scope<'a> {
    ast: &'a SourceFile,
    module: &'a ModuleDecl,
    /// port adı → (yön, saat mi, `@Domain`)
    ports: HashMap<&'a str, (PortDir, bool, Option<&'a str>)>,
    regs: HashSet<&'a str>,
    nets: HashSet<&'a str>,
    /// `let ad = ifade` — kaynak saati ifadedeki sinyallerden izlenir.
    let_exprs: HashMap<&'a str, Idx<volt_ast::Expr>>,
    /// register → onu süren `on` bloğunun saat portu
    reg_clock: HashMap<&'a str, &'a str>,
}

impl<'a> Scope<'a> {
    fn new(ast: &'a SourceFile, module: &'a ModuleDecl) -> Self {
        let mut scope = Scope {
            ast,
            module,
            ports: HashMap::new(),
            regs: HashSet::new(),
            nets: HashSet::new(),
            let_exprs: HashMap::new(),
            reg_clock: HashMap::new(),
        };
        for p in &module.ports {
            let is_clock = matches!(ast.types[p.ty].kind, TypeRefKind::Clock);
            scope.ports.insert(
                p.name.text.as_str(),
                (
                    p.direction,
                    is_clock,
                    p.domain.as_ref().map(|d| d.text.as_str()),
                ),
            );
        }
        for &stmt in &module.body {
            match &ast.stmts[stmt].kind {
                StmtKind::Reg(r) => {
                    scope.regs.insert(r.name.text.as_str());
                }
                StmtKind::Wire(w) => {
                    scope.nets.insert(w.name.text.as_str());
                }
                StmtKind::Let(l) => {
                    scope.nets.insert(l.name.text.as_str());
                    scope.let_exprs.insert(l.name.text.as_str(), l.value);
                }
                StmtKind::On(on) => {
                    if let volt_ast::OnTrigger::Clock(clk) = &on.trigger {
                        let mut targets = Vec::new();
                        collect_nonblocking_targets(ast, on.body, &mut targets);
                        for t in targets {
                            scope.reg_clock.entry(t).or_insert(clk.text.as_str());
                        }
                    }
                }
                _ => {}
            }
        }
        scope
    }

    /// Saat portları kaynak sırasıyla.
    fn clock_ports(&self) -> impl Iterator<Item = &'a volt_ast::Port> + '_ {
        self.module
            .ports
            .iter()
            .filter(move |p| self.ports.get(p.name.text.as_str()).is_some_and(|e| e.1))
    }

    fn is_clock_port(&self, name: &str) -> bool {
        self.ports.get(name).is_some_and(|e| e.1)
    }

    /// `@Domain` anotasyonlu bir sinyalin alanının saat portu.
    fn clock_of_domain(&self, domain: &str) -> Option<&'a str> {
        self.clock_ports()
            .find(|p| p.domain.as_ref().is_some_and(|d| d.text == domain))
            .map(|p| p.name.text.as_str())
    }

    /// Bir sinyalin sürücü saati: port ise alanının saati, register ise
    /// `on` bloğunun saati, `let` ise ifadesindeki ilk saatli sinyalin
    /// saati (sığ izleme, derinlik sınırlı).
    fn driver_clock(&self, name: &str) -> Option<&'a str> {
        self.driver_clock_depth(name, 0)
    }

    fn driver_clock_depth(&self, name: &str, depth: usize) -> Option<&'a str> {
        if let Some((_, _, Some(domain))) = self.ports.get(name) {
            return self.clock_of_domain(domain);
        }
        if let Some(clk) = self.reg_clock.get(name) {
            return Some(clk);
        }
        if depth >= 8 {
            return None;
        }
        let expr = *self.let_exprs.get(name)?;
        let mut names = Vec::new();
        collect_path_names(self.ast, expr, &mut names);
        names
            .into_iter()
            .find_map(|n| self.driver_clock_depth(n, depth + 1))
    }
}

/// İfadedeki tek segmentli yol adları (sığ: alt ifadeler dâhil, çağrı
/// adları hariç).
fn collect_path_names<'a>(ast: &'a SourceFile, e: Idx<volt_ast::Expr>, out: &mut Vec<&'a str>) {
    match &ast.exprs[e].kind {
        ExprKind::Path(p) if p.segments.len() == 1 => out.push(p.segments[0].text.as_str()),
        ExprKind::Binary { lhs, rhs, .. } => {
            collect_path_names(ast, *lhs, out);
            collect_path_names(ast, *rhs, out);
        }
        ExprKind::Unary { operand, .. } => collect_path_names(ast, *operand, out),
        ExprKind::Index { base, index } => {
            collect_path_names(ast, *base, out);
            collect_path_names(ast, *index, out);
        }
        ExprKind::Range { base, .. }
        | ExprKind::PartSelect { base, .. }
        | ExprKind::Field { base, .. } => collect_path_names(ast, *base, out),
        ExprKind::Cast { expr, .. } => collect_path_names(ast, *expr, out),
        ExprKind::Call { args, .. } => {
            for a in args {
                collect_path_names(ast, *a, out);
            }
        }
        ExprKind::If {
            cond,
            then_expr,
            else_expr,
        } => {
            collect_path_names(ast, *cond, out);
            collect_path_names(ast, *then_expr, out);
            collect_path_names(ast, *else_expr, out);
        }
        _ => {}
    }
}

/// `on` bloğu içindeki (`if`/`match` dâhil) `x <= ...` hedef adları.
fn collect_nonblocking_targets<'a>(ast: &'a SourceFile, block: Idx<Block>, out: &mut Vec<&'a str>) {
    for stmt in &ast.blocks[block].stmts {
        match stmt {
            BlockStmt::NonBlockAssign { lhs, .. } => out.push(lhs.base.text.as_str()),
            BlockStmt::If(i) => collect_if_targets(ast, i, out),
            BlockStmt::Match(m) => {
                for arm in &m.arms {
                    if let MatchArmBody::Block(b) = &arm.body {
                        collect_nonblocking_targets(ast, *b, out);
                    }
                }
            }
            BlockStmt::For(f) => collect_nonblocking_targets(ast, f.body, out),
            _ => {}
        }
    }
}

fn collect_if_targets<'a>(ast: &'a SourceFile, i: &'a volt_ast::IfStmt, out: &mut Vec<&'a str>) {
    collect_nonblocking_targets(ast, i.then_block, out);
    match &i.else_branch {
        Some(ElseBranch::Block(b)) => collect_nonblocking_targets(ast, *b, out),
        Some(ElseBranch::If(e)) => collect_if_targets(ast, e, out),
        None => {}
    }
}

struct Collector<'a> {
    ast: &'a SourceFile,
    domains: HashMap<&'a str, DomainDecl>,
    diags: Vec<Diagnostic>,
    /// Aynı konum için tek E0017 (alt modül birden çok üstten yürünür).
    reported: HashSet<(FileId, u32, u32)>,
}

pub(super) fn collect(ast: &SourceFile) -> ConstraintResult {
    let mut c = Collector {
        ast,
        domains: HashMap::new(),
        diags: Vec::new(),
        reported: HashSet::new(),
    };
    c.collect_domains();
    let mut modules = Vec::new();
    let mut seen_names: HashSet<&str> = HashSet::new();
    for &item_idx in &ast.items {
        let item = &ast.items_arena[item_idx];
        let ItemKind::Module(m) = &item.kind else {
            continue;
        };
        if !seen_names.insert(m.name.text.as_str()) {
            continue;
        }
        if let Some(mc) = c.collect_module(m, &item.attrs) {
            modules.push(mc);
        }
    }
    ConstraintResult {
        modules,
        diagnostics: c.diags,
    }
}

impl<'a> Collector<'a> {
    fn push(&mut self, d: Diagnostic) {
        let key = d
            .primary_span()
            .map(|s| (s.span.file, s.span.start, s.span.end))
            .unwrap_or((FileId(u32::MAX), 0, 0));
        if self.reported.insert(key) {
            self.diags.push(d);
        }
    }

    fn collect_domains(&mut self) {
        for &item_idx in &self.ast.items {
            let ItemKind::Domain(d) = &self.ast.items_arena[item_idx].kind else {
                continue;
            };
            let mut freq_hz = None;
            let has_reset = !d.fields.iter().any(|f| {
                f.key == DomainKey::Reset
                    && match &f.value {
                        DomainValue::ClockEdge(ClockEdge::None) => true,
                        DomainValue::Reset(spec) => spec.sync == ResetSync::None,
                        _ => false,
                    }
            });
            for field in &d.fields {
                if field.key != DomainKey::Frequency {
                    continue;
                }
                match &field.value {
                    DomainValue::Literal(e) => match super::parse::parse_frequency(self.ast, *e) {
                        Ok(hz) => freq_hz = Some(hz),
                        Err(diag) => self.push(*diag),
                    },
                    DomainValue::Error => {}
                    _ => self.push(unsupported(
                        field.span,
                        lstr!(en: "'frequency' expects a frequency literal"; tr: "'frequency' bir frekans literal'i bekler"),
                        lstr!(en: "write frequency = 100.mhz (or 25_175.khz, 25175000)"; tr: "frequency = 100.mhz (ya da 25_175.khz, 25175000) yazın"),
                    )),
                }
            }
            self.domains.insert(
                d.name.text.as_str(),
                DomainDecl {
                    freq_hz,
                    has_reset,
                    span: d.name.span,
                },
            );
        }
    }

    /// Modülün (üst modül kabul edilerek) kısıtları; saat portu yoksa
    /// `None` (nitelik denetimi yine koşar).
    fn collect_module(
        &mut self,
        m: &'a ModuleDecl,
        attrs: &'a [Attribute],
    ) -> Option<ModuleConstraints> {
        let scope = Scope::new(self.ast, m);
        let mut clocks: Vec<ClockConstraint> = scope
            .clock_ports()
            .map(|p| {
                let domain = p
                    .domain
                    .as_ref()
                    .map_or(p.name.text.as_str(), |d| d.text.as_str());
                let decl = p
                    .domain
                    .as_ref()
                    .and_then(|d| self.domains.get(d.text.as_str()));
                ClockConstraint {
                    port: p.name.text.clone(),
                    domain: domain.to_string(),
                    edge: ClockEdge::Posedge,
                    freq_hz: decl.and_then(|d| d.freq_hz),
                    freq_source: decl.and_then(|d| d.freq_hz.map(|_| FreqSource::Domain)),
                    span: p.name.span,
                    domain_span: decl.map(|d| d.span),
                }
            })
            .collect();
        self.apply_edges(&mut clocks);

        let mut paths = Vec::new();
        let mut bridges = Vec::new();
        let mut reset_chains = Vec::new();
        let ctx = Ctx {
            prefix: String::new(),
            clock_map: None,
            depth: 0,
        };
        self.module_attrs(m, attrs, &scope, &ctx, &mut clocks, &mut paths);
        self.body(
            m,
            &scope,
            &ctx,
            &clocks,
            &mut bridges,
            &mut paths,
            &mut reset_chains,
            &mut Vec::new(),
        );

        if clocks.is_empty() {
            return None;
        }
        let mut groups: Vec<Vec<String>> = Vec::new();
        let mut group_of: HashMap<&str, usize> = HashMap::new();
        for c in &clocks {
            let idx = *group_of.entry(c.domain.as_str()).or_insert_with(|| {
                groups.push(Vec::new());
                groups.len() - 1
            });
            groups[idx].push(c.port.clone());
        }
        let raw_resets = m
            .ports
            .iter()
            .filter(|p| is_raw_reset(self.ast, p))
            .map(|p| p.name.text.clone())
            .collect();
        Some(ModuleConstraints {
            module: m.name.text.clone(),
            clocks,
            groups,
            bridges,
            paths,
            raw_resets,
            reset_chains,
        })
    }

    /// Alan bildirimindeki `clock = negedge` kenarı.
    fn apply_edges(&self, clocks: &mut [ClockConstraint]) {
        for &item_idx in &self.ast.items {
            let ItemKind::Domain(d) = &self.ast.items_arena[item_idx].kind else {
                continue;
            };
            let edge = d.fields.iter().find_map(|f| match (&f.key, &f.value) {
                (DomainKey::Clock, DomainValue::ClockEdge(e)) => Some(*e),
                _ => None,
            });
            if let Some(edge) = edge {
                for c in clocks.iter_mut().filter(|c| c.domain == d.name.text) {
                    c.edge = edge;
                }
            }
        }
    }

    // ═══ Nitelikler ══════════════════════════════════════════════

    /// Modül, port ve deyim üstündeki kısıt nitelikleri. Alt modülde
    /// (`ctx.prefix` dolu) yalnız yol kuralları toplanır ve saat
    /// gereksinimleri üst saate karşı denetlenir; biçim hataları alt
    /// modülün kendi geçişinde raporlanır.
    fn module_attrs(
        &mut self,
        m: &'a ModuleDecl,
        attrs: &'a [Attribute],
        scope: &Scope<'a>,
        ctx: &Ctx,
        clocks: &mut [ClockConstraint],
        paths: &mut Vec<PathRule>,
    ) {
        let top = ctx.prefix.is_empty();
        for attr in attrs {
            match attr.name.text.as_str() {
                ATTR_TIMING => {
                    let (forms, diags) = parse_timing(self.ast, attr);
                    if top {
                        diags.into_iter().for_each(|d| self.push(d));
                    }
                    for (form, span) in forms {
                        self.timing_form(form, span, scope, ctx, clocks, paths);
                    }
                }
                ATTR_FALSE_PATH => match parse_false_path(self.ast, attr, false) {
                    Ok(f) => self.path_rule(
                        PathKind::FalsePath,
                        f.from.as_deref(),
                        f.to.as_deref(),
                        attr.span,
                        scope,
                        ctx,
                        paths,
                        "@false_path",
                    ),
                    Err(d) if top => self.push(*d),
                    Err(_) => {}
                },
                ATTR_MULTICYCLE => match parse_multicycle(self.ast, attr, false) {
                    Ok(f) => self.path_rule(
                        PathKind::Multicycle(f.cycles),
                        f.from.as_deref(),
                        f.to.as_deref(),
                        attr.span,
                        scope,
                        ctx,
                        paths,
                        "@multicycle",
                    ),
                    Err(d) if top => self.push(*d),
                    Err(_) => {}
                },
                _ => {}
            }
        }
        for p in &m.ports {
            for attr in &p.attrs {
                self.port_attr(attr, p, scope, ctx, paths);
            }
        }
        for &stmt_idx in &m.body {
            let stmt = &self.ast.stmts[stmt_idx];
            for attr in &stmt.attrs {
                self.stmt_attr(attr, stmt, scope, ctx, paths);
            }
        }
    }

    fn timing_form(
        &mut self,
        form: TimingForm,
        span: Span,
        scope: &Scope<'a>,
        ctx: &Ctx,
        clocks: &mut [ClockConstraint],
        paths: &mut Vec<PathRule>,
    ) {
        match form {
            TimingForm::ClockExact { clock, hz } => {
                self.clock_requirement(&clock, hz, true, span, scope, ctx, clocks)
            }
            TimingForm::ClockAtLeast { clock, hz } => {
                self.clock_requirement(&clock, hz, false, span, scope, ctx, clocks)
            }
            TimingForm::MaxDelay { from, to, ps } => self.path_rule(
                PathKind::MaxDelay(ps),
                Some(&from),
                Some(&to),
                span,
                scope,
                ctx,
                paths,
                "@timing(max_delay)",
            ),
            TimingForm::MinDelay { from, to, ps } => self.path_rule(
                PathKind::MinDelay(ps),
                Some(&from),
                Some(&to),
                span,
                scope,
                ctx,
                paths,
                "@timing(min_delay)",
            ),
        }
    }

    /// `clk = F` / `clk >= F`: portun alanının frekansıyla karşılaştırır;
    /// alan frekansı yoksa bu değer `create_clock`'a kaynak olur.
    #[allow(clippy::too_many_arguments)]
    fn clock_requirement(
        &mut self,
        clock: &str,
        hz: u64,
        exact: bool,
        span: Span,
        scope: &Scope<'a>,
        ctx: &Ctx,
        clocks: &mut [ClockConstraint],
    ) {
        if !scope.is_clock_port(clock) {
            if ctx.prefix.is_empty() {
                self.push(unsupported(
                    span,
                    lstr!(en: "'{clock}' is not a clock port of module '{}'", scope.module.name.text;
                          tr: "'{clock}' '{}' modülünün saat portu değil", scope.module.name.text),
                    lstr!(en: "name a port declared as 'in {clock} : clock'"; tr: "'in {clock} : clock' olarak bildirilmiş bir portu adlandırın"),
                ));
            }
            return;
        }
        let top_name = ctx.map_clock(clock);
        let Some(entry) = top_name.and_then(|n| clocks.iter_mut().find(|c| c.port == n)) else {
            return;
        };
        match entry.freq_hz {
            None => {
                entry.freq_hz = Some(hz);
                entry.freq_source = Some(FreqSource::Timing);
            }
            Some(have) if exact && have != hz => {
                let d = Diagnostic::error(
                    volt_diagnostics::ErrorCode::E0017,
                    lstr!(en: "@timing requires '{clock}' = {hz} Hz, but its domain '{}' declares {have} Hz", entry.domain;
                          tr: "@timing '{clock}' = {hz} Hz istiyor, ama '{}' alanı {have} Hz bildiriyor", entry.domain),
                    LabeledSpan::primary(
                        span,
                        lstr!(en: "conflicts with the domain frequency"; tr: "alan frekansıyla çelişiyor"),
                    ),
                    lstr!(en: "keep one source of truth: declare the frequency in the domain and write '@timing({clock} >= ...)' only for a requirement";
                          tr: "tek doğruluk kaynağı bırakın: frekansı alanda bildirin, '@timing({clock} >= ...)' yalnız gereksinim için yazın"),
                )
                .with_note(
                    NoteKind::Reason,
                    lstr!(en: "a create_clock can carry one period; two different values would silently pick one";
                          tr: "create_clock tek periyot taşır; iki farklı değerden biri sessizce seçilirdi"),
                );
                self.push(match entry.domain_span {
                    Some(ds) => d.with_secondary(
                        ds,
                        lstr!(en: "domain declared here"; tr: "alan burada bildirildi"),
                    ),
                    None => d,
                });
            }
            Some(have) if !exact && have < hz => {
                let d = Diagnostic::error(
                    volt_diagnostics::ErrorCode::E0017,
                    lstr!(en: "@timing requires '{clock}' >= {hz} Hz, but its domain '{}' runs at {have} Hz", entry.domain;
                          tr: "@timing '{clock}' >= {hz} Hz istiyor, ama '{}' alanı {have} Hz'de çalışıyor", entry.domain),
                    LabeledSpan::primary(
                        span,
                        lstr!(en: "requirement not met by the domain frequency"; tr: "alan frekansı gereksinimi karşılamıyor"),
                    ),
                    lstr!(en: "raise the domain frequency or lower the requirement; the module cannot meet its own timing contract at this clock";
                          tr: "alan frekansını yükseltin ya da gereksinimi düşürün; modül bu saatte kendi zamanlama sözünü tutamaz"),
                )
                .with_note(
                    NoteKind::Reason,
                    lstr!(en: "the module was written for at least {hz} Hz (e.g. a pixel or baud clock); a slower clock breaks its function, not only its timing";
                          tr: "modül en az {hz} Hz için yazılmış (ör. piksel ya da baud saati); daha yavaş bir saat yalnız zamanlamayı değil işlevi bozar"),
                );
                self.push(match entry.domain_span {
                    Some(ds) => d.with_secondary(
                        ds,
                        lstr!(en: "domain declared here"; tr: "alan burada bildirildi"),
                    ),
                    None => d,
                });
            }
            Some(_) => {}
        }
    }

    fn port_attr(
        &mut self,
        attr: &'a Attribute,
        port: &'a volt_ast::Port,
        scope: &Scope<'a>,
        ctx: &Ctx,
        paths: &mut Vec<PathRule>,
    ) {
        let name = port.name.text.as_str();
        let (from, to) = match port.direction {
            PortDir::In => (Some(name), None),
            _ => (None, Some(name)),
        };
        match attr.name.text.as_str() {
            ATTR_FALSE_PATH => match parse_false_path(self.ast, attr, true) {
                Ok(f) => {
                    let from = f.from.as_deref().or(from);
                    let to = f.to.as_deref().or(to);
                    self.path_rule(PathKind::FalsePath, from, to, attr.span, scope, ctx, paths, "@false_path");
                }
                Err(d) if ctx.prefix.is_empty() => self.push(*d),
                Err(_) => {}
            },
            ATTR_MULTICYCLE => match parse_multicycle(self.ast, attr, true) {
                Ok(f) => {
                    let from = f.from.as_deref().or(from);
                    let to = f.to.as_deref().or(to);
                    self.path_rule(PathKind::Multicycle(f.cycles), from, to, attr.span, scope, ctx, paths, "@multicycle");
                }
                Err(d) if ctx.prefix.is_empty() => self.push(*d),
                Err(_) => {}
            },
            ATTR_TIMING if ctx.prefix.is_empty() => self.push(unsupported(
                attr.span,
                lstr!(en: "@timing belongs on the module, not on a port"; tr: "@timing porta değil modüle yazılır"),
                lstr!(en: "move it above 'module {}' and name the port there: @timing({name} = 100.mhz)", scope.module.name.text;
                      tr: "'module {}' üstüne taşıyın ve portu orada adlandırın: @timing({name} = 100.mhz)", scope.module.name.text),
            )),
            _ => {}
        }
    }

    fn stmt_attr(
        &mut self,
        attr: &'a Attribute,
        stmt: &'a Stmt,
        scope: &Scope<'a>,
        ctx: &Ctx,
        paths: &mut Vec<PathRule>,
    ) {
        let kind = attr.name.text.as_str();
        if kind != ATTR_FALSE_PATH && kind != ATTR_MULTICYCLE && kind != ATTR_TIMING {
            return;
        }
        let top = ctx.prefix.is_empty();
        let StmtKind::Reg(r) = &stmt.kind else {
            if top {
                self.push(unsupported(
                    attr.span,
                    lstr!(en: "@{kind} on a statement is only allowed above a 'reg' declaration"; tr: "deyim üstünde @{kind} yalnız 'reg' bildiriminin üstünde olabilir"),
                    lstr!(en: "put it on the register the path ends at, or on the module with from = ... / to = ..."; tr: "yolun bittiği register'ın üstüne ya da modüle from = ... / to = ... ile yazın"),
                ));
            }
            return;
        };
        let reg = r.name.text.as_str();
        match kind {
            ATTR_FALSE_PATH => match parse_false_path(self.ast, attr, true) {
                Ok(f) => {
                    let to = f.to.as_deref().or(Some(reg));
                    self.path_rule(PathKind::FalsePath, f.from.as_deref(), to, attr.span, scope, ctx, paths, "@false_path");
                }
                Err(d) if top => self.push(*d),
                Err(_) => {}
            },
            ATTR_MULTICYCLE => match parse_multicycle(self.ast, attr, true) {
                Ok(f) => {
                    let to = f.to.as_deref().or(Some(reg));
                    self.path_rule(PathKind::Multicycle(f.cycles), f.from.as_deref(), to, attr.span, scope, ctx, paths, "@multicycle");
                }
                Err(d) if top => self.push(*d),
                Err(_) => {}
            },
            _ if top => self.push(unsupported(
                attr.span,
                lstr!(en: "@timing belongs on the module, not on a register"; tr: "@timing register'a değil modüle yazılır"),
                lstr!(en: "move it above 'module {}'; use @false_path / @multicycle on a register", scope.module.name.text;
                      tr: "'module {}' üstüne taşıyın; register üstünde @false_path / @multicycle kullanın", scope.module.name.text),
            )),
            _ => {}
        }
    }

    /// Uç noktaları çözer ve kuralı ekler; çözülmeyen ad E0017.
    #[allow(clippy::too_many_arguments)]
    fn path_rule(
        &mut self,
        kind: PathKind,
        from: Option<&str>,
        to: Option<&str>,
        span: Span,
        scope: &Scope<'a>,
        ctx: &Ctx,
        paths: &mut Vec<PathRule>,
        label: &str,
    ) {
        let resolve = |name: Option<&str>| -> Result<Option<Target>, Box<Diagnostic>> {
            name.map(|n| self.endpoint(n, span, scope, ctx)).transpose()
        };
        let (from_t, to_t) = match (resolve(from), resolve(to)) {
            (Ok(f), Ok(t)) => (f, t),
            (Err(d), _) | (_, Err(d)) => {
                if ctx.prefix.is_empty() {
                    self.push(*d);
                }
                return;
            }
        };
        let where_ = if ctx.prefix.is_empty() {
            String::new()
        } else {
            format!(" in {}", ctx.prefix.trim_end_matches('/'))
        };
        paths.push(PathRule {
            kind,
            from: from_t,
            to: to_t,
            comment: format!(
                "{label} on {}{where_}: {} -> {}",
                scope.module.name.text,
                from.unwrap_or("*"),
                to.unwrap_or("*")
            ),
            crossing: None,
        });
    }

    /// Port → `get_ports` (üst) / `get_pins` (alt modül), register →
    /// `get_cells`; wire/let uç nokta olamaz.
    fn endpoint(
        &self,
        name: &str,
        span: Span,
        scope: &Scope<'a>,
        ctx: &Ctx,
    ) -> Result<Target, Box<Diagnostic>> {
        let hier = format!("{}{name}", ctx.prefix);
        if scope.ports.contains_key(name) {
            return Ok(if ctx.prefix.is_empty() {
                Target::Port(hier)
            } else {
                Target::Pin(hier)
            });
        }
        if scope.regs.contains(name) {
            return Ok(Target::Cells(hier));
        }
        if scope.nets.contains(name) {
            return Err(Box::new(unsupported(
                span,
                lstr!(en: "'{name}' is a wire/let, not a timing endpoint"; tr: "'{name}' bir wire/let, zamanlama uç noktası değil"),
                lstr!(en: "paths start and end at ports or registers; name the register '{name}' feeds or is fed by"; tr: "yollar portta ya da register'da başlar ve biter; '{name}'in beslediği ya da beslendiği register'ı adlandırın"),
            )));
        }
        Err(Box::new(unsupported(
            span,
            lstr!(en: "'{name}' is not a port or register of module '{}'", scope.module.name.text;
                  tr: "'{name}' '{}' modülünün portu ya da register'ı değil", scope.module.name.text),
            lstr!(en: "check the spelling; instance outputs (inst.port) cannot be named here"; tr: "yazımı denetleyin; örnek çıkışları (inst.port) burada adlandırılamaz"),
        )))
    }

    // ═══ Gövde: köprüler ve alt modüller ════════════════════════

    #[allow(clippy::too_many_arguments)]
    fn body(
        &mut self,
        m: &'a ModuleDecl,
        scope: &Scope<'a>,
        ctx: &Ctx,
        clocks: &[ClockConstraint],
        bridges: &mut Vec<Bridge>,
        paths: &mut Vec<PathRule>,
        chains: &mut Vec<ResetChain>,
        visiting: &mut Vec<&'a str>,
    ) {
        chains.extend(self.reset_chains(m, scope, ctx));
        for &stmt_idx in &m.body {
            match &self.ast.stmts[stmt_idx].kind {
                StmtKind::Assign(a) => {
                    if let Some(b) = self.sync_bridge(a, scope, ctx, clocks) {
                        bridges.push(b);
                    }
                }
                StmtKind::Instance(inst) => {
                    let target = inst
                        .module_path
                        .segments
                        .last()
                        .map(|s| s.text.as_str())
                        .unwrap_or("");
                    if let Some(prim) = BuiltinPrim::from_name(target) {
                        if let Some(b) = self.builtin_bridge(prim, inst, ctx) {
                            bridges.push(b);
                        }
                    } else {
                        self.user_instance(
                            inst, target, scope, ctx, clocks, bridges, paths, chains, visiting,
                        );
                    }
                }
                _ => {}
            }
        }
    }

    /// `dest = sync(src, dst_clk)` / `sync3(...)` — sv-emit
    /// `try_emit_sync_bridge` adlandırması: `sync_<src>_stage<i>` ve
    /// başka alandan gelen portta `sync_<src>_src` yakalama register'ı.
    fn sync_bridge(
        &self,
        a: &'a volt_ast::AssignStmt,
        scope: &Scope<'a>,
        ctx: &Ctx,
        clocks: &[ClockConstraint],
    ) -> Option<Bridge> {
        let ExprKind::Call { callee, args } = &self.ast.exprs[a.rhs].kind else {
            return None;
        };
        let (kind, stages) = match single(self.ast, *callee)? {
            "sync" => ("sync", 2),
            "sync3" => ("sync3", 3),
            _ => return None,
        };
        if !a.lhs.suffixes.is_empty() || args.len() != 2 {
            return None;
        }
        let src = single(self.ast, args[0])?;
        let dst_clk = single(self.ast, args[1])?;
        if !scope.is_clock_port(dst_clk) {
            return None;
        }
        let src_clk = scope.driver_clock(src).filter(|c| *c != dst_clk);
        let base = format!("{}sync_{src}", ctx.prefix);
        let capture = src_clk.is_some() && scope.ports.contains_key(src);
        let from = if capture {
            Some(Target::Cells(format!("{base}_src")))
        } else if scope.ports.contains_key(src) {
            Some(if ctx.prefix.is_empty() {
                Target::Port(src.to_string())
            } else {
                Target::Pin(format!("{}{src}", ctx.prefix))
            })
        } else if scope.regs.contains(src) {
            Some(Target::Cells(format!("{}{src}", ctx.prefix)))
        } else {
            src_clk
                .and_then(|c| ctx.map_clock(c))
                .filter(|c| clocks.iter().any(|k| k.port == *c && k.freq_hz.is_some()))
                .map(Target::Clock)
        };
        let to_clock = ctx.map_clock(dst_clk);
        let from_clock = src_clk.and_then(|c| ctx.map_clock(c));
        Some(Bridge {
            kind,
            name: base.clone(),
            from_clock: from_clock.clone(),
            to_clock: to_clock.clone(),
            rules: vec![PathRule {
                kind: PathKind::FalsePath,
                from,
                to: Some(Target::Cells(format!("{base}_stage0"))),
                comment: format!(
                    "{kind}(): {src} -> {} ({stages} stages)",
                    to_clock.as_deref().unwrap_or(dst_clk)
                ),
                crossing: Some(Crossing {
                    class: CrossingClass::Control,
                    src_clock: from_clock.clone(),
                }),
            }],
            async_regs: (0..stages).map(|i| format!("{base}_stage{i}")).collect(),
        })
    }

    /// Yerleşik CDC bileşenlerinin geçiş register'ları (sv-emit
    /// `builtin_prim.rs` adlandırması: `<örnek>_<reg>`).
    fn builtin_bridge(
        &self,
        prim: BuiltinPrim,
        inst: &'a InstanceDecl,
        ctx: &Ctx,
    ) -> Option<Bridge> {
        let (kind, crossings, async_regs, src_port, dst_port): PrimSpec = match prim {
            BuiltinPrim::AsyncFifo => (
                "AsyncFifo",
                &[
                    ("rgray", "rgray_s0", CrossingClass::Gray, true),
                    ("wgray", "wgray_s0", CrossingClass::Gray, false),
                    ("mem", "rd_data", CrossingClass::Data, false),
                ],
                &["rgray_s0", "rgray_s1", "wgray_s0", "wgray_s1"],
                "wr_clk",
                "rd_clk",
            ),
            BuiltinPrim::HandshakeSync => (
                "HandshakeSync",
                &[
                    ("req", "req_s0", CrossingClass::Control, false),
                    ("ack", "ack_s0", CrossingClass::Control, true),
                    ("data_q", "data_out", CrossingClass::Data, false),
                ],
                &["req_s0", "req_s1", "ack_s0", "ack_s1"],
                "src_clk",
                "dst_clk",
            ),
            BuiltinPrim::PulseSync => (
                "PulseSync",
                &[("toggle", "sync0", CrossingClass::Control, false)],
                &["sync0", "sync1", "sync2"],
                "src_clk",
                "dst_clk",
            ),
            BuiltinPrim::AsyncDualPortRam => (
                "AsyncDualPortRam",
                &[("mem", "rd_data", CrossingClass::Data, false)],
                &[],
                "wr_clk",
                "rd_clk",
            ),
            _ => return None,
        };
        let name = format!("{}{}", ctx.prefix, inst.name.text);
        let bound_clock = |port: &str| -> Option<String> {
            let b = inst.bindings.iter().find(|b| b.port_name.text == port)?;
            let local = match b.value {
                None => b.port_name.text.as_str(),
                Some(e) => single(self.ast, e)?,
            };
            ctx.map_clock(local)
        };
        let from_clock = bound_clock(src_port);
        let to_clock = bound_clock(dst_port);
        let arrow = format!(
            "{} -> {}",
            from_clock.as_deref().unwrap_or("?"),
            to_clock.as_deref().unwrap_or("?")
        );
        let rules = crossings
            .iter()
            .map(|&(from, to, class, reverse)| PathRule {
                kind: PathKind::FalsePath,
                from: Some(Target::Cells(format!("{name}_{from}"))),
                to: Some(Target::Cells(format!("{name}_{to}"))),
                comment: format!("{kind} '{}': {from} -> {to} ({arrow})", inst.name.text),
                crossing: Some(Crossing {
                    class,
                    src_clock: if reverse {
                        to_clock.clone()
                    } else {
                        from_clock.clone()
                    },
                }),
            })
            .collect();
        Some(Bridge {
            kind,
            name: name.clone(),
            from_clock,
            to_clock,
            rules,
            async_regs: async_regs.iter().map(|r| format!("{name}_{r}")).collect(),
        })
    }

    /// Kullanıcı modülü örneği: alt modülün köprüleri ve yol kuralları
    /// `örnek/` önekiyle, saatleri bağlama üzerinden üst adlara eşlenerek.
    #[allow(clippy::too_many_arguments)]
    fn user_instance(
        &mut self,
        inst: &'a InstanceDecl,
        target: &str,
        scope: &Scope<'a>,
        ctx: &Ctx,
        clocks: &[ClockConstraint],
        bridges: &mut Vec<Bridge>,
        paths: &mut Vec<PathRule>,
        chains: &mut Vec<ResetChain>,
        visiting: &mut Vec<&'a str>,
    ) {
        if ctx.depth >= MAX_DEPTH || visiting.contains(&target) {
            return;
        }
        let Some((sub, sub_attrs)) = self.module_named(target) else {
            return;
        };
        let sub_scope = Scope::new(self.ast, sub);
        let mut clock_map: HashMap<String, String> = HashMap::new();
        for p in sub_scope.clock_ports() {
            let Some(b) = inst
                .bindings
                .iter()
                .find(|b| b.port_name.text == p.name.text)
            else {
                continue;
            };
            let local = match b.value {
                None => Some(b.port_name.text.as_str()),
                Some(e) => single(self.ast, e),
            };
            if let Some(mapped) = local
                .filter(|l| scope.is_clock_port(l))
                .and_then(|l| ctx.map_clock(l))
            {
                clock_map.insert(p.name.text.clone(), mapped);
            }
        }
        let sub_ctx = Ctx {
            prefix: format!("{}{}/", ctx.prefix, inst.name.text),
            clock_map: Some(clock_map),
            depth: ctx.depth + 1,
        };
        let mut sub_clocks: Vec<ClockConstraint> = clocks.to_vec();
        visiting.push(target_static(sub));
        self.module_attrs(sub, sub_attrs, &sub_scope, &sub_ctx, &mut sub_clocks, paths);
        self.body(
            sub, &sub_scope, &sub_ctx, clocks, bridges, paths, chains, visiting,
        );
        visiting.pop();
    }

    /// Modülün ham reset senkronizörleri (ADR-0065 §1-§2) — sv-emit
    /// `reset_sync::feeding_raw` ile aynı bağlama kuralı: tek anotasyonsuz
    /// ham port reset'li bütün alanları besler; aksi hâlde `@D` anotasyonu
    /// saat portunun alanıyla eşleşen. `reset = none` alanı zincir almaz.
    fn reset_chains(&self, m: &'a ModuleDecl, scope: &Scope<'a>, ctx: &Ctx) -> Vec<ResetChain> {
        let raws: Vec<&Port> = m
            .ports
            .iter()
            .filter(|p| is_raw_reset(self.ast, p))
            .collect();
        if raws.is_empty() {
            return Vec::new();
        }
        let mut chains = Vec::new();
        for clk in scope.clock_ports() {
            let domain = clk.domain.as_ref().map(|d| d.text.as_str());
            let has_reset = domain
                .and_then(|d| self.domains.get(d))
                .is_none_or(|d| d.has_reset);
            if !has_reset {
                continue;
            }
            let raw = match raws.as_slice() {
                [only] if only.domain.is_none() => Some(*only),
                raws => raws
                    .iter()
                    .find(|r| {
                        r.domain
                            .as_ref()
                            .is_some_and(|d| Some(d.text.as_str()) == domain)
                    })
                    .copied(),
            };
            let Some(raw) = raw else {
                continue;
            };
            let local = clk.name.text.as_str();
            chains.push(ResetChain {
                raw_port: raw.name.text.clone(),
                clock: ctx.map_clock(local),
                stages: (0..RESET_SYNC_STAGES)
                    .map(|i| format!("{}rst_sync_{local}_stage{i}", ctx.prefix))
                    .collect(),
            });
        }
        chains
    }

    fn module_named(&self, name: &str) -> Option<(&'a ModuleDecl, &'a [Attribute])> {
        self.ast.items.iter().find_map(|&i| {
            let item = &self.ast.items_arena[i];
            match &item.kind {
                ItemKind::Module(m) if m.name.text == name => Some((m, item.attrs.as_slice())),
                _ => None,
            }
        })
    }
}

/// Ham reset portu: giriş yönlü `reset` tipli port (sv-emit
/// `reset_sync::is_raw_reset`).
fn is_raw_reset(ast: &SourceFile, port: &Port) -> bool {
    port.direction == PortDir::In && matches!(ast.types[port.ty].kind, TypeRefKind::Reset(_))
}

/// Modül adının `'a` ömürlü dilimi (ziyaret yığını için).
fn target_static(m: &ModuleDecl) -> &str {
    m.name.text.as_str()
}

/// Hiyerarşi bağlamı: önek ve alt saat portu → üst saat adı eşlemesi.
struct Ctx {
    prefix: String,
    /// Üst modülde `None` (adlar doğrudan geçer).
    clock_map: Option<HashMap<String, String>>,
    depth: usize,
}

impl Ctx {
    fn map_clock(&self, local: &str) -> Option<String> {
        match &self.clock_map {
            None => Some(local.to_string()),
            Some(map) => map.get(local).cloned(),
        }
    }
}

fn single(ast: &SourceFile, e: Idx<volt_ast::Expr>) -> Option<&str> {
    match &ast.exprs[e].kind {
        ExprKind::Path(p) if p.segments.len() == 1 => Some(p.segments[0].text.as_str()),
        _ => None,
    }
}
