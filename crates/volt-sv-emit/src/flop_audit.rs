//! Üreticide varsayım yok: flop denetimi (ADR-0098 eki 2).
//!
//! SV üreticisi her flop'un saat kenarını ve reset dalını kaynaktan
//! türetir; türetemediği hiçbir şeyi varsaymaz. Eskiden üç varsayım
//! vardı: kenarsız alan (`clock = none`) `posedge` yazılıyordu; `on`
//! bloğunun tetikleyicisi saat portu değilse ilk saatin (o da yoksa
//! varsayılan alanın) yapılandırması alınıyordu; `on` bloğunda yazılan
//! register olmayan hedef (tel, çıkış portu) reset dalında hiç yer
//! almıyordu (boş `if (rst) begin end`).
//!
//! Kural iki katmanlıdır:
//!
//! 1. **Üretimden önce denetim** (bu modül). Birimin her modülü gezilir,
//!    flop üreten her yer için kenar ve reset bilgisi kaynaktan
//!    türetilebiliyor mu bakılır. İhlal tanı olur ve hiçbir modül
//!    üretilmez. Flop üreten yerler: `on` blokları; `sync()` köprüsünün
//!    hedef aşamaları ve kaynak yakalama flop'u; yerleşik primitiflerin
//!    saat portları; kontratlar (kontrat üreten kiplerde, modülün ilk saat
//!    portunda); reset senkronizörü (yalnız bu flop'lardan biri onu
//!    kullanınca üretilir, kendi başına denetlenmez).
//! 2. **Üretimde temsil edilemezlik.** Kenar `DomainInfo::flop_edge`'den
//!    gelir: kenarsız alan orada `unreachable!` (ICE). Reset dalı
//!    `reset_assignments`'tan gelir: register olmayan hedef orada
//!    `unreachable!`. Denetim ikisini de üretime ulaşmadan durdurur.
//!
//! Ön uç aynı kuralları kullanıcı tanısı olarak verir (E3016, E0020);
//! buradaki tanı ön ucun kaçırdığı bir girdiyi gösterir.

use volt_ast::{
    BlockStmt, ElseBranch, Expr, ExprKind, IfStmt, ItemKind, MatchArmBody, ModuleDecl, OnTrigger,
    Port, PortDir, SourceFile, StmtKind, TypeRefKind,
};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use std::collections::HashMap;

use volt_ast::builtin::{BuiltinPrim, PortKind};
use volt_ast::{Block, ClockEdge, Idx};

use crate::{clock_ports_of, is_sync_call, path_single, ClockPort, DomainInfo, Emitter};

/// Saat portunun alanı, kaynaktan (ADR-0098 eki 2): açıklamasız port
/// örtük alandadır (spec varsayılanı, [`DomainInfo::SPEC_DEFAULT`]);
/// `@Alan` bir `domain` bildirimiyse onun bilgisi; `@saat` aynı modülün
/// başka bir saat portuysa onun alanı. Türetilemiyorsa (bilinmeyen ad,
/// açıklama döngüsü) `None`.
pub(crate) fn port_domain(
    ast: &SourceFile,
    domains: &HashMap<String, DomainInfo>,
    module: &ModuleDecl,
    port: &Port,
) -> Option<DomainInfo> {
    let mut cur = port;
    let mut seen: Vec<&str> = Vec::new();
    loop {
        let Some(ann) = &cur.domain else {
            return Some(DomainInfo::SPEC_DEFAULT);
        };
        if let Some(info) = domains.get(&ann.text) {
            return Some(info.clone());
        }
        if seen.contains(&ann.text.as_str()) {
            return None;
        }
        seen.push(&ann.text);
        cur = module
            .ports
            .iter()
            .find(|p| p.name.text == ann.text && is_clock_type(ast, p))?;
    }
}

pub(crate) fn is_clock_type(ast: &SourceFile, p: &Port) -> bool {
    matches!(
        ast.types[crate::alias::resolve(ast, p.ty)].kind,
        TypeRefKind::Clock
    )
}

impl Emitter<'_> {
    /// Üretimden önce flop denetimi: birimin her modülünde her flop'un
    /// kenarı ve reset dalı kaynaktan türetilebiliyor mu. İhlaller tanı
    /// olarak eklenir; temizse true.
    pub(crate) fn audit_flops(&mut self) -> bool {
        let ast = self.ast;
        let before = self.diagnostics.len();
        for &item in &ast.items {
            if let ItemKind::Module(m) = &ast.items_arena[item].kind {
                self.audit_module(m);
            }
        }
        self.diagnostics[before..]
            .iter()
            .all(|d| d.code.is_warning())
    }

    fn audit_module(&mut self, m: &ModuleDecl) {
        let ast = self.ast;
        // Saat portlarının alanı türetilemiyorsa modülün flop'ları da
        // türetilemez: ilk ihlal yeter.
        for p in m.ports.iter().filter(|p| is_clock_type(ast, p)) {
            if port_domain(ast, &self.domains, m, p).is_none() {
                let ann = p.domain.as_ref().map_or("", |a| a.text.as_str());
                self.refuse(
                    ErrorCode::E3002,
                    p.name.span,
                    lstr!(
                        en: "cannot derive the clock domain of '{}' from '@{ann}'", p.name.text;
                        tr: "'{}' saatinin alanı '@{ann}' açıklamasından türetilemiyor", p.name.text
                    ),
                    lstr!(
                        en: "annotate the clock port with a 'domain' declaration, or remove the \
                             annotation (the port then has its own implicit domain)";
                        tr: "saat portunu bir 'domain' bildirimiyle açıklayın ya da açıklamayı \
                             kaldırın (port o zaman kendi örtük alanındadır)"
                    ),
                );
                return;
            }
        }
        let clocks = clock_ports_of(ast, &self.domains, m);
        for &s in &m.body {
            match &ast.stmts[s].kind {
                StmtKind::On(on) => {
                    let span = ast.stmts[s].span;
                    let name = match &on.trigger {
                        OnTrigger::Clock(n) | OnTrigger::Reset(n) => n,
                        OnTrigger::Error => {
                            self.refuse_trigger(span, "", &m.name.text);
                            continue;
                        }
                    };
                    let Some(clock) = clocks.iter().find(|c| c.name == name.text) else {
                        self.refuse_trigger(name.span, &name.text, &m.name.text);
                        continue;
                    };
                    self.audit_edge(
                        clock,
                        name.span,
                        lstr!(en: "an 'on' block"; tr: "bir 'on' bloğu"),
                    );
                    if !clock.info.reset.is_none() {
                        self.audit_reset_targets(m, on.body);
                    }
                }
                StmtKind::Assign(a) if is_sync_call(ast, a.rhs) => {
                    self.audit_sync(m, &clocks, a.rhs);
                }
                StmtKind::Let(l) if is_sync_call(ast, l.value) => {
                    self.audit_sync(m, &clocks, l.value);
                }
                StmtKind::Instance(inst) => self.audit_builtin(&clocks, inst),
                StmtKind::Reg(_)
                | StmtKind::Let(_)
                | StmtKind::Wire(_)
                | StmtKind::Assign(_)
                | StmtKind::Comb(_)
                | StmtKind::For(_)
                | StmtKind::Expr(_)
                | StmtKind::Error => {}
            }
        }
        // Kontratlar yalnız kontrat üreten kiplerde flop/örnekleme olur.
        if self.sva_mode != crate::SvaMode::None {
            if let (Some(first), Some(clock)) = (m.contracts.first(), clocks.first()) {
                self.audit_edge(clock, first.span, lstr!(en: "contracts"; tr: "kontratlar"));
            }
        }
    }

    /// `sync(src, clk)`: hedef aşamalar `clk`'de; kaynak `@Alan`
    /// açıklamalı bir portsa ve aynı açıklamalı ilk saat hedef değilse
    /// kaynak önce o saatte yakalanır (`try_emit_sync_bridge` ile aynı
    /// kural). Köprü kurulamayan biçimler üretimde E0003 alır.
    fn audit_sync(&mut self, m: &ModuleDecl, clocks: &[ClockPort], call: Idx<Expr>) {
        let ast = self.ast;
        let ExprKind::Call { args, .. } = &ast.exprs[call].kind else {
            return;
        };
        let [src_arg, clk_arg] = args.as_slice() else {
            return;
        };
        let (Some(src), Some(dst_name)) = (path_single(ast, *src_arg), path_single(ast, *clk_arg))
        else {
            return;
        };
        let Some(dst) = clocks.iter().find(|c| c.name == dst_name) else {
            return;
        };
        let what = lstr!(en: "a sync() destination"; tr: "bir sync() hedefi");
        self.audit_edge(dst, ast.exprs[*clk_arg].span, what);
        let capture = m
            .ports
            .iter()
            .find(|p| p.name.text == src)
            .and_then(|p| p.domain.as_ref())
            .and_then(|d| clocks.iter().find(|c| c.domain.as_deref() == Some(&d.text)))
            .filter(|c| c.name != dst.name);
        if let Some(cap) = capture {
            let what = lstr!(en: "a sync() source capture"; tr: "bir sync() kaynak yakalaması");
            self.audit_edge(cap, ast.exprs[*src_arg].span, what);
        }
    }

    /// Yerleşik primitifin saat portları (`builtin_prim` aynı adla bulur;
    /// bulunamayan bağlama üretimde E0003 alır).
    fn audit_builtin(&mut self, clocks: &[ClockPort], inst: &volt_ast::InstanceDecl) {
        let ast = self.ast;
        let prim = match inst.module_path.segments.as_slice() {
            [seg] => BuiltinPrim::from_name(&seg.text),
            _ => None,
        };
        let Some(prim) = prim else {
            return;
        };
        for b in &inst.bindings {
            let Some(port) = prim.port(&b.port_name.text) else {
                continue;
            };
            if port.kind != PortKind::Clock || port.dir != PortDir::In {
                continue;
            }
            let name = match b.value {
                Some(e) => path_single(ast, e),
                None => Some(b.port_name.text.as_str()),
            };
            if let Some(clock) = name.and_then(|n| clocks.iter().find(|c| c.name == n)) {
                self.audit_edge(clock, b.span, prim.name().to_string());
            }
        }
    }

    /// Flop'u zamanlayan saatin alanının kenarı olmalı (E3016).
    fn audit_edge(&mut self, clock: &ClockPort, span: Span, what: String) {
        if !matches!(clock.info.edge, ClockEdge::None) {
            return;
        }
        let name = clock.domain.as_deref().unwrap_or(&clock.name);
        self.refuse(
            ErrorCode::E3016,
            span,
            lstr!(
                en: "{what} would clock flip-flops on '{}', a clock of domain '{name}', which has no clock edge", clock.name;
                tr: "{what}, saat kenarı olmayan '{name}' alanının '{}' saatiyle flop zamanlardı", clock.name
            ),
            lstr!(
                en: "give the domain a clock edge: domain {name} {{ clock = posedge }}";
                tr: "alana bir saat kenarı verin: domain {name} {{ clock = posedge }}"
            ),
        );
    }

    /// Reset'li alandaki `on` bloğunda yazılan her hedef bir register
    /// olmalı: reset değeri yalnız `reg` bildiriminden gelir (E0020).
    fn audit_reset_targets(&mut self, m: &ModuleDecl, body: Idx<Block>) {
        let ast = self.ast;
        let regs: Vec<&str> = m
            .body
            .iter()
            .filter_map(|&s| {
                if let StmtKind::Reg(r) = &ast.stmts[s].kind {
                    Some(r.name.text.as_str())
                } else {
                    None
                }
            })
            .collect();
        let mut targets = Vec::new();
        written_targets(ast, &ast.blocks[body], &mut targets);
        for (name, span) in targets {
            if regs.contains(&name.as_str()) {
                continue;
            }
            self.refuse(
                ErrorCode::E0020,
                span,
                lstr!(
                    en: "'{name}' is written with '<=' in an 'on' block but is not a register, so its flip-flop has no reset value";
                    tr: "'{name}' bir 'on' bloğunda '<=' ile yazılıyor ama register değil; flop'unun reset değeri yok"
                ),
                lstr!(
                    en: "keep the value in a register with a reset value ('reg {name}_r : <type> = <reset value>') and drive '{name}' from it";
                    tr: "değeri reset değerli bir register'da tutun ('reg {name}_r : <tip> = <reset değeri>') ve '{name}' sinyalini ondan sürün"
                ),
            );
        }
    }

    fn refuse_trigger(&mut self, span: Span, name: &str, module: &str) {
        self.refuse(
            ErrorCode::E3002,
            span,
            lstr!(
                en: "'{name}' is not a clock port of '{module}': an 'on' block is triggered by a clock";
                tr: "'{name}' '{module}' modülünün saat portu değil: 'on' bloğunu bir saat tetikler"
            ),
            lstr!(
                en: "declare the trigger as a clock port (in {name} : clock) or write 'on <clock>' with one of the module's clock ports";
                tr: "tetikleyiciyi saat portu olarak bildirin (in {name} : clock) ya da modülün saat portlarından biriyle 'on <saat>' yazın"
            ),
        );
    }

    /// Denetim tanısı, 5 parça: kod, konum, açıklama, öneri, gerekçe.
    fn refuse(&mut self, code: ErrorCode, span: Span, message: String, help: String) {
        let diag = Diagnostic::error(
            code,
            message,
            LabeledSpan::primary(
                span,
                lstr!(
                    en: "SystemVerilog generation stops here";
                    tr: "SystemVerilog üretimi burada durur"
                ),
            ),
            help,
        )
        .with_note(
            NoteKind::Reason,
            lstr!(
                en: "the SystemVerilog generator checks every flip-flop before generating and \
                     does not assume a clock edge or a reset value (ADR-0098); an earlier check \
                     should have reported this input, so reaching the generator is a compiler \
                     bug: please report it";
                tr: "SystemVerilog üreticisi üretimden önce her flop'u denetler ve saat kenarı \
                     ya da reset değeri varsaymaz (ADR-0098); bu girdiyi daha önceki bir denetim \
                     bildirmeliydi, üreticiye ulaşması bir derleyici hatasıdır: lütfen bildirin"
            ),
        );
        if !self.diagnostics.contains(&diag) {
            self.diagnostics.push(diag);
        }
    }
}

/// `on` gövdesinde `<=`/`=` hedefleri (taban ad ve konum), ilk görülme
/// sırasıyla — `collect_written` (reset dalı) ile aynı küme.
fn written_targets(ast: &SourceFile, block: &Block, out: &mut Vec<(String, Span)>) {
    for stmt in &block.stmts {
        match stmt {
            BlockStmt::NonBlockAssign { lhs, .. } | BlockStmt::BlockAssign { lhs, .. } => {
                if !out.iter().any(|(n, _)| n == &lhs.base.text) {
                    out.push((lhs.base.text.clone(), lhs.span));
                }
            }
            BlockStmt::If(if_stmt) => written_targets_if(ast, if_stmt, out),
            BlockStmt::Match(mt) => {
                for arm in &mt.arms {
                    match &arm.body {
                        MatchArmBody::Block(b) => written_targets(ast, &ast.blocks[*b], out),
                        // Deyim konumunda ifade gövdeli kol atama yapamaz.
                        MatchArmBody::Expr(_) => {}
                    }
                }
            }
            BlockStmt::For(f) => written_targets(ast, &ast.blocks[f.body], out),
            BlockStmt::Let(_) | BlockStmt::Error => {}
        }
    }
}

fn written_targets_if(ast: &SourceFile, if_stmt: &IfStmt, out: &mut Vec<(String, Span)>) {
    written_targets(ast, &ast.blocks[if_stmt.then_block], out);
    match &if_stmt.else_branch {
        Some(ElseBranch::Block(b)) => written_targets(ast, &ast.blocks[*b], out),
        Some(ElseBranch::If(elif)) => written_targets_if(ast, elif, out),
        None => {}
    }
}
