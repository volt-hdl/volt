//! Volt AST → SystemVerilog üretimi (F0, string template).
//!
//! Bağlayıcı referans: docs/spec/sv-mapping.md. CIRCT yok — F3'te
//! volt-lower devralacak. Tip bilgisi kaba çıkarımla gelir (F2'de HIR
//! düzeltecek); belirsizlikte E2005 üretilir, tahmin edilmez.

mod expr;

use std::collections::HashMap;

use volt_ast::{
    Block, BlockStmt, ClockEdge, DomainKey, DomainValue, ElseBranch, Idx, IfStmt, ItemKind, LValue,
    LValueSuffix, ModuleDecl, OnBlock, OnTrigger, Port, PortDir, ResetPolarity, ResetSync,
    SourceFile, StmtKind, TypeRef, TypeRefKind,
};
use volt_diagnostics::{Diagnostic, ErrorCode, LabeledSpan, Severity};
use volt_span::Span;

pub use expr::Sig;

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
struct ResetCfg {
    sync: ResetSync, // None → reset yok
    polarity: ResetPolarity,
}

impl ResetCfg {
    const DEFAULT: ResetCfg = ResetCfg {
        sync: ResetSync::Sync,
        polarity: ResetPolarity::ActiveHigh,
    };

    fn is_none(&self) -> bool {
        self.sync == ResetSync::None
    }

    fn port_name(&self) -> &'static str {
        match self.polarity {
            ResetPolarity::ActiveHigh => "rst",
            ResetPolarity::ActiveLow => "rst_n",
        }
    }

    fn condition(&self) -> &'static str {
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
struct DomainInfo {
    edge: ClockEdge,
    reset: ResetCfg,
}

const DEFAULT_DOMAIN: DomainInfo = DomainInfo {
    edge: ClockEdge::Posedge,
    reset: ResetCfg::DEFAULT,
};

/// Dosyadaki tüm modülleri tek SV dosyasına üretir.
pub fn emit(ast: &SourceFile, source_name: &str) -> EmitResult {
    let mut emitter = Emitter {
        ast,
        diagnostics: Vec::new(),
        domains: collect_domains(ast),
        symbols: HashMap::new(),
    };

    let mut modules = Vec::new();
    for &item_idx in &ast.items {
        let item = &ast.items_arena[item_idx];
        if let ItemKind::Module(module) = &item.kind {
            modules.push(emitter.emit_module(module, item.doc.as_deref()));
        }
    }

    let mut sv = header(source_name);
    sv.push('\n');
    sv.push_str(&modules.join("\n"));
    sv.push('\n');
    sv.push_str("`default_nettype wire\n");

    EmitResult {
        sv,
        diagnostics: emitter.diagnostics,
    }
}

/// sv-mapping.md §12 — deterministik başlık (tarih yok).
fn header(source_name: &str) -> String {
    format!(
        "// Bu dosya Volt tarafından otomatik üretilmiştir.\n\
         // Kaynak: {source_name}\n\
         // Volt sürümü: {VOLT_VERSION}\n\
         //\n\
         // DÜZENLEMEYİN — değişiklikler kaynak dosyada yapılmalıdır.\n\
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
            format!("{what} F0 SV üretiminde desteklenmiyor"),
            span,
            "bu yapı F1+ sürümünde eklenecek",
        );
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
                        format!("'{}' register tipi belirlenemiyor", reg.name.text),
                        span,
                        "F0'da reg tipi açık yazılmalı: reg isim : u8 = 0",
                    ),
                }
            }
        }

        // Saat portları ve reset yapılandırması (§7)
        let clock_ports: Vec<&Port> = module
            .ports
            .iter()
            .filter(|p| matches!(ast.types[p.ty].kind, TypeRefKind::Clock))
            .collect();
        let domain_info = clock_ports
            .first()
            .and_then(|p| p.domain.as_ref())
            .and_then(|d| self.domains.get(&d.text).copied())
            .unwrap_or(DEFAULT_DOMAIN);
        let reset = if clock_ports.is_empty() || domain_info.reset.is_none() {
            None
        } else {
            Some(domain_info.reset)
        };
        if let Some(cfg) = reset {
            self.symbols.insert(
                cfg.port_name().to_string(),
                Sig {
                    width: 1,
                    signed: false,
                },
            );
        }

        let ports_block = self.emit_ports(module, reset);
        let body_chunks = self.emit_body(module, domain_info, reset);

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

    /// Port sırası (§1): clock → reset → in → inout → out.
    fn emit_ports(&mut self, module: &'a ModuleDecl, reset: Option<ResetCfg>) -> String {
        let ast = self.ast;
        let is_clock = |p: &Port| matches!(ast.types[p.ty].kind, TypeRefKind::Clock);

        let mut lines: Vec<(&'static str, String, String)> = Vec::new();
        for port in module.ports.iter().filter(|p| is_clock(p)) {
            lines.push(("input", "logic".into(), port.name.text.clone()));
        }
        if let Some(cfg) = reset {
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
    fn emit_body(
        &mut self,
        module: &'a ModuleDecl,
        domain: DomainInfo,
        reset: Option<ResetCfg>,
    ) -> Vec<String> {
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
                                format!("'{}' genişliği belirlenemiyor", decl.name.text),
                                stmt.span,
                                "let bağlamasına açık tip yazın: let x : u8 = ...",
                            );
                            None
                        }
                    }
                }
                StmtKind::On(on) => Some((
                    Kind::Always,
                    self.emit_on_block(module, on, domain, reset, stmt.span),
                )),
                StmtKind::Assign(assign) => {
                    let lhs_sig = self.lvalue_sig(&assign.lhs);
                    let lhs = self.emit_lvalue(&assign.lhs);
                    let rhs = self.emit_expr(assign.rhs, lhs_sig);
                    Some((Kind::Assign, format!("    assign {lhs} = {rhs};")))
                }
                StmtKind::Expr(_) | StmtKind::Error => None, // parse tanısı zaten var
                // F1 parser yapıları — SV üretimi sonraki aşamalarda
                StmtKind::Wire(w) => {
                    self.future(stmt.span, &format!("'wire {}' SV üretimi", w.name.text));
                    None
                }
                StmtKind::Instance(inst) => {
                    self.future(
                        stmt.span,
                        &format!("'{}' modül örneklemesinin SV üretimi", inst.name.text),
                    );
                    None
                }
                StmtKind::Comb(_) => {
                    self.future(stmt.span, "'comb' bloğunun SV üretimi");
                    None
                }
                StmtKind::For(_) => {
                    self.future(stmt.span, "'for' generate döngüsünün SV üretimi");
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
        chunks.into_iter().map(|(_, text)| text).collect()
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
                self.future(span, "'on saat.reset' blokları");
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
                    self.future(span, &format!("blok içi 'let {}'", decl.name.text));
                }
                BlockStmt::Error => {}
                // F1 parser yapıları — SV üretimi sonraki aşamalarda
                BlockStmt::Match(m) => self.future(m.span, "'match' deyiminin SV üretimi"),
                BlockStmt::For(f) => {
                    let span = ast.blocks[f.body].span;
                    self.future(span, "'for' generate döngüsünün SV üretimi");
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
                            "bits<N> genişliği derleme zamanında belirlenemiyor".into(),
                            span,
                            "N sabit bir ifade olmalı (ör. bits<8>)",
                        );
                        None
                    }
                }
            }
            TypeRefKind::Trit => {
                self.future(span, "'Trit' tipinin SV eşlemesi");
                None
            }
            TypeRefKind::Reset(_) => {
                self.future(span, "açık 'reset' portları");
                None
            }
            TypeRefKind::Error => None, // parse tanısı zaten var
            // F1 parser tipleri — SV eşlemesi sonraki aşamalarda
            TypeRefKind::Array { .. } | TypeRefKind::Tuple(_) => {
                self.future(span, "dizi/tuple tiplerinin SV eşlemesi");
                None
            }
            TypeRefKind::Path { .. } => {
                self.future(span, "kullanıcı tanımlı tiplerin SV eşlemesi");
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
