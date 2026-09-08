//! Volt AST → SystemVerilog üretimi (F0, string template).
//!
//! Bağlayıcı referans: docs/spec/sv-mapping.md. CIRCT yok — F3'te
//! volt-lower devralacak. Tip bilgisi kaba çıkarımla gelir (F2'de HIR
//! düzeltecek); belirsizlikte E2005 üretilir, tahmin edilmez.

mod builtin_prim;
mod expr;
mod sby;
mod sva;

use std::collections::HashMap;

use volt_ast::builtin::BuiltinPrim;
use volt_ast::{
    AssignStmt, Block, BlockStmt, ClockEdge, DomainKey, DomainValue, ElseBranch, Expr, ExprKind,
    Idx, IfStmt, ItemKind, LValue, LValueSuffix, ModuleDecl, OnBlock, OnTrigger, PortDir,
    ResetPolarity, ResetSync, SourceFile, StmtKind, TypeRef, TypeRefKind,
};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, Severity};
use volt_span::Span;

pub use expr::Sig;
pub use sby::{sby_config, SbyEngine, SbyMode, SbyOptions};
pub use sva::{SvaFile, SvaMode, SvaProp};

pub const VOLT_VERSION: &str = "0.1.0";

#[derive(Debug)]
pub struct EmitResult {
    pub sv: String,
    pub diagnostics: Vec<Diagnostic>,
}

impl EmitResult {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }
}

/// Reset üretim varyantı (sv-mapping.md §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResetCfg {
    pub(crate) sync: ResetSync, // None → reset yok
    pub(crate) polarity: ResetPolarity,
}

impl ResetCfg {
    const DEFAULT: ResetCfg = ResetCfg {
        sync: ResetSync::Sync,
        polarity: ResetPolarity::ActiveHigh,
    };

    pub(crate) fn is_none(&self) -> bool {
        self.sync == ResetSync::None
    }

    pub(crate) fn port_name(&self) -> &'static str {
        match self.polarity {
            ResetPolarity::ActiveHigh => "rst",
            ResetPolarity::ActiveLow => "rst_n",
        }
    }

    pub(crate) fn condition(&self) -> &'static str {
        match self.polarity {
            ResetPolarity::ActiveHigh => "rst",
            ResetPolarity::ActiveLow => "!rst_n",
        }
    }

    fn async_sensitivity(&self) -> &'static str {
        match (self.sync, self.polarity) {
            (ResetSync::Async, ResetPolarity::ActiveHigh) => " or posedge rst",
            (ResetSync::Async, ResetPolarity::ActiveLow) => " or negedge rst_n",
            _ => "",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct DomainInfo {
    pub(crate) edge: ClockEdge,
    pub(crate) reset: ResetCfg,
}

const DEFAULT_DOMAIN: DomainInfo = DomainInfo {
    edge: ClockEdge::Posedge,
    reset: ResetCfg::DEFAULT,
};

/// Modülün bir saat portu: adı, `@Domain` anotasyonu ve alan bilgisi.
#[derive(Debug, Clone)]
pub(crate) struct ClockPort {
    pub(crate) name: String,
    pub(crate) domain: Option<String>,
    pub(crate) info: DomainInfo,
}

/// Benzersiz reset portları, saat portu sırası korunarak (ada göre teklenir).
fn reset_port_set(clocks: &[ClockPort]) -> Vec<ResetCfg> {
    let mut out: Vec<ResetCfg> = Vec::new();
    for clock in clocks {
        let cfg = clock.info.reset;
        if cfg.is_none() {
            continue;
        }
        if !out.iter().any(|c| c.port_name() == cfg.port_name()) {
            out.push(cfg);
        }
    }
    out
}

/// `on <clk>` bloğunun alanı: tetikleyen saat portundan; bulunamazsa
/// ilk saat portu, o da yoksa varsayılan alan.
fn domain_of_trigger(clocks: &[ClockPort], on: &OnBlock) -> DomainInfo {
    let name = match &on.trigger {
        OnTrigger::Clock(n) | OnTrigger::Reset(n) => Some(n.text.as_str()),
        OnTrigger::Error => None,
    };
    name.and_then(|n| clocks.iter().find(|c| c.name == n))
        .or_else(|| clocks.first())
        .map(|c| c.info)
        .unwrap_or(DEFAULT_DOMAIN)
}

/// Tek segmentli Path ifadesinin metni.
fn path_single(ast: &SourceFile, idx: Idx<Expr>) -> Option<&str> {
    match &ast.exprs[idx].kind {
        ExprKind::Path(p) if p.segments.len() == 1 => Some(p.segments[0].text.as_str()),
        _ => None,
    }
}

/// Reset değeri olarak sıfır literali (§10 boyutlandırması).
fn zero_of(sig: Sig) -> String {
    match (sig.width, sig.signed) {
        (1, _) => "1'b0".to_string(),
        (w, false) => format!("{w}'d0"),
        (w, true) => format!("{w}'sd0"),
    }
}

/// Senkronizatör aşamaları için always_ff bloğu (§4 reset varyantları).
fn sync_always_ff(clk: &str, info: DomainInfo, chain: &[(String, String)], zero: &str) -> String {
    let edge = match info.edge {
        ClockEdge::Negedge => "negedge",
        _ => "posedge",
    };
    let cfg = info.reset;
    let mut out = String::new();
    if cfg.is_none() {
        out.push_str(&format!("    always_ff @({edge} {clk}) begin\n"));
        for (lhs, rhs) in chain {
            out.push_str(&format!("        {lhs} <= {rhs};\n"));
        }
        out.push_str("    end");
    } else {
        out.push_str(&format!(
            "    always_ff @({edge} {clk}{}) begin\n",
            cfg.async_sensitivity()
        ));
        out.push_str(&format!("        if ({}) begin\n", cfg.condition()));
        for (lhs, _) in chain {
            out.push_str(&format!("            {lhs} <= {zero};\n"));
        }
        out.push_str("        end else begin\n");
        for (lhs, rhs) in chain {
            out.push_str(&format!("            {lhs} <= {rhs};\n"));
        }
        out.push_str("        end\n    end");
    }
    out
}

/// Dosyadaki tüm modülleri tek SV dosyasına üretir (SVA'sız).
pub fn emit(ast: &SourceFile, source_name: &str) -> EmitResult {
    let out = emit_full(ast, source_name, "", SvaMode::None);
    EmitResult {
        sv: out.sv,
        diagnostics: out.diagnostics,
    }
}

/// SV + SVA çıktısı (F4a). `source` kaynak metni — SVA yorumlarındaki
/// satır numaraları buradan hesaplanır.
#[derive(Debug)]
pub struct EmitOutput {
    pub sv: String,
    /// Ayrı modda kontratlı her modül için bir .sva içeriği.
    pub sva_files: Vec<SvaFile>,
    /// Üretilen her property'nin kimliği (F4b — sby FAIL eşlemesi).
    pub sva_props: Vec<SvaProp>,
    /// İki ve daha çok saat portlu modüller — `.sby` dosyasına
    /// `multiclock on` eklenmesi için (ADR-0027, clk2fflogic akışı).
    pub multiclock_modules: Vec<String>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Tüm modülleri üretir; `mode`'a göre kontratlardan SVA da çıkarır.
pub fn emit_full(ast: &SourceFile, source_name: &str, source: &str, mode: SvaMode) -> EmitOutput {
    let mut emitter = Emitter {
        ast,
        diagnostics: Vec::new(),
        domains: collect_domains(ast),
        symbols: HashMap::new(),
        builtin_insts: HashMap::new(),
        source,
        source_name,
        sva_mode: mode,
        sva_files: Vec::new(),
        sva_props: Vec::new(),
    };

    let mut modules = Vec::new();
    let mut multiclock_modules = Vec::new();
    for &item_idx in &ast.items {
        let item = &ast.items_arena[item_idx];
        if let ItemKind::Module(module) = &item.kind {
            if emitter.collect_clock_ports(module).len() >= 2 {
                multiclock_modules.push(module.name.text.clone());
            }
            modules.push(emitter.emit_module(module, item.doc.as_deref()));
        }
    }

    let mut sv = header(source_name);
    sv.push('\n');
    sv.push_str(&modules.join("\n"));
    sv.push('\n');
    sv.push_str("`default_nettype wire\n");

    EmitOutput {
        sv,
        sva_files: emitter.sva_files,
        sva_props: emitter.sva_props,
        multiclock_modules,
        diagnostics: emitter.diagnostics,
    }
}

/// sv-mapping.md §12 — deterministik başlık (tarih yok).
pub(crate) fn header(source_name: &str) -> String {
    format!(
        "// This file was generated by Volt.\n\
         // Source:  {source_name}\n\
         // Version: {VOLT_VERSION}\n\
         //\n\
         // DO NOT EDIT — make changes in the source file instead.\n\
         \n\
         `default_nettype none\n"
    )
}

fn collect_domains(ast: &SourceFile) -> HashMap<String, DomainInfo> {
    let mut map = HashMap::new();
    for &item_idx in &ast.items {
        if let ItemKind::Domain(domain) = &ast.items_arena[item_idx].kind {
            let mut info = DEFAULT_DOMAIN;
            for field in &domain.fields {
                match (&field.key, &field.value) {
                    (DomainKey::Clock, DomainValue::ClockEdge(edge)) => info.edge = *edge,
                    (DomainKey::Reset, DomainValue::Reset(spec)) => {
                        info.reset = ResetCfg {
                            sync: spec.sync,
                            polarity: spec.polarity,
                        };
                    }
                    (DomainKey::Reset, DomainValue::ClockEdge(ClockEdge::None)) => {
                        info.reset = ResetCfg {
                            sync: ResetSync::None,
                            ..ResetCfg::DEFAULT
                        };
                    }
                    _ => {}
                }
            }
            map.insert(domain.name.text.clone(), info);
        }
    }
    map
}

pub(crate) struct Emitter<'a> {
    pub(crate) ast: &'a SourceFile,
    pub(crate) diagnostics: Vec<Diagnostic>,
    domains: HashMap<String, DomainInfo>,
    /// Modül içi sinyal tablosu: isim → genişlik/işaret.
    pub(crate) symbols: HashMap<String, Sig>,
    /// Modül içi yerleşik CDC primitif örnekleri (ADR-0027): örnek adı →
    /// doğrulanmış bilgi. Ön geçişte doldurulur ki `f.rd_data` alan
    /// erişimleri deyim sırasından bağımsız `f_rd_data`'ya çevrilsin.
    pub(crate) builtin_insts: HashMap<String, builtin_prim::BuiltinInst>,
    /// Kaynak metin — SVA yorumlarındaki satır numaraları için.
    pub(crate) source: &'a str,
    pub(crate) source_name: &'a str,
    pub(crate) sva_mode: SvaMode,
    pub(crate) sva_files: Vec<SvaFile>,
    pub(crate) sva_props: Vec<SvaProp>,
}

impl<'a> Emitter<'a> {
    pub(crate) fn error(&mut self, code: ErrorCode, message: String, span: Span, help: &str) {
        self.diagnostics.push(Diagnostic::error(
            code,
            message,
            LabeledSpan::primary(span, ""),
            help,
        ));
    }

    pub(crate) fn future(&mut self, span: Span, what: &str) {
        self.error(
            ErrorCode::E0003,
            lstr!(
                en: "{what} is not supported in F0 SV generation";
                tr: "{what} F0 SV üretiminde desteklenmiyor"
            ),
            span,
            &lstr!(
                en: "this construct will be added in F1+";
                tr: "bu yapı F1+ sürümünde eklenecek"
            ),
        );
    }

    /// Saat portları, port sırasıyla; `@Domain` yoksa varsayılan alan.
    fn collect_clock_ports(&self, module: &ModuleDecl) -> Vec<ClockPort> {
        module
            .ports
            .iter()
            .filter(|p| matches!(self.ast.types[p.ty].kind, TypeRefKind::Clock))
            .map(|p| {
                let domain = p.domain.as_ref().map(|d| d.text.clone());
                let info = domain
                    .as_ref()
                    .and_then(|d| self.domains.get(d).copied())
                    .unwrap_or(DEFAULT_DOMAIN);
                ClockPort {
                    name: p.name.text.clone(),
                    domain,
                    info,
                }
            })
            .collect()
    }

    // ═══ Modül ════════════════════════════════════════════════════

    fn emit_module(&mut self, module: &'a ModuleDecl, doc: Option<&str>) -> String {
        let ast = self.ast;
        self.symbols.clear();

        // Sembol tablosu: portlar + reg'ler (let'ler sırayla eklenir)
        for port in &module.ports {
            if let Some(sig) = self.sig_of_typeref(port.ty, port.span) {
                self.symbols.insert(port.name.text.clone(), sig);
            }
        }
        for &stmt_idx in &module.body {
            if let StmtKind::Reg(reg) = &ast.stmts[stmt_idx].kind {
                let span = ast.stmts[stmt_idx].span;
                match reg.ty {
                    Some(ty) => {
                        if let Some(sig) = self.sig_of_typeref(ty, span) {
                            self.symbols.insert(reg.name.text.clone(), sig);
                        }
                    }
                    None => self.error(
                        ErrorCode::E2012,
                        lstr!(
                            en: "cannot determine the type of register '{}'", reg.name.text;
                            tr: "'{}' register tipi belirlenemiyor", reg.name.text
                        ),
                        span,
                        &lstr!(
                            en: "in F0 the reg type must be written explicitly: reg name : u8 = 0";
                            tr: "F0'da reg tipi açık yazılmalı: reg isim : u8 = 0"
                        ),
                    ),
                }
            }
        }

        // Saat portları ve alan başına reset yapılandırması (§7)
        let clocks = self.collect_clock_ports(module);
        let resets = reset_port_set(&clocks);
        // Yerleşik primitif örnekleri (ADR-0027) — sembol ön geçişi gibi
        // deyimlerden ÖNCE toplanır; alan erişimi çevirisi buna bakar.
        self.collect_builtin_insts(module, &clocks);
        for cfg in &resets {
            self.symbols.insert(
                cfg.port_name().to_string(),
                Sig {
                    width: 1,
                    signed: false,
                },
            );
        }

        let ports_block = self.emit_ports(module, &resets);
        let mut body_chunks = self.emit_body(module, &clocks);

        // F4a — kontratlardan SVA üretimi (moda göre gömülü ya da ayrı).
        match self.sva_mode {
            SvaMode::None => {}
            SvaMode::Inline => {
                if let Some(block) = self.sva_properties(module, &clocks, 4) {
                    body_chunks.push(block);
                }
            }
            SvaMode::Immediate => {
                if let Some(block) = self.sva_immediate(module, &clocks, 4) {
                    body_chunks.push(block);
                }
            }
            SvaMode::Separate => {
                if let Some(file) = self.sva_file(module, &clocks) {
                    self.sva_files.push(file);
                }
            }
        }

        let mut out = String::new();
        if let Some(doc) = doc {
            for line in doc.lines() {
                out.push_str(&format!("// {line}\n"));
            }
        }
        out.push_str(&format!(
            "module {} (\n{}\n);\n",
            module.name.text, ports_block
        ));
        if body_chunks.is_empty() {
            out.push_str("endmodule\n");
        } else {
            out.push('\n');
            out.push_str(&body_chunks.join("\n\n"));
            out.push_str("\n\nendmodule\n");
        }
        out
    }

    /// Port sırası (§1): clock'lar → reset'ler → in → inout → out.
    fn emit_ports(&mut self, module: &'a ModuleDecl, resets: &[ResetCfg]) -> String {
        let ast = self.ast;
        let is_clock = |p: &volt_ast::Port| matches!(ast.types[p.ty].kind, TypeRefKind::Clock);

        let mut lines: Vec<(&'static str, String, String)> = Vec::new();
        for port in module.ports.iter().filter(|p| is_clock(p)) {
            lines.push(("input", "logic".into(), port.name.text.clone()));
        }
        for cfg in resets {
            lines.push(("input", "logic".into(), cfg.port_name().into()));
        }
        for pass in [PortDir::In, PortDir::InOut, PortDir::Out] {
            for port in &module.ports {
                if port.direction != pass || is_clock(port) {
                    continue;
                }
                let ty = self.sv_type_string(port.ty, port.span);
                let dir = match pass {
                    PortDir::In => "input",
                    PortDir::InOut => "inout",
                    PortDir::Out => "output",
                };
                lines.push((dir, ty, port.name.text.clone()));
            }
        }

        let ty_width = lines.iter().map(|(_, ty, _)| ty.len()).max().unwrap_or(5);
        let count = lines.len();
        lines
            .iter()
            .enumerate()
            .map(|(i, (dir, ty, name))| {
                let comma = if i + 1 < count { "," } else { "" };
                format!("    {dir:<6} {ty:<ty_width$} {name}{comma}")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Gövde: ardışık aynı-tür tek satırlık bildirimler tek chunk'ta
    /// gruplanır; chunk'lar boş satırla ayrılır.
    fn emit_body(&mut self, module: &'a ModuleDecl, clocks: &[ClockPort]) -> Vec<String> {
        #[derive(PartialEq, Clone, Copy)]
        enum Kind {
            Decl,
            Always,
            Assign,
        }
        let ast = self.ast;
        let mut chunks: Vec<(Kind, String)> = Vec::new();

        for &stmt_idx in &module.body {
            let stmt = &ast.stmts[stmt_idx];
            let entry = match &stmt.kind {
                // Sembolde yoksa E2012 zaten üretildi
                StmtKind::Reg(reg) => self.symbols.get(&reg.name.text).copied().map(|sig| {
                    (
                        Kind::Decl,
                        format!("    {} {};", sig.decl_type(), reg.name.text),
                    )
                }),
                StmtKind::Let(decl) => {
                    let sig = self
                        .width_of(decl.value)
                        .or_else(|| decl.ty.and_then(|t| self.sig_of_typeref(t, stmt.span)));
                    match sig {
                        Some(sig) => {
                            self.symbols.insert(decl.name.text.clone(), sig);
                            let value = self.emit_expr(decl.value, Some(sig));
                            Some((
                                Kind::Decl,
                                format!(
                                    "    wire {}{} = {};",
                                    sig.wire_prefix(),
                                    decl.name.text,
                                    value
                                ),
                            ))
                        }
                        None => {
                            self.error(
                                ErrorCode::E2005,
                                lstr!(
                                    en: "cannot determine the width of '{}'", decl.name.text;
                                    tr: "'{}' genişliği belirlenemiyor", decl.name.text
                                ),
                                stmt.span,
                                &lstr!(
                                    en: "write an explicit type on the let binding: let x : u8 = ...";
                                    tr: "let bağlamasına açık tip yazın: let x : u8 = ..."
                                ),
                            );
                            None
                        }
                    }
                }
                StmtKind::On(on) => {
                    let info = domain_of_trigger(clocks, on);
                    let reset = if clocks.is_empty() || info.reset.is_none() {
                        None
                    } else {
                        Some(info.reset)
                    };
                    Some((
                        Kind::Always,
                        self.emit_on_block(module, on, info, reset, stmt.span),
                    ))
                }
                StmtKind::Assign(assign) => {
                    match self.try_emit_sync_bridge(module, clocks, assign, stmt.span) {
                        Some(chunk) => Some((Kind::Always, chunk)),
                        None => {
                            let lhs_sig = self.lvalue_sig(&assign.lhs);
                            let lhs = self.emit_lvalue(&assign.lhs);
                            let rhs = self.emit_expr(assign.rhs, lhs_sig);
                            Some((Kind::Assign, format!("    assign {lhs} = {rhs};")))
                        }
                    }
                }
                StmtKind::Expr(_) | StmtKind::Error => None, // parse tanısı zaten var
                // F1 parser yapıları — SV üretimi sonraki aşamalarda
                StmtKind::Wire(w) => {
                    self.future(
                        stmt.span,
                        &lstr!(
                            en: "SV generation of 'wire {}'", w.name.text;
                            tr: "'wire {}' SV üretimi", w.name.text
                        ),
                    );
                    None
                }
                StmtKind::Instance(inst) => {
                    let is_builtin = inst.module_path.segments.len() == 1
                        && BuiltinPrim::from_name(&inst.module_path.segments[0].text).is_some();
                    if is_builtin {
                        // Ön geçiş doğrulayamadıysa tanı üretildi — boş chunk.
                        self.emit_builtin_instance(&module.name.text, &inst.name.text, stmt.span)
                            .map(|chunk| (Kind::Always, chunk))
                    } else {
                        self.future(
                            stmt.span,
                            &lstr!(
                                en: "SV generation of module instance '{}'", inst.name.text;
                                tr: "'{}' modül örneklemesinin SV üretimi", inst.name.text
                            ),
                        );
                        None
                    }
                }
                StmtKind::Comb(_) => {
                    self.future(
                        stmt.span,
                        &lstr!(
                            en: "SV generation of the 'comb' block";
                            tr: "'comb' bloğunun SV üretimi"
                        ),
                    );
                    None
                }
                StmtKind::For(_) => {
                    self.future(
                        stmt.span,
                        &lstr!(
                            en: "SV generation of the 'for' generate loop";
                            tr: "'for' generate döngüsünün SV üretimi"
                        ),
                    );
                    None
                }
            };

            if let Some((kind, text)) = entry {
                match chunks.last_mut() {
                    Some((last_kind, chunk)) if *last_kind == kind && kind != Kind::Always => {
                        chunk.push('\n');
                        chunk.push_str(&text);
                    }
                    _ => chunks.push((kind, text)),
                }
            }
        }
        chunks
            .into_iter()
            .map(|(_, text)| text)
            .filter(|text| !text.is_empty())
            .collect()
    }

    /// sv-mapping.md §8 — `dest = sync(src, dst_clk)` / `sync3(...)` köprüsü.
    ///
    /// Kaynak alanda bir yakalama register'ı, hedef alanda N aşama üretir;
    /// yakalama, senkronizatöre kombinasyonel yol girmesini engeller ve
    /// kaynak saat portunu üretilen SV'de kullanılır kılar. RHS sync
    /// çağrısı değilse None döner (normal assign yolu); çağrı desteklenen
    /// biçimde değilse tanı üretilir ve boş chunk döner.
    fn try_emit_sync_bridge(
        &mut self,
        module: &'a ModuleDecl,
        clocks: &[ClockPort],
        assign: &'a AssignStmt,
        span: Span,
    ) -> Option<String> {
        let ast = self.ast;
        let ExprKind::Call { callee, args } = &ast.exprs[assign.rhs].kind else {
            return None;
        };
        let stages: usize = match path_single(ast, *callee) {
            Some("sync") => 2,
            Some("sync3") => 3,
            _ => return None,
        };

        if !assign.lhs.suffixes.is_empty() {
            self.future(
                span,
                &lstr!(
                    en: "sync() into an indexed or sliced target";
                    tr: "indeksli/dilimli hedefe sync()"
                ),
            );
            return Some(String::new());
        }
        let dest = assign.lhs.base.text.clone();

        if args.len() != 2 {
            self.future(
                span,
                &lstr!(
                    en: "sync() with {} argument(s) — expected sync(src, dst_clock)", args.len();
                    tr: "{} argümanlı sync() — beklenen sync(kaynak, hedef_saat)", args.len()
                ),
            );
            return Some(String::new());
        }
        let (src_arg, clk_arg) = (args[0], args[1]);
        let Some(src) = path_single(ast, src_arg).map(str::to_owned) else {
            self.future(
                span,
                &lstr!(
                    en: "sync() with a compound source expression — bind it with let first";
                    tr: "bileşik kaynak ifadeli sync() — önce let ile bağlayın"
                ),
            );
            return Some(String::new());
        };
        let Some(dst_clk) = path_single(ast, clk_arg).map(str::to_owned) else {
            self.future(
                span,
                &lstr!(
                    en: "sync() whose clock argument is not a simple clock port name";
                    tr: "saat argümanı basit bir saat portu adı olmayan sync()"
                ),
            );
            return Some(String::new());
        };
        let Some(dst) = clocks.iter().find(|c| c.name == dst_clk).cloned() else {
            self.future(
                span,
                &lstr!(
                    en: "sync() whose clock argument '{dst_clk}' is not a clock port of this module";
                    tr: "saat argümanı '{dst_clk}' bu modülün saat portu olmayan sync()"
                ),
            );
            return Some(String::new());
        };

        let sig = self
            .symbols
            .get(&src)
            .copied()
            .or_else(|| self.symbols.get(&dest).copied());
        let Some(sig) = sig else {
            self.error(
                ErrorCode::E2005,
                lstr!(
                    en: "cannot determine the width of the sync() source '{src}'";
                    tr: "sync() kaynağı '{src}' genişliği belirlenemiyor"
                ),
                span,
                &lstr!(
                    en: "declare '{src}' as a port or register with an explicit type";
                    tr: "'{src}' portunu/register'ını açık tiple bildirin"
                ),
            );
            return Some(String::new());
        };

        // Kaynağın alanı → o alanın saat portu (yakalama aşaması için)
        let src_clock = module
            .ports
            .iter()
            .find(|p| p.name.text == src)
            .and_then(|p| p.domain.as_ref())
            .and_then(|d| clocks.iter().find(|c| c.domain.as_deref() == Some(&d.text)))
            .filter(|c| c.name != dst.name)
            .cloned();

        let base = format!("sync_{src}");
        let ty = sig.decl_type();
        let zero = zero_of(sig);
        let src_label = src_clock.as_ref().map_or(src.as_str(), |c| c.name.as_str());

        let mut out = format!("    // CDC synchronizer: {src_label} -> {}\n", dst.name);
        if src_clock.is_some() {
            out.push_str(&format!("    {ty} {base}_src;\n"));
        }
        for i in 0..stages {
            out.push_str(&format!("    {ty} {base}_stage{i};\n"));
        }
        out.push('\n');

        if let Some(cap) = &src_clock {
            out.push_str(&sync_always_ff(
                &cap.name,
                cap.info,
                &[(format!("{base}_src"), src.clone())],
                &zero,
            ));
            out.push_str("\n\n");
        }

        let mut prev = match src_clock {
            Some(_) => format!("{base}_src"),
            None => src,
        };
        let mut chain = Vec::with_capacity(stages);
        for i in 0..stages {
            let cur = format!("{base}_stage{i}");
            chain.push((cur.clone(), prev));
            prev = cur;
        }
        out.push_str(&sync_always_ff(&dst.name, dst.info, &chain, &zero));
        out.push_str("\n\n");
        out.push_str(&format!("    assign {dest} = {prev};"));
        Some(out)
    }

    /// sv-mapping.md §4: always_ff + otomatik reset bloğu.
    fn emit_on_block(
        &mut self,
        module: &'a ModuleDecl,
        on: &'a OnBlock,
        domain: DomainInfo,
        reset: Option<ResetCfg>,
        span: Span,
    ) -> String {
        let clk = match &on.trigger {
            OnTrigger::Clock(name) => name.text.clone(),
            OnTrigger::Reset(name) => {
                self.future(
                    span,
                    &lstr!(
                        en: "the 'on clock.reset' block";
                        tr: "'on saat.reset' blokları"
                    ),
                );
                name.text.clone()
            }
            OnTrigger::Error => "clk".to_string(),
        };
        let edge = match domain.edge {
            ClockEdge::Negedge => "negedge",
            _ => "posedge",
        };

        let mut out = String::new();
        match reset {
            Some(cfg) => {
                out.push_str(&format!(
                    "    always_ff @({edge} {clk}{}) begin\n",
                    cfg.async_sensitivity()
                ));
                out.push_str(&format!("        if ({}) begin\n", cfg.condition()));
                for line in self.reset_assignments(module, on) {
                    out.push_str(&format!("            {line}\n"));
                }
                out.push_str("        end else begin\n");
                for line in self.emit_block(on.body, 12) {
                    out.push_str(&line);
                    out.push('\n');
                }
                out.push_str("        end\n    end");
            }
            None => {
                out.push_str(&format!("    always_ff @({edge} {clk}) begin\n"));
                for line in self.emit_block(on.body, 8) {
                    out.push_str(&line);
                    out.push('\n');
                }
                out.push_str("    end");
            }
        }
        out
    }

    /// Bu on-bloğunda yazılan reg'ler, bildirim sırasıyla `reg <= init;`.
    fn reset_assignments(&mut self, module: &'a ModuleDecl, on: &'a OnBlock) -> Vec<String> {
        let ast = self.ast;
        let mut written = Vec::new();
        collect_written(ast, &ast.blocks[on.body], &mut written);

        let mut lines = Vec::new();
        for &stmt_idx in &module.body {
            if let StmtKind::Reg(reg) = &ast.stmts[stmt_idx].kind {
                if written.iter().any(|w| w == &reg.name.text) {
                    let sig = self.symbols.get(&reg.name.text).copied();
                    let init = self.emit_expr(reg.init, sig);
                    lines.push(format!("{} <= {};", reg.name.text, init));
                }
            }
        }
        lines
    }

    fn emit_block(&mut self, block: Idx<Block>, indent: usize) -> Vec<String> {
        let ast = self.ast;
        let ind = " ".repeat(indent);
        let mut lines = Vec::new();
        for stmt in &ast.blocks[block].stmts {
            match stmt {
                BlockStmt::NonBlockAssign { lhs, rhs, .. } => {
                    let sig = self.lvalue_sig(lhs);
                    let lhs_s = self.emit_lvalue(lhs);
                    let rhs_s = self.emit_expr(*rhs, sig);
                    lines.push(format!("{ind}{lhs_s} <= {rhs_s};"));
                }
                BlockStmt::BlockAssign { lhs, rhs, .. } => {
                    let sig = self.lvalue_sig(lhs);
                    let lhs_s = self.emit_lvalue(lhs);
                    let rhs_s = self.emit_expr(*rhs, sig);
                    lines.push(format!("{ind}{lhs_s} = {rhs_s};"));
                }
                BlockStmt::If(if_stmt) => self.emit_if(if_stmt, indent, &mut lines),
                BlockStmt::Let(decl) => {
                    let span = ast.blocks[block].span;
                    self.future(
                        span,
                        &lstr!(
                            en: "'let {}' inside a block", decl.name.text;
                            tr: "blok içi 'let {}'", decl.name.text
                        ),
                    );
                }
                BlockStmt::Error => {}
                // F1 parser yapıları — SV üretimi sonraki aşamalarda
                BlockStmt::Match(m) => self.future(
                    m.span,
                    &lstr!(
                        en: "SV generation of the 'match' statement";
                        tr: "'match' deyiminin SV üretimi"
                    ),
                ),
                BlockStmt::For(f) => {
                    let span = ast.blocks[f.body].span;
                    self.future(
                        span,
                        &lstr!(
                            en: "SV generation of the 'for' generate loop";
                            tr: "'for' generate döngüsünün SV üretimi"
                        ),
                    );
                }
            }
        }
        lines
    }

    fn emit_if(&mut self, if_stmt: &'a IfStmt, indent: usize, lines: &mut Vec<String>) {
        let ind = " ".repeat(indent);
        let one_bit = Some(Sig {
            width: 1,
            signed: false,
        });
        let cond = self.emit_expr(if_stmt.cond, one_bit);
        lines.push(format!("{ind}if ({cond}) begin"));
        lines.extend(self.emit_block(if_stmt.then_block, indent + 4));
        let mut current = if_stmt.else_branch.as_ref();
        loop {
            match current {
                None => {
                    lines.push(format!("{ind}end"));
                    break;
                }
                Some(ElseBranch::Block(block)) => {
                    lines.push(format!("{ind}end else begin"));
                    lines.extend(self.emit_block(*block, indent + 4));
                    lines.push(format!("{ind}end"));
                    break;
                }
                Some(ElseBranch::If(elif)) => {
                    let cond = self.emit_expr(elif.cond, one_bit);
                    lines.push(format!("{ind}end else if ({cond}) begin"));
                    lines.extend(self.emit_block(elif.then_block, indent + 4));
                    current = elif.else_branch.as_ref();
                }
            }
        }
    }

    // ═══ LValue ═══════════════════════════════════════════════════

    fn emit_lvalue(&mut self, lv: &'a LValue) -> String {
        // Yerleşik primitif alanı hedefte: `f.rd_data` → `f_rd_data`.
        if let [LValueSuffix::Field(f)] = lv.suffixes.as_slice() {
            if self.builtin_insts.contains_key(&lv.base.text) {
                return format!("{}_{}", lv.base.text, f.text);
            }
        }
        let mut out = lv.base.text.clone();
        for suffix in &lv.suffixes {
            match suffix {
                LValueSuffix::Index(i) => {
                    let i = self.emit_plain(*i);
                    out.push_str(&format!("[{i}]"));
                }
                LValueSuffix::Range { hi, lo } => {
                    let hi = self.emit_plain(*hi);
                    let lo = self.emit_plain(*lo);
                    out.push_str(&format!("[{hi}:{lo}]"));
                }
                LValueSuffix::Field(name) => out.push_str(&format!(".{}", name.text)),
            }
        }
        out
    }

    fn lvalue_sig(&mut self, lv: &LValue) -> Option<Sig> {
        let mut sig = self.symbols.get(&lv.base.text).copied();
        for suffix in &lv.suffixes {
            sig = match suffix {
                LValueSuffix::Index(_) => Some(Sig {
                    width: 1,
                    signed: false,
                }),
                LValueSuffix::Range { hi, lo } => {
                    let (hi, lo) = (self.eval_const(*hi)?, self.eval_const(*lo)?);
                    Some(Sig {
                        width: (hi.saturating_sub(lo) + 1) as u32,
                        signed: false,
                    })
                }
                LValueSuffix::Field(_) => None,
            };
        }
        sig
    }

    // ═══ Tipler (§2) ══════════════════════════════════════════════

    pub(crate) fn sig_of_typeref(&mut self, ty: Idx<TypeRef>, span: Span) -> Option<Sig> {
        match &self.ast.types[ty].kind {
            TypeRefKind::Bool | TypeRefKind::Clock => Some(Sig {
                width: 1,
                signed: false,
            }),
            TypeRefKind::UInt(n) => Some(Sig {
                width: *n as u32,
                signed: false,
            }),
            TypeRefKind::SInt(n) => Some(Sig {
                width: *n as u32,
                signed: true,
            }),
            TypeRefKind::Bits(e) => {
                let e = *e;
                match self.eval_const(e) {
                    Some(n) if n >= 1 => Some(Sig {
                        width: n as u32,
                        signed: false,
                    }),
                    _ => {
                        self.error(
                            ErrorCode::E2005,
                            lstr!(
                                en: "the width of bits<N> cannot be determined at compile time";
                                tr: "bits<N> genişliği derleme zamanında belirlenemiyor"
                            ),
                            span,
                            &lstr!(
                                en: "N must be a constant expression (e.g. bits<8>)";
                                tr: "N sabit bir ifade olmalı (ör. bits<8>)"
                            ),
                        );
                        None
                    }
                }
            }
            TypeRefKind::Trit => {
                self.future(
                    span,
                    &lstr!(
                        en: "SV mapping of the 'Trit' type";
                        tr: "'Trit' tipinin SV eşlemesi"
                    ),
                );
                None
            }
            TypeRefKind::Reset(_) => {
                self.future(
                    span,
                    &lstr!(
                        en: "the explicit 'reset' port";
                        tr: "açık 'reset' portları"
                    ),
                );
                None
            }
            TypeRefKind::Error => None, // parse tanısı zaten var
            // F1 parser tipleri — SV eşlemesi sonraki aşamalarda
            TypeRefKind::Array { .. } | TypeRefKind::Tuple(_) => {
                self.future(
                    span,
                    &lstr!(
                        en: "SV mapping of array/tuple types";
                        tr: "dizi/tuple tiplerinin SV eşlemesi"
                    ),
                );
                None
            }
            TypeRefKind::Path { .. } => {
                self.future(
                    span,
                    &lstr!(
                        en: "SV mapping of user-defined types";
                        tr: "kullanıcı tanımlı tiplerin SV eşlemesi"
                    ),
                );
                None
            }
        }
    }

    fn sv_type_string(&mut self, ty: Idx<TypeRef>, span: Span) -> String {
        match self.sig_of_typeref(ty, span) {
            Some(sig) => sig.decl_type(),
            None => "logic".to_string(),
        }
    }
}

fn collect_written(ast: &SourceFile, block: &Block, out: &mut Vec<String>) {
    for stmt in &block.stmts {
        match stmt {
            BlockStmt::NonBlockAssign { lhs, .. } | BlockStmt::BlockAssign { lhs, .. }
                if !out.contains(&lhs.base.text) =>
            {
                out.push(lhs.base.text.clone());
            }
            BlockStmt::If(if_stmt) => collect_written_if(ast, if_stmt, out),
            _ => {}
        }
    }
}

fn collect_written_if(ast: &SourceFile, if_stmt: &IfStmt, out: &mut Vec<String>) {
    collect_written(ast, &ast.blocks[if_stmt.then_block], out);
    match &if_stmt.else_branch {
        Some(ElseBranch::Block(b)) => collect_written(ast, &ast.blocks[*b], out),
        Some(ElseBranch::If(elif)) => collect_written_if(ast, elif, out),
        None => {}
    }
}
