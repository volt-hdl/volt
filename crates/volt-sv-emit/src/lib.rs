//! Volt AST → SystemVerilog üretimi (F0, string template).
//!
//! Bağlayıcı referans: docs/spec/sv-mapping.md. CIRCT yok — F3'te
//! volt-lower devralacak. Tip bilgisi kaba çıkarımla gelir (F2'de HIR
//! düzeltecek); belirsizlikte E2005 üretilir, tahmin edilmez.

mod alias;
mod builtin_prim;
mod const_array;
mod enums;
mod expr;
mod generate;
mod inline;
mod instance;
mod past;
mod reset_sync;
mod sby;
pub mod sim;
mod sim_contract;
mod sim_script;
mod structs;
mod sv_names;
mod sva;
mod trit;

use std::collections::{HashMap, HashSet};

use volt_ast::builtin::BuiltinPrim;
use volt_ast::{
    ArrayLitKind, AssignStmt, Block, BlockStmt, ClockEdge, DomainKey, DomainValue, ElseBranch,
    Expr, ExprKind, Idx, IfStmt, ItemKind, LValue, LValueSuffix, MatchArmBody, MatchStmt,
    ModuleDecl, OnBlock, OnTrigger, Pattern, PatternKind, PortDir, ResetPolarity, ResetSync,
    SourceFile, StmtKind, TypeRef, TypeRefKind,
};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, Severity};
use volt_span::{FileId, Span};

pub use const_array::ConstArrayStyle;
pub use expr::Sig;
pub use sby::{sby_config, sby_config_tasks, SbyEngine, SbyMode, SbyOptions, SbyTask};
pub use sim::{
    collect_sim_ports, find_module, load_config_vlt, run_testbench_cpp, run_testbench_cpp_with,
    test_testbench_cpp, test_testbench_cpp_with, SimPort, SimReset, TbAssertKind, TbPortCheck,
    TbStep, TbTest, TbValue,
};
pub use sim_contract::uses_sim_contracts;
pub use sva::{AutoProp, SvaFile, SvaMode, SvaProp};

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResetCfg {
    pub(crate) sync: ResetSync, // None → reset yok
    pub(crate) polarity: ResetPolarity,
    /// Ham reset portundan üretilen bırakma senkronizörünün çıkışı
    /// (ADR-0065 §2, `rst_sync_<saat>_stage1`); `None` → otomatik port.
    pub(crate) synced: Option<String>,
}

impl ResetCfg {
    const DEFAULT: ResetCfg = ResetCfg {
        sync: ResetSync::Sync,
        polarity: ResetPolarity::ActiveHigh,
        synced: None,
    };

    pub(crate) fn is_none(&self) -> bool {
        self.sync == ResetSync::None
    }

    /// Otomatik reset portunun adı (polariteye göre).
    pub(crate) fn port_name(&self) -> &'static str {
        match self.polarity {
            ResetPolarity::ActiveHigh => "rst",
            ResetPolarity::ActiveLow => "rst_n",
        }
    }

    /// Alanın register'larını sıfırlayan sinyal: zincir çıkışı ya da port.
    pub(crate) fn signal(&self) -> &str {
        self.synced.as_deref().unwrap_or(self.port_name())
    }

    pub(crate) fn condition(&self) -> String {
        match self.polarity {
            ResetPolarity::ActiveHigh => self.signal().to_string(),
            ResetPolarity::ActiveLow => format!("!{}", self.signal()),
        }
    }

    fn async_sensitivity(&self) -> String {
        match (self.sync, self.polarity) {
            (ResetSync::Async, ResetPolarity::ActiveHigh) => {
                format!(" or posedge {}", self.signal())
            }
            (ResetSync::Async, ResetPolarity::ActiveLow) => {
                format!(" or negedge {}", self.signal())
            }
            _ => String::new(),
        }
    }
}

#[derive(Debug, Clone)]
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
    /// Alanını besleyen ham reset portu (ADR-0065 §1); varsa
    /// `info.reset.synced` bu saatin zincir çıkışıdır.
    pub(crate) raw_reset: Option<reset_sync::RawReset>,
}

/// Benzersiz reset portları, saat portu sırası korunarak (ada göre teklenir).
fn reset_port_set(clocks: &[ClockPort]) -> Vec<ResetCfg> {
    let mut out: Vec<ResetCfg> = Vec::new();
    for clock in clocks {
        let cfg = &clock.info.reset;
        // Ham portla beslenen alan otomatik port almaz (ADR-0065 §1).
        if cfg.is_none() || cfg.synced.is_some() {
            continue;
        }
        if !out.iter().any(|c| c.port_name() == cfg.port_name()) {
            out.push(cfg.clone());
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
        .map(|c| c.info.clone())
        .unwrap_or(DEFAULT_DOMAIN)
}

/// Tek segmentli Path ifadesinin metni.
fn path_single(ast: &SourceFile, idx: Idx<Expr>) -> Option<&str> {
    match &ast.exprs[idx].kind {
        ExprKind::Path(p) if p.segments.len() == 1 => Some(p.segments[0].text.as_str()),
        _ => None,
    }
}

/// `text` içinde `name` tanımlayıcısının tam sözcük geçiş sayısı.
fn count_ident(text: &str, name: &str) -> usize {
    let is_ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    text.match_indices(name)
        .filter(|&(i, _)| {
            let before = text[..i].chars().next_back();
            let after = text[i + name.len()..].chars().next();
            !before.is_some_and(is_ident) && !after.is_some_and(is_ident)
        })
        .count()
}

/// Kullanılmayan otomatik reset portlarının satırı Verilator
/// `UNUSEDSIGNAL` susturmasıyla sarılır (ADR-0076); port kalır.
fn silence_unused_resets(ports_block: String, unused: &[&str]) -> String {
    if unused.is_empty() {
        return ports_block;
    }
    let mut out: Vec<String> = Vec::new();
    for line in ports_block.lines() {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        let is_unused = matches!(tokens.as_slice(), ["input", "logic", name]
            if unused.contains(&name.trim_end_matches(',')));
        if is_unused {
            out.push(
                "    // domain reset kept in the interface; no register here uses it".to_string(),
            );
            out.push("    // verilator lint_off UNUSEDSIGNAL".to_string());
            out.push(line.to_string());
            out.push("    // verilator lint_on UNUSEDSIGNAL".to_string());
        } else {
            out.push(line.to_string());
        }
    }
    out.join("\n")
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
fn sync_always_ff(clk: &str, info: &DomainInfo, chain: &[(String, String)], zero: &str) -> String {
    let edge = match info.edge {
        ClockEdge::Negedge => "negedge",
        _ => "posedge",
    };
    let cfg = &info.reset;
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

/// Verilator'ın C++ sözcüğü saydığı port adı (ADR-0078): SV geçerlidir ama
/// üst modülde Verilator portu `__SYM__<ad>` diye adlandırır ve varsayılan
/// açık SYMRSVDWORD uyarısıyla durur. Bildirim susturmayla sarılır.
fn verilator_cpp_word_port(name: &str, decl: String) -> String {
    if !volt_ast::reserved::is_verilator_cpp_word(name) {
        return decl;
    }
    format!(
        "    // C++ word: as a Verilator top-level port this is __SYM__{name}\n    \
         // verilator lint_off SYMRSVDWORD\n\
         {decl}\n    \
         // verilator lint_on SYMRSVDWORD"
    )
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
    /// Tüm modüller tek metinde (kaynak sırası) — `--single-file`,
    /// simülasyon ve formal akışı bunu kullanır.
    pub sv: String,
    /// Modül başına bağımsız SV dosyası içeriği (ADR-0024): dosya adı =
    /// modül adı, DECLFILENAME susar.
    pub modules: Vec<SvModule>,
    /// Ayrı modda kontratlı her modül için bir .sva içeriği.
    pub sva_files: Vec<SvaFile>,
    /// Üretilen her property'nin kimliği (F4b — sby FAIL eşlemesi).
    pub sva_props: Vec<SvaProp>,
    /// İki ve daha çok saat portlu modüller — `.sby` dosyasına
    /// `multiclock on` eklenmesi için (ADR-0027, clk2fflogic akışı).
    pub multiclock_modules: Vec<String>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Tek modülün SV dosyası (ADR-0024).
#[derive(Debug, Clone)]
pub struct SvModule {
    pub name: String,
    pub sv: String,
}

/// Derleme birimindeki bir kaynak dosya — SVA yorumlarındaki
/// `dosya:satır` bilgisi span'in dosyasına göre buradan bulunur.
#[derive(Debug, Clone, Copy)]
pub struct SourceText<'a> {
    pub file: FileId,
    pub name: &'a str,
    pub text: &'a str,
}

/// Birim dosyalarından emit girdisi: `names` bağımlılık sırasıyla (ana
/// dosya SONDA, `LoadedUnit::source_names`); dönen listede ana dosya
/// İLK sıradadır (yedek kaynak). Sürücü ve LSP aynı listeyi kurar.
pub fn unit_source_texts<'a>(
    names: &'a [(FileId, String)],
    map: &'a volt_span::SourceMap,
) -> Vec<SourceText<'a>> {
    let mut sources: Vec<SourceText<'a>> = names
        .iter()
        .map(|(fid, name)| SourceText {
            file: *fid,
            name,
            text: map.source(*fid),
        })
        .collect();
    sources.rotate_right(1);
    sources
}

/// Çıktı üretmeden emit doğrulaması (ADR-0070): `volt check` ve LSP,
/// `volt build`'in varsayılan emit'inin tanılarını aynen görür (SV
/// eşlemesi henüz olmayan yapılar dahil). Üretilen SV atılır.
pub fn validate_unit(
    ast: &SourceFile,
    source_name: &str,
    sources: &[SourceText<'_>],
) -> Vec<Diagnostic> {
    emit_unit(
        ast,
        source_name,
        sources,
        SvaMode::None,
        ConstArrayStyle::default(),
    )
    .diagnostics
}

/// Tüm modülleri üretir; `mode`'a göre kontratlardan SVA da çıkarır.
pub fn emit_full(ast: &SourceFile, source_name: &str, source: &str, mode: SvaMode) -> EmitOutput {
    emit_full_opts(ast, source_name, source, mode, ConstArrayStyle::default())
}

/// `emit_full` + değişken indeksli const dizilerin SV biçimi (ADR-0041).
pub fn emit_full_opts(
    ast: &SourceFile,
    source_name: &str,
    source: &str,
    mode: SvaMode,
    const_array_style: ConstArrayStyle,
) -> EmitOutput {
    let sources = [SourceText {
        file: FileId(0),
        name: source_name,
        text: source,
    }];
    emit_unit(ast, source_name, &sources, mode, const_array_style)
}

/// Çoklu dosya derleme birimi (ADR-0042): `sources` birimdeki tüm
/// dosyalar, `source_name` ana dosyanın adı (birleşik çıktı başlığı).
pub fn emit_unit(
    ast: &SourceFile,
    source_name: &str,
    sources: &[SourceText<'_>],
    mode: SvaMode,
    const_array_style: ConstArrayStyle,
) -> EmitOutput {
    // ADR-0081 Karar 12.4: fn gövdeleri tanımda, çağrı sayısından
    // bağımsız bir kez doğrulanır; doğrulanamayanın çağrıları açılmaz.
    let (invalid_fns, fn_diags) = validate_functions(ast, source_name, sources, const_array_style);
    // ADR-0081: fn çağrıları çağrı yerinde açılır (struct indirgemesinden
    // ÖNCE); çağrı yoksa özgün AST (çıktı byte-aynı).
    let (inlined, inline_diags) = inline::lower(ast, &invalid_fns);
    let no_inline = inline::InlineNotes::default();
    let (ast, inline_notes) = match &inlined {
        Some(l) => (&l.ast, &l.notes),
        None => (ast, &no_inline),
    };
    // ADR-0077: struct tipli sinyaller yaprak sinyallere indirgenir; struct
    // kullanmayan birimde özgün AST (çıktı byte-aynı).
    let (lowered, struct_diags) = structs::lower(ast);
    let no_notes = structs::StructNotes::default();
    let (ast, struct_notes) = match &lowered {
        Some(l) => (&l.ast, &l.notes),
        None => (ast, &no_notes),
    };
    let mut emitter = new_emitter(
        ast,
        source_name,
        sources,
        mode,
        const_array_style,
        struct_notes,
        inline_notes,
    );
    let mut diagnostics = fn_diags;
    diagnostics.extend(inline_diags);
    diagnostics.extend(struct_diags);
    emitter.diagnostics = diagnostics;
    // ADR-0078: SV anahtar sözcüğü olan adlar (E1013) — bütün modüllerin
    // kesin denetimi emit'ten ÖNCE: alt modülün portu üst modülün örnek
    // bağlantısında (`.table(a)`) daha önce görünür, güvenlik ağı onu
    // modül sırasına bağlı olarak ikinci kez bildirmesin.
    emitter.audit_unit_names();

    let mut modules = Vec::new();
    let mut per_module = Vec::new();
    let mut multiclock_modules = Vec::new();
    for &item_idx in &ast.items {
        let item = &ast.items_arena[item_idx];
        if let ItemKind::Module(module) = &item.kind {
            if emitter.collect_clock_ports(module).len() >= 2 {
                multiclock_modules.push(module.name.text.clone());
            }
            let body = emitter.emit_module(module, item.doc.as_deref());
            emitter.audit_emitted_text(module, &body);
            // ADR-0024: her modül kendi dosyasında; başlık o modülün
            // kaynak dosyasını gösterir.
            let origin = emitter.source_name_of(item.span.file);
            let mut sv = header_for(origin, Some(&module.name.text));
            sv.push('\n');
            sv.push_str(&body);
            sv.push('\n');
            sv.push_str("`default_nettype wire\n");
            per_module.push(SvModule {
                name: module.name.text.clone(),
                sv,
            });
            modules.push(body);
        }
    }

    let mut sv = header(source_name);
    sv.push('\n');
    sv.push_str(&modules.join("\n"));
    sv.push('\n');
    sv.push_str("`default_nettype wire\n");

    EmitOutput {
        sv,
        modules: per_module,
        sva_files: emitter.sva_files,
        sva_props: emitter.sva_props,
        multiclock_modules,
        diagnostics: emitter.diagnostics,
    }
}

/// Emitter'ı kurar (asıl emit ve fn doğrulaması aynı kurulumu kullanır).
fn new_emitter<'a>(
    ast: &'a SourceFile,
    source_name: &'a str,
    sources: &'a [SourceText<'a>],
    mode: SvaMode,
    const_array_style: ConstArrayStyle,
    struct_notes: &'a structs::StructNotes,
    inline_notes: &'a inline::InlineNotes,
) -> Emitter<'a> {
    Emitter {
        ast,
        diagnostics: Vec::new(),
        domains: collect_domains(ast),
        symbols: HashMap::new(),
        trits: HashSet::new(),
        array_dims: HashMap::new(),
        packed_arrays: HashMap::new(),
        builtin_insts: HashMap::new(),
        user_insts: HashMap::new(),
        consts: collect_consts(ast),
        const_array_style,
        array_consts_used: Vec::new(),
        loop_vars: Vec::new(),
        pre_decls: Vec::new(),
        bus_wires: HashMap::new(),
        sources,
        source_name,
        sva_mode: mode,
        sva_files: Vec::new(),
        sva_props: Vec::new(),
        past_regs: HashMap::new(),
        sim_dpi: sim_contract::SimDpiUse::default(),
        enum_used: Vec::new(),
        enum_sigs: HashMap::new(),
        struct_notes,
        inline_notes,
        inline_headers_done: HashSet::new(),
        module_name: String::new(),
        sv_name_reported: HashSet::new(),
    }
}

impl Emitter<'_> {
    /// fn açılımının çağrı başlığı (ADR-0081 Karar 12.2): çağrının ilk
    /// telinin üstüne `// <fn>(<argümanlar>) — <dosya>:<satır>`.
    fn inline_header(&mut self, name: &str) -> String {
        let Some(key) = self.inline_notes.header_key(&self.module_name, name) else {
            return String::new();
        };
        if !self.inline_headers_done.insert(key.clone()) {
            return String::new();
        }
        let h = &self.inline_notes.headers[&key];
        let args: Vec<String> = h.args.iter().map(|&a| self.source_snippet(a)).collect();
        let (file, line) = self.location_of(h.call);
        format!(
            "    // {}({}) — {file}:{line}\n",
            h.fn_name,
            args.join(", ")
        )
    }

    /// Span'in kaynak metni, boşlukları tek boşluğa indirgenmiş.
    fn source_snippet(&self, span: Span) -> String {
        let src = self
            .sources
            .iter()
            .find(|s| s.file == span.file)
            .or_else(|| self.sources.first());
        let text = src
            .and_then(|s| s.text.get(span.start as usize..span.end as usize))
            .unwrap_or("…");
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

/// ADR-0081 Karar 12.4: her fn gövdesini, onu ikame kipinde süren
/// sentetik bir modülle bir kez emit eder (metin atılır). Tanımın
/// içine düşen tanılar fn'e aittir; hatalı fn'in çağrıları açılmaz.
fn validate_functions(
    ast: &SourceFile,
    source_name: &str,
    sources: &[SourceText<'_>],
    const_array_style: ConstArrayStyle,
) -> (HashSet<String>, Vec<Diagnostic>) {
    let mut invalid = HashSet::new();
    let mut diags = Vec::new();
    let Some((unit, fns)) = inline::validation_unit(ast) else {
        return (invalid, diags);
    };
    // Birimde çağrı kalmadı (iç çağrılar port): açılım notu yok.
    let inline_notes = inline::InlineNotes::default();
    // Struct bildirimi tanıları asıl geçişte raporlanır.
    let (lowered, _) = structs::lower(&unit);
    let no_notes = structs::StructNotes::default();
    let (unit, struct_notes) = match &lowered {
        Some(l) => (&l.ast, &l.notes),
        None => (&unit, &no_notes),
    };
    let mut emitter = new_emitter(
        unit,
        source_name,
        sources,
        SvaMode::None,
        const_array_style,
        struct_notes,
        &inline_notes,
    );
    for &item in &unit.items {
        let ItemKind::Module(m) = &unit.items_arena[item].kind else {
            continue;
        };
        let Some(name) = m.name.text.strip_prefix(inline::VALIDATION_PREFIX) else {
            continue;
        };
        let Some(&(_, fn_span)) = fns.iter().find(|(n, _)| n == name) else {
            continue;
        };
        let before = emitter.diagnostics.len();
        let _ = emitter.emit_module(m, None);
        for d in emitter.diagnostics.drain(before..) {
            let inside = d.primary_span().is_some_and(|l| {
                l.span.file == fn_span.file
                    && l.span.start >= fn_span.start
                    && l.span.end <= fn_span.end
            });
            if !inside {
                continue;
            }
            if !d.code.is_warning() {
                invalid.insert(name.to_string());
            }
            diags.push(d);
        }
    }
    (invalid, diags)
}

/// sv-mapping.md §12 — deterministik başlık (tarih yok).
pub(crate) fn header(source_name: &str) -> String {
    header_for(source_name, None)
}

/// Başlık; modül başına dosyada (ADR-0024) `Module:` satırı da bulunur.
pub(crate) fn header_for(source_name: &str, module: Option<&str>) -> String {
    let module_line = module.map_or(String::new(), |m| format!("// Module:  {m}\n"));
    format!(
        "// This file was generated by Volt.\n\
         // Source:  {source_name}\n\
         {module_line}\
         // Version: {VOLT_VERSION}\n\
         //\n\
         // DO NOT EDIT — make changes in the source file instead.\n\
         \n\
         `default_nettype none\n"
    )
}

/// Üst düzey `const` öğeleri: isim → (bildirilen tip, değer ifadesi).
/// SV üretiminde const referansları boyutlandırılmış literale katlanır
/// (localparam üretilmez; ADR-0031 uygulama notu).
fn collect_consts(ast: &SourceFile) -> HashMap<String, (Idx<TypeRef>, Idx<Expr>)> {
    let mut map = HashMap::new();
    for &item_idx in &ast.items {
        if let ItemKind::Const(c) = &ast.items_arena[item_idx].kind {
            map.insert(c.name.text.clone(), (c.ty, c.value));
        }
    }
    map
}

/// Modülün saat portları, alan bilgisi ve besleyen ham reset portuyla
/// (ADR-0065 §1): beslenen alanın reset'i zincir çıkışıdır.
pub(crate) fn clock_ports_of(
    ast: &SourceFile,
    domains: &HashMap<String, DomainInfo>,
    module: &ModuleDecl,
) -> Vec<ClockPort> {
    module
        .ports
        .iter()
        .filter(|p| {
            matches!(
                ast.types[crate::alias::resolve(ast, p.ty)].kind,
                TypeRefKind::Clock
            )
        })
        .map(|p| {
            let domain = p.domain.as_ref().map(|d| d.text.clone());
            let mut info = domain
                .as_ref()
                .and_then(|d| domains.get(d).cloned())
                .unwrap_or(DEFAULT_DOMAIN);
            let raw_reset = reset_sync::feeding_raw(ast, module, domain.as_deref(), &info);
            if raw_reset.is_some() {
                let last = reset_sync::RESET_SYNC_STAGES - 1;
                info.reset.synced = Some(reset_sync::stage_name(&p.name.text, last));
            }
            ClockPort {
                name: p.name.text.clone(),
                domain,
                info,
                raw_reset,
            }
        })
        .collect()
}

pub(crate) fn collect_domains(ast: &SourceFile) -> HashMap<String, DomainInfo> {
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
                            synced: None,
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
    /// Modül içi sinyal tablosu: isim → genişlik/işaret. Dizi tipli
    /// reg'lerde ELEMAN imzası tutulur; boyut `array_dims`'tedir.
    pub(crate) symbols: HashMap<String, Sig>,
    /// Trit tipli sinyaller (ADR-0003) — `Trit * x` seçicisi için.
    pub(crate) trits: HashSet<String>,
    /// Dizi tipli reg'ler (ADR-0035): isim → eleman sayısı N.
    /// SV bildirimi `logic [W-1:0] ad [0:N-1]` biçimindedir.
    pub(crate) array_dims: HashMap<String, u32>,
    /// Dizi tipli port ve wire'lar (ADR-0056): isim → (eleman imzası,
    /// eleman sayısı). PAKETLENMİŞ vektör olarak üretilir (`logic
    /// [N*W-1:0] ad`; Yosys unpacked dizi PORTU kabul etmez), eleman
    /// erişimi `ad[W*i +: W]` part-select'tir; `symbols` toplam
    /// genişliği taşır.
    pub(crate) packed_arrays: HashMap<String, (Sig, u32)>,
    /// Modül içi yerleşik CDC primitif örnekleri (ADR-0027): örnek adı →
    /// doğrulanmış bilgi. Ön geçişte doldurulur ki `f.rd_data` alan
    /// erişimleri deyim sırasından bağımsız `f_rd_data`'ya çevrilsin.
    pub(crate) builtin_insts: HashMap<String, builtin_prim::BuiltinInst>,
    /// Modül içi kullanıcı modülü örnekleri (ADR-0041): örnek adı →
    /// hedef modül + çıkış telleri; `f.result` → `f_result`.
    pub(crate) user_insts: HashMap<String, instance::UserInst>,
    /// Üst düzey const tablosu — Path referansları literale katlanır.
    /// Modül sinyalleri (symbols) aynı adı gölgeler.
    pub(crate) consts: HashMap<String, (Idx<TypeRef>, Idx<Expr>)>,
    /// Değişken indeksli const dizinin SV biçimi (ADR-0041).
    pub(crate) const_array_style: ConstArrayStyle,
    /// Bu modülde değişken indeksle kullanılan dizi sabitleri (ilk
    /// kullanım sırasıyla) — gövde başına tablo bildirimi üretilir.
    pub(crate) array_consts_used: Vec<String>,
    /// Açılmakta olan `for` döngülerinin değişkenleri (içten dışa
    /// gölgeleme; en son eklenen kazanır).
    pub(crate) loop_vars: Vec<(String, i128)>,
    /// Gövde başına konan ön bildirimler: örnek çıkış telleri.
    /// Örnek çıkış telleri: (tel adı, bildirim satırı).
    pub(crate) pre_decls: Vec<(String, String)>,
    /// Bir örneğin `inout`/`opendrain` portuna bağlanan üst modül
    /// telleri (ADR-0051): `wire` (inout) ya da `tri1` (opendrain —
    /// pull-up'lı kablolu-VE) olarak bildirilir, `logic` değil.
    pub(crate) bus_wires: HashMap<String, PortDir>,
    /// Kaynak metin — SVA yorumlarındaki satır numaraları için.
    /// Birimdeki kaynak dosyalar (ADR-0042) — ilk giriş ana dosya.
    pub(crate) sources: &'a [SourceText<'a>],
    pub(crate) source_name: &'a str,
    pub(crate) sva_mode: SvaMode,
    pub(crate) sva_files: Vec<SvaFile>,
    pub(crate) sva_props: Vec<SvaProp>,
    /// Immediate modda prev() çağrısı → yardımcı reg adı (ADR-0040).
    pub(crate) past_regs: HashMap<Idx<Expr>, String>,
    /// Simulation modunda bu modülün kullandığı DPI geri çağrıları
    /// (ADR-0064) — gövde başına yalnız gerekenlerin `import`'u konur.
    pub(crate) sim_dpi: sim_contract::SimDpiUse,
    /// Modülde adı geçen enum varyantları (enum, varyant), ilk kullanım
    /// sırasıyla — `localparam` yalnız bunlar için (ADR-0074).
    pub(crate) enum_used: Vec<(String, String)>,
    /// Enum tipli modül sinyalleri: ad → enum adı (ADR-0074).
    pub(crate) enum_sigs: HashMap<String, String>,
    /// Struct indirgemesinin notları (ADR-0077): düzen yorumu, okunmayan
    /// yaprak, paketlenmiş dizi register'ı.
    pub(crate) struct_notes: &'a structs::StructNotes,
    /// fn açılımının notları (ADR-0081): çağrı başlığı yorumu, boyut
    /// dönüşümleri, const'a bağlı gövde yolları.
    pub(crate) inline_notes: &'a inline::InlineNotes,
    /// Başlığı yazılmış çağrılar (modül başına bir kez).
    pub(crate) inline_headers_done: HashSet<(String, String)>,
    /// Üretilmekte olan modülün adı (struct notlarının anahtarı).
    pub(crate) module_name: String,
    /// E1013 verilmiş SV adları (ADR-0078) — güvenlik ağı aynı adı
    /// ikinci kez bildirmez.
    pub(crate) sv_name_reported: HashSet<String>,
}

impl<'a> Emitter<'a> {
    /// Aynı düğüm birden çok geçişte sorgulanabilir (ör. port tipi hem
    /// sembol tablosunda hem bildirimde `sig_of_typeref`'ten geçer);
    /// birebir aynı tanı ikinci kez eklenmez.
    pub(crate) fn error(&mut self, code: ErrorCode, message: String, span: Span, help: &str) {
        let diag = Diagnostic::error(code, message, LabeledSpan::primary(span, ""), help);
        if !self.diagnostics.contains(&diag) {
            self.diagnostics.push(diag);
        }
    }

    /// E0003 — geçerli Volt ama SystemVerilog eşlemesi henüz yok. Mesaj
    /// NEYİN desteklenmediğini söyler (ADR-0070); `check` ve editör de
    /// aynı tanıyı görür (çıktısız emit doğrulaması).
    pub(crate) fn future(&mut self, span: Span, what: &str) {
        self.error(
            ErrorCode::E0003,
            lstr!(
                en: "not supported yet: {what}";
                tr: "henüz desteklenmiyor: {what}"
            ),
            span,
            &lstr!(
                en: "this is valid Volt but has no SystemVerilog mapping yet; express it with supported constructs (see volt explain E0003)";
                tr: "bu geçerli Volt ama henüz SystemVerilog eşlemesi yok; desteklenen yapılarla yazın (bkz. volt explain E0003)"
            ),
        );
    }

    /// `clk`'nin ham reset zinciri bu modülde tüketiliyor mu (ADR-0072;
    /// kural volt-ast `reset_chain`, SDC tarafıyla ortak).
    fn chain_consumed(&self, module: &ModuleDecl, clk: &str) -> bool {
        volt_ast::reset_chain::chain_consumed(self.ast, module, clk, |child, port| {
            clock_ports_of(self.ast, &self.domains, child)
                .iter()
                .any(|c| c.name == port && !c.info.reset.is_none() && c.info.reset.synced.is_none())
        })
    }

    /// Saat portları, port sırasıyla; `@Domain` yoksa varsayılan alan.
    fn collect_clock_ports(&self, module: &ModuleDecl) -> Vec<ClockPort> {
        clock_ports_of(self.ast, &self.domains, module)
    }

    // ═══ Modül ════════════════════════════════════════════════════

    fn emit_module(&mut self, module: &'a ModuleDecl, doc: Option<&str>) -> String {
        let ast = self.ast;
        self.symbols.clear();
        self.trits.clear();
        self.array_dims.clear();
        self.packed_arrays.clear();
        self.array_consts_used.clear();
        self.loop_vars.clear();
        self.pre_decls.clear();
        self.bus_wires.clear();
        self.sim_dpi = sim_contract::SimDpiUse::default();
        self.enum_used.clear();
        self.enum_sigs.clear();
        self.module_name = module.name.text.clone();

        // Sembol tablosu: portlar + reg'ler + wire'lar (let'ler sırayla eklenir)
        for port in &module.ports {
            // Ham reset portu (ADR-0065): 1 bit, `reset` tip eşlemesi yok.
            if reset_sync::is_raw_reset(ast, port) {
                self.symbols.insert(port.name.text.clone(), Sig::BIT);
                continue;
            }
            self.note_trit(&port.name.text, port.ty);
            self.note_enum_signal(&port.name.text, port.ty);
            if let Some(sig) = self.signal_sig(&port.name.text, port.ty, port.span) {
                self.symbols.insert(port.name.text.clone(), sig);
            }
        }
        for &stmt_idx in &module.body {
            let span = ast.stmts[stmt_idx].span;
            // ADR-0074: enum tipli `let` (tipli ya da enum ifadesinden
            // çıkan) gövdedeki konumundan bağımsız bilinmeli — match
            // sınananı olarak kendisinden önce kullanılabilir.
            if let StmtKind::Let(l) = &ast.stmts[stmt_idx].kind {
                match l.ty {
                    Some(t) => self.note_enum_signal(&l.name.text, t),
                    None => {
                        if let Some(e) = self.enum_of_expr(l.value) {
                            self.enum_sigs
                                .insert(l.name.text.clone(), e.name.text.clone());
                        }
                    }
                }
            }
            if let StmtKind::Wire(w) = &ast.stmts[stmt_idx].kind {
                self.note_trit(&w.name.text, w.ty);
                self.note_enum_signal(&w.name.text, w.ty);
                if let Some(sig) = self.signal_sig(&w.name.text, w.ty, span) {
                    self.symbols.insert(w.name.text.clone(), sig);
                }
            }
            if let StmtKind::Reg(reg) = &ast.stmts[stmt_idx].kind {
                if let Some(ty) = reg.ty {
                    self.note_trit(&reg.name.text, ty);
                    self.note_enum_signal(&reg.name.text, ty);
                }
                match reg.ty {
                    // Struct yaprağı olan dizi register'ı paketlenmiş vektördür
                    // (ADR-0077 Karar 3: `p as uN` birleştirmesi) — wire gibi.
                    Some(ty)
                        if self
                            .struct_notes
                            .is_packed(&self.module_name, &reg.name.text) =>
                    {
                        if let Some(sig) = self.signal_sig(&reg.name.text, ty, span) {
                            self.symbols.insert(reg.name.text.clone(), sig);
                        }
                    }
                    // Dizi tipli reg (ADR-0035): eleman imzası + boyut.
                    Some(ty)
                        if matches!(
                            &ast.types[crate::alias::resolve(ast, ty)].kind,
                            TypeRefKind::Array { .. }
                        ) =>
                    {
                        if let Some((sig, len)) = self.array_reg_sig(ty, span) {
                            self.symbols.insert(reg.name.text.clone(), sig);
                            self.array_dims.insert(reg.name.text.clone(), len);
                        }
                    }
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
                            en: "write the reg type explicitly: reg name : u8 = 0";
                            tr: "reg tipini açık yazın: reg isim : u8 = 0"
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
        self.collect_user_insts(module);
        for cfg in &resets {
            self.symbols.insert(cfg.port_name().to_string(), Sig::BIT);
        }
        for clock in clocks.iter().filter(|c| c.raw_reset.is_some()) {
            for i in 0..reset_sync::RESET_SYNC_STAGES {
                self.symbols
                    .insert(reset_sync::stage_name(&clock.name, i), Sig::BIT);
            }
        }

        let ports_block = self.emit_ports(module, &resets);
        let mut body_chunks = self.emit_body(module, &clocks);
        let const_lines = self.const_array_lines();
        // ADR-0051: çift yönlü portların üç durumlu tamponları.
        if let Some(chunk) = self.emit_bidir_drivers(module) {
            body_chunks.push(chunk);
        }

        // F4a — kontratlardan SVA üretimi (moda göre gömülü ya da ayrı).
        let sva_before = self.sva_files.len();
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
            SvaMode::Simulation => {
                if let Some(block) = self.sva_simulation(module, &clocks, 4) {
                    body_chunks.push(block);
                }
            }
        }
        // ADR-0065 §2: bırakma senkronizörleri gövdenin başında — yalnız
        // zincir tüketiliyorsa (ADR-0072); ham portu çocuğa geçiren ara
        // seviyenin zinciri ölü mantıktı. Üretilen metinde (gövde, SVA,
        // izleyiciler) zincir adı geçiyorsa her hâlükârda üretilir.
        let sva_owned = self.sva_files.get(sva_before).map(|f| f.content.clone());
        let sva_text = sva_owned.as_deref();
        self.silence_unread_builtin_outputs(&mut body_chunks, sva_text);
        let synchronizers: Vec<String> = clocks
            .iter()
            .filter(|c| {
                self.chain_consumed(module, &c.name)
                    || reset_sync::chain_referenced(c, &body_chunks, sva_text)
            })
            .filter_map(reset_sync::synchronizer_block)
            .collect();
        // Bir örnek çıkış teli örnek bağlantısı dışında da geçiyorsa okunur.
        let read = |wire: &str| {
            let uses: usize = body_chunks.iter().map(|c| count_ident(c, wire)).sum();
            uses > 1 || sva_text.is_some_and(|s| count_ident(s, wire) > 0)
        };
        let pre = self.pre_decl_chunk(const_lines, read);
        body_chunks.splice(0..0, synchronizers);
        if let Some(pre) = pre {
            body_chunks.insert(0, pre);
        }
        // ADR-0074: gövdede ve gömülü SVA'da adı geçen enum varyantları.
        let used = std::mem::take(&mut self.enum_used);
        if let Some(params) = self.enum_localparams(&used, module.name.span) {
            body_chunks.insert(0, params);
        }
        // İzleyiciler (modül ve primitif kontratları) üretildikten SONRA:
        // gövde başına yalnız kullanılan DPI bildirimleri.
        if self.sva_mode == SvaMode::Simulation {
            if let Some(imports) = self.sim_dpi_imports() {
                body_chunks.insert(0, imports);
            }
        }

        // ADR-0076: alanının reset'ini hiçbir register kullanmayan modül
        // (flop'suz) otomatik reset portunu arayüzde TUTAR — arayüz
        // gövdeye değil bildirilen saat alanlarına bağlıdır (ADR-0012);
        // yalnız Verilator -Wall UNUSEDSIGNAL susturulur.
        let unused_resets: Vec<&str> = resets
            .iter()
            .map(ResetCfg::port_name)
            .filter(|rst| {
                body_chunks.iter().all(|c| count_ident(c, rst) == 0)
                    && sva_text.is_none_or(|s| count_ident(s, rst) == 0)
            })
            .collect();
        let ports_block = silence_unused_resets(ports_block, &unused_resets);

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

    /// Gövde başı ön bildirimleri (ADR-0041): değişken indeksli dizi
    /// sabitlerinin tabloları + kullanıcı örneklerinin çıkış telleri.
    /// Modülün okumadığı çıkış telleri (`read` yalnız örnek bağlantısında
    /// geçiyorsa) Verilator `UNUSEDSIGNAL` susturmasıyla sarılır: Volt'ta
    /// bir örnek çıkışını okumamak meşrudur, boş bağlantı (`.p()`) ise
    /// `-Wall`'da PINCONNECTEMPTY verir (ADR-0072).
    fn pre_decl_chunk(
        &mut self,
        const_lines: Vec<String>,
        read: impl Fn(&str) -> bool,
    ) -> Option<String> {
        let mut lines = const_lines;
        let mut unread = Vec::new();
        for (wire, line) in std::mem::take(&mut self.pre_decls) {
            if read(&wire) {
                lines.push(line);
            } else {
                unread.push(line);
            }
        }
        if !unread.is_empty() {
            lines.push("    // instance outputs this module does not read".to_string());
            lines.push("    // verilator lint_off UNUSEDSIGNAL".to_string());
            lines.extend(unread);
            lines.push("    // verilator lint_on UNUSEDSIGNAL".to_string());
        }
        (!lines.is_empty()).then(|| lines.join("\n"))
    }

    /// Değişken indeksli dizi sabitlerinin tabloları (ADR-0041).
    fn const_array_lines(&mut self) -> Vec<String> {
        let used = std::mem::take(&mut self.array_consts_used);
        used.iter()
            .filter_map(|name| self.emit_const_array_decl(name))
            .collect()
    }

    /// Port sırası (§1): clock'lar → reset'ler → in → inout → out.
    fn emit_ports(&mut self, module: &'a ModuleDecl, resets: &[ResetCfg]) -> String {
        let ast = self.ast;
        let is_clock = |p: &volt_ast::Port| {
            matches!(
                ast.types[crate::alias::resolve(ast, p.ty)].kind,
                TypeRefKind::Clock
            )
        };

        let mut lines: Vec<(&'static str, String, String)> = Vec::new();
        for port in module.ports.iter().filter(|p| is_clock(p)) {
            lines.push(("input", "logic".into(), port.name.text.clone()));
        }
        for cfg in resets {
            lines.push(("input", "logic".into(), cfg.port_name().into()));
        }
        // Ham reset portları otomatik portlardan sonra (ADR-0065 §1).
        for port in module
            .ports
            .iter()
            .filter(|p| reset_sync::is_raw_reset(ast, p))
        {
            lines.push(("input", "logic".into(), port.name.text.clone()));
        }
        for pass in [
            PortDir::In,
            PortDir::InOut,
            PortDir::OpenDrain,
            PortDir::Out,
        ] {
            for port in &module.ports {
                if port.direction != pass || is_clock(port) || reset_sync::is_raw_reset(ast, port) {
                    continue;
                }
                let mut ty = match self.packed_arrays.get(&port.name.text) {
                    Some(_) => self.symbols[&port.name.text].decl_type(),
                    None => self.sv_type_string(port.ty, port.span),
                };
                let dir = match pass {
                    PortDir::In => "input",
                    // IEEE 1800 23.2.2.3: inout portu net olmalı (ADR-0051).
                    PortDir::InOut | PortDir::OpenDrain => {
                        ty = ty.replacen("logic", "wire", 1);
                        "inout"
                    }
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
                let note = self.enum_comment(name);
                let decl = self.struct_decl_lines(
                    name,
                    format!("    {dir:<6} {ty:<ty_width$} {name}{comma}{note}"),
                );
                verilator_cpp_word_port(name, decl)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Struct yaprağının bildirim satırı (ADR-0077 Karar 5): grubun ilk
    /// yaprağının üstünde düzen yorumu; modülün okumadığı yaprak
    /// Verilator UNUSEDSIGNAL susturmasıyla sarılır (kural 2, 13).
    pub(crate) fn struct_decl_lines(&self, name: &str, line: String) -> String {
        let module = &self.module_name;
        let mut out = String::new();
        if let Some(note) = self.struct_notes.layout_of(module, name) {
            out.push_str(&format!("    {note}\n"));
        }
        if self.struct_notes.is_unused(module, name) {
            out.push_str("    // struct field unused in this module\n");
            out.push_str("    // verilator lint_off UNUSEDSIGNAL\n");
            out.push_str(&line);
            out.push_str("\n    // verilator lint_on UNUSEDSIGNAL");
        } else {
            out.push_str(&line);
        }
        out
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
                    // Unpacked dizi boyutu isimden SONRA yazılır (§2):
                    // `logic [31:0] regs [0:31];` — sentez araçları bunu
                    // BRAM/dağıtık RAM'e eşleyebilir.
                    let dims = match self.array_dims.get(&reg.name.text) {
                        Some(n) => format!(" [0:{}]", n - 1),
                        None => String::new(),
                    };
                    (
                        Kind::Decl,
                        self.struct_decl_lines(
                            &reg.name.text,
                            format!(
                                "    {} {}{dims};{}",
                                sig.decl_type(),
                                reg.name.text,
                                self.enum_comment(&reg.name.text)
                            ),
                        ),
                    )
                }),
                StmtKind::Let(decl) => {
                    // Bildirilen tip wire genişliğini SÜRER (ADR-0041):
                    // `let p : i32 = a * b` → 32 bitlik wire; tip yoksa
                    // kaba çıkarım.
                    if let Some(t) = decl.ty {
                        self.note_enum_signal(&decl.name.text, t);
                    }
                    let sig = decl
                        .ty
                        .and_then(|t| self.sig_of_typeref(t, stmt.span))
                        .or_else(|| self.width_of(decl.value));
                    let is_trit = match decl.ty {
                        Some(t) => self.is_trit_typeref(t),
                        None => self.is_trit(decl.value),
                    };
                    if is_trit {
                        self.trits.insert(decl.name.text.clone());
                    }
                    match sig {
                        Some(sig) => {
                            self.symbols.insert(decl.name.text.clone(), sig);
                            let value = self.emit_assigned(decl.value, Some(sig));
                            let header = self.inline_header(&decl.name.text);
                            Some((
                                Kind::Decl,
                                header
                                    + &self.struct_decl_lines(
                                        &decl.name.text,
                                        format!(
                                            "    wire {}{} = {};{}",
                                            sig.wire_prefix(),
                                            decl.name.text,
                                            value,
                                            self.enum_comment(&decl.name.text)
                                        ),
                                    ),
                            ))
                        }
                        // Açık tip zaten tanılandı (sig_of_typeref); tipsizse
                        // değer üretilir ki desteklenmeyen ifade (sync, match,
                        // struct literali ...) kendi E0003'ünü versin — kaskad
                        // E2005 yalnız başka tanı yoksa (ADR-0070).
                        None if decl.ty.is_some() => None,
                        None if {
                            let before = self.diagnostics.len();
                            let _ = self.emit_assigned(decl.value, None);
                            self.diagnostics.len() > before
                        } =>
                        {
                            None
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
                        Some(info.reset.clone())
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
                            let rhs = self.emit_assigned(assign.rhs, lhs_sig);
                            Some((Kind::Assign, format!("    assign {lhs} = {rhs};")))
                        }
                    }
                }
                StmtKind::Expr(_) | StmtKind::Error => None, // parse tanısı zaten var
                // `wire x : T` → `logic` bildirimi (ADR-0041); sembol ön
                // geçişte eklendi, dizi tipli wire hâlâ future.
                StmtKind::Wire(w) => match self.symbols.get(&w.name.text).copied() {
                    // Bir örneğin çift yönlü portuna bağlanan tel net'tir
                    // (ADR-0051): `wire`; açık drenaj hattı `tri1` (pull-up).
                    Some(sig) => Some((
                        Kind::Decl,
                        // Yosys'in Verilog ön ucu `tri1` tanımaz: formal
                        // (Immediate) çıktısında pull-up'sız `wire` (ADR-0051 sınırı).
                        match self.bus_wires.get(&w.name.text) {
                            Some(PortDir::OpenDrain) if self.sva_mode != SvaMode::Immediate => {
                                format!("    tri1 {}{};", sig.wire_prefix(), w.name.text)
                            }
                            Some(_) => format!("    wire {}{};", sig.wire_prefix(), w.name.text),
                            None => self.struct_decl_lines(
                                &w.name.text,
                                format!(
                                    "    {} {};{}",
                                    sig.decl_type(),
                                    w.name.text,
                                    self.enum_comment(&w.name.text)
                                ),
                            ),
                        },
                    )),
                    // Tip ön geçişte (`signal_sig`) zaten tanılandı (ADR-0070:
                    // aynı sorun için ikinci E0003 yok).
                    None => None,
                },
                StmtKind::Instance(inst) => {
                    let is_builtin = inst.module_path.segments.len() == 1
                        && BuiltinPrim::from_name(&inst.module_path.segments[0].text).is_some();
                    if is_builtin {
                        // Ön geçiş doğrulayamadıysa tanı üretildi — boş chunk.
                        self.emit_builtin_instance(&module.name.text, &inst.name.text, stmt.span)
                            .map(|chunk| (Kind::Always, chunk))
                    } else {
                        // Kullanıcı modülü (ADR-0041) — hedef yoksa E0003.
                        self.emit_user_instance(module, clocks, inst, stmt.span)
                            .map(|chunk| (Kind::Always, chunk))
                    }
                }
                StmtKind::Comb(block) => Some((Kind::Always, self.emit_comb(*block))),
                // Modül seviyesi `for` parser'da açıldı (ADR-0056);
                // sınırı sabit olmayan döngü tanıyla birlikte düşürüldü.
                StmtKind::For(_) => None,
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

    /// sv-mapping.md §17 (ADR-0051) — çift yönlü portların üç durumlu
    /// tamponları. Sürücü register'ları parser sentezledi (`<p>_oe` +
    /// `<p>_out`, `<p>_drive_low`); yalnız sürülen (ya da `released`/
    /// `driving` ile gözlenen) portun register'ı vardır — yalnız okunan
    /// pad için `assign` üretilmez (modül hattı sürmez).
    fn emit_bidir_drivers(&mut self, module: &'a ModuleDecl) -> Option<String> {
        let mut lines = Vec::new();
        for port in &module.ports {
            let Some(regs) = port.direction.bidir_regs(&port.name.text) else {
                continue;
            };
            if !self.symbols.contains_key(&regs.enable) {
                continue;
            }
            let Some(sig) = self.symbols.get(&port.name.text).copied() else {
                continue;
            };
            let name = &port.name.text;
            // Formal (Immediate) model: Yosys reads a released `'z` net as
            // constant 0, which would freeze any pad that waits for its
            // pull-up. The external device is modelled as an unconstrained
            // `(* anyseq *)` driver that owns the line while it is released:
            // released → free value, driven → the module's value.
            let formal = self.sva_mode == SvaMode::Immediate;
            let released = if formal {
                lines.push(format!(
                    "    // formal model of the external device on {name} (ADR-0051): free while released"
                ));
                lines.push(format!("    (* anyseq *) {} {name}_ext;", sig.decl_type()));
                format!("{name}_ext")
            } else if sig.width == 1 {
                "1'bz".to_string()
            } else {
                format!("{{{}{{1'bz}}}}", sig.width)
            };
            let line = match (port.direction, regs.data) {
                (PortDir::OpenDrain, _) => {
                    format!("    assign {name} = {} ? 1'b0 : {released};", regs.enable)
                }
                (_, Some(data)) if self.symbols.contains_key(&data) => {
                    format!("    assign {name} = {} ? {data} : {released};", regs.enable)
                }
                _ => format!("    assign {name} = {released};"),
            };
            if !formal {
                lines.push(format!(
                    "    // {} pad (ADR-0051): driven only while {} is high",
                    port.direction.keyword(),
                    regs.enable
                ));
            }
            lines.push(line);
        }
        (!lines.is_empty()).then(|| lines.join("\n"))
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

        // Analiz (typeck) aynı tanıyı verir ve emit'i kapatır; burası
        // yalnız HIR'siz `emit()` çağrıları için (aynı kod ve metin).
        if args.len() != 2 {
            let name = if stages == 3 { "sync3" } else { "sync" };
            let got = args.len();
            self.error(
                ErrorCode::E2003,
                lstr!(en: "'{name}()' takes 2 arguments (source, destination clock), {got} given";
                      tr: "'{name}()' 2 argüman alır (kaynak, hedef saat), {got} verildi"),
                span,
                &lstr!(en: "write it as: dest = {name}(src, dst_clk)";
                       tr: "şöyle yazın: hedef = {name}(kaynak, hedef_saat)"),
            );
            return Some(String::new());
        }
        let (src_arg, clk_arg) = (args[0], args[1]);
        let Some(src) = path_single(ast, src_arg).map(str::to_owned) else {
            self.future(
                span,
                &lstr!(
                    en: "sync() with a compound source expression (bind it with let first)";
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
                &cap.info,
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
        out.push_str(&sync_always_ff(&dst.name, &dst.info, &chain, &zero));
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
                        en: "'on <clock>.reset' blocks";
                        tr: "'on <saat>.reset' blokları"
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
        // ADR-0077: yazılan struct yaprağının kardeşleri de reset alır.
        for w in written.clone() {
            if let Some(sibs) = self.struct_notes.siblings_of(&self.module_name, &w) {
                for s in sibs {
                    if !written.contains(s) {
                        written.push(s.clone());
                    }
                }
            }
        }

        let mut lines = Vec::new();
        for &stmt_idx in &module.body {
            if let StmtKind::Reg(reg) = &ast.stmts[stmt_idx].kind {
                if written.iter().any(|w| w == &reg.name.text) {
                    let sig = self.symbols.get(&reg.name.text).copied();
                    if let Some(n) = self.array_dims.get(&reg.name.text).copied() {
                        lines.extend(self.array_reset_lines(&reg.name.text, reg.init, sig, n));
                        continue;
                    }
                    let init = self.emit_expr(reg.init, sig);
                    lines.push(format!("{} <= {};", reg.name.text, init));
                }
            }
        }
        lines
    }

    /// Dizi reg reset satırları (ADR-0035). `'{default: v}` deseni Yosys
    /// tarafından desteklenmediğinden tekrar literali for döngüsüne,
    /// liste literali eleman atamalarına açılır — iki araç da kabul eder.
    fn array_reset_lines(
        &mut self,
        name: &str,
        init: Idx<Expr>,
        sig: Option<Sig>,
        n: u32,
    ) -> Vec<String> {
        match &self.ast.exprs[init].kind {
            // `reg r : [T; N] = COEFFS` — dizi sabiti literaline açılır.
            ExprKind::Path(p)
                if p.segments.len() == 1 && self.is_const_array(&p.segments[0].text) =>
            {
                let (_, value) = self.consts[&p.segments[0].text];
                self.array_reset_lines(name, value, sig, n)
            }
            ExprKind::ArrayLit(ArrayLitKind::Repeat { value, .. }) => {
                let value = *value;
                let v = self.emit_expr(value, sig);
                vec![format!(
                    "for (int volt_i = 0; volt_i < {n}; volt_i = volt_i + 1) {name}[volt_i] <= {v};"
                )]
            }
            ExprKind::ArrayLit(ArrayLitKind::List(items)) => {
                let items = items.clone();
                items
                    .iter()
                    .enumerate()
                    .map(|(k, &e)| {
                        let v = self.emit_expr(e, sig);
                        format!("{name}[{k}] <= {v};")
                    })
                    .collect()
            }
            _ => {
                let v = self.emit_expr(init, sig);
                vec![format!("{name} <= {v};")]
            }
        }
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
                    let rhs_s = self.emit_assigned(*rhs, sig);
                    lines.push(format!("{ind}{lhs_s} <= {rhs_s};"));
                }
                BlockStmt::BlockAssign { lhs, rhs, .. } => {
                    let sig = self.lvalue_sig(lhs);
                    let lhs_s = self.emit_lvalue(lhs);
                    let rhs_s = self.emit_assigned(*rhs, sig);
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
                BlockStmt::Match(m) => self.emit_match(m, indent, &mut lines),
                // Derleme zamanı döngüsü: gövde iterasyon başına açılır.
                BlockStmt::For(f) => self.emit_for_in_block(f, indent, &mut lines),
            }
        }
        lines
    }

    /// `match` deyimi `case` yapısına iner (ADR-0032). Desen kapsamı:
    /// literal, literal `A | B` alternatifi ve joker `_` (→ default).
    /// Muhafız (guard) ile bağlama/yol/tuple desenleri sonraki aşamalarda.
    fn emit_match(&mut self, m: &'a MatchStmt, indent: usize, lines: &mut Vec<String>) {
        let ind = " ".repeat(indent);
        let scrut_sig = self.width_of(m.scrutinee);
        let scrut_enum = self.enum_of_expr(m.scrutinee);
        let scrut = self.emit_expr(m.scrutinee, scrut_sig);
        // ADR-0074 Karar 4: erişilemez kol atlanır; kapsayıcı `_`'sız enum
        // match'inde son adlı kol `default` olur (geçersiz kodlar dahil).
        let plan = scrut_enum.map(|d| self.enum_match_plan(m, d));
        // Sayısal sınananda aynı kural (ADR-0075): değerleri önceki
        // kollarda geçen kol yazılmaz (HIR W2014).
        let value_skip = match plan {
            Some(_) => Vec::new(),
            None => volt_ast::match_cover::unreachable_value_arms(self.ast, m),
        };
        lines.push(format!("{ind}case ({scrut})"));
        for (i, arm) in m.arms.iter().enumerate() {
            if plan.as_ref().is_some_and(|p| p.skip[i]) || value_skip.get(i) == Some(&true) {
                continue;
            }
            if let Some(p) = plan.as_ref().filter(|p| p.default_arm == Some(i)) {
                let names = &p.default_names;
                let invalid = if p.has_invalid_codes {
                    " (and invalid codes)"
                } else {
                    ""
                };
                lines.push(format!("{ind}    default: begin // {names}{invalid}"));
                self.emit_arm_body(arm, indent, lines);
                lines.push(format!("{ind}    end"));
                continue;
            }
            if arm.guard.is_some() {
                self.future(
                    arm.span,
                    &lstr!(
                        en: "'match' arm guards ('if' after a pattern)";
                        tr: "'match' kolu muhafızları (desenden sonra 'if')"
                    ),
                );
                continue;
            }
            let Some(label) = self.match_arm_label(arm.pattern, scrut_sig, scrut_enum) else {
                continue;
            };
            lines.push(format!("{ind}    {label}: begin"));
            self.emit_arm_body(arm, indent, lines);
            lines.push(format!("{ind}    end"));
        }
        lines.push(format!("{ind}endcase"));
    }

    fn emit_arm_body(
        &mut self,
        arm: &'a volt_ast::MatchArm,
        indent: usize,
        lines: &mut Vec<String>,
    ) {
        match &arm.body {
            MatchArmBody::Block(b) => lines.extend(self.emit_block(*b, indent + 8)),
            MatchArmBody::Expr(e) => {
                let span = self.ast.exprs[*e].span;
                self.future(
                    span,
                    &lstr!(
                        en: "expression-bodied 'match' arms in statement position";
                        tr: "deyim konumunda ifade gövdeli 'match' kolları"
                    ),
                );
            }
        }
    }

    /// Kol etiketi: literal(ler) virgülle ayrılır, joker `default` olur;
    /// enum sınananda varyant yolu `<Enum>_<Varyant>` (ADR-0074).
    /// None → tanı üretildi ya da desen zaten hatalı, kol atlanır.
    fn match_arm_label(
        &mut self,
        pattern: Idx<Pattern>,
        scrut_sig: Option<Sig>,
        scrut_enum: Option<&'a volt_ast::EnumDecl>,
    ) -> Option<String> {
        let ast = self.ast;
        match &ast.patterns[pattern].kind {
            PatternKind::Path { path, args: None }
                if scrut_enum.is_some() && self.enum_variant_of_path(path).is_some() =>
            {
                self.emit_enum_variant(path)
            }
            PatternKind::Binding(name)
                if scrut_enum
                    .is_some_and(|d| d.variants.iter().any(|v| v.name.text == name.text)) =>
            {
                let enum_name = scrut_enum.map_or("", |d| d.name.text.as_str());
                let variant = name.text.clone();
                self.error(
                    ErrorCode::E0003,
                    lstr!(
                        en: "not supported yet: binding pattern '{variant}' in 'match' (a bare name binds, it does not name the variant)";
                        tr: "henüz desteklenmiyor: 'match' içinde bağlama deseni '{variant}' (çıplak ad bağlar, varyantı adlandırmaz)"
                    ),
                    ast.patterns[pattern].span,
                    &lstr!(
                        en: "write the variant path: {enum_name}::{variant}";
                        tr: "varyant yolunu yazın: {enum_name}::{variant}"
                    ),
                );
                None
            }
            PatternKind::Wildcard => Some("default".to_string()),
            PatternKind::Literal(e) => Some(self.emit_expr(*e, scrut_sig)),
            PatternKind::Or(alts) => {
                if alts
                    .iter()
                    .any(|&a| matches!(ast.patterns[a].kind, PatternKind::Wildcard))
                {
                    return Some("default".to_string());
                }
                let mut labels = Vec::with_capacity(alts.len());
                for &a in alts {
                    labels.push(self.match_arm_label(a, scrut_sig, scrut_enum)?);
                }
                Some(labels.join(", "))
            }
            PatternKind::Error => None, // parse tanısı zaten var
            PatternKind::Binding(_) | PatternKind::Path { .. } | PatternKind::Tuple(_) => {
                self.future(
                    ast.patterns[pattern].span,
                    &lstr!(
                        en: "binding, path and tuple patterns in 'match' (only literals and '_' map to SV)";
                        tr: "'match' içinde bağlama, yol ve tuple desenleri (SV'ye yalnız literal ve '_' iner)"
                    ),
                );
                None
            }
        }
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
            if self.user_insts.contains_key(&lv.base.text) {
                self.error(
                    ErrorCode::E4011,
                    lstr!(
                        en: "cannot assign to '{}.{}': instance ports are driven by the instance", lv.base.text, f.text;
                        tr: "'{}.{}' atanamaz: örnek portlarını örneğin kendisi sürer", lv.base.text, f.text
                    ),
                    lv.span,
                    &lstr!(
                        en: "bind inputs in the instance literal and read outputs as {}.{}", lv.base.text, f.text;
                        tr: "girişleri örnekleme literalinde bağlayın, çıkışları {}.{} ile okuyun", lv.base.text, f.text
                    ),
                );
                return format!("{}_{}", lv.base.text, f.text);
            }
        }
        let mut out = lv.base.text.clone();
        let packed = self.packed_arrays.get(&lv.base.text).copied();
        for (n, suffix) in lv.suffixes.iter().enumerate() {
            match suffix {
                // Paketlenmiş dizi (ADR-0056): ilk indeks eleman part-select'i.
                LValueSuffix::Index(i) if n == 0 && packed.is_some() => {
                    let w = packed.map_or(1, |(s, _)| s.width);
                    let i = self.emit_plain(*i);
                    out.push_str(&packed_select(&i, w));
                }
                LValueSuffix::Index(i) => {
                    let i = self.emit_plain(*i);
                    out.push_str(&format!("[{i}]"));
                }
                LValueSuffix::Range { hi, lo } => {
                    let hi = self.emit_plain(*hi);
                    let lo = self.emit_plain(*lo);
                    out.push_str(&format!("[{hi}:{lo}]"));
                }
                LValueSuffix::PartSelect {
                    start,
                    width,
                    ascending,
                } => {
                    let s = self.emit_plain(*start);
                    let w = self.emit_plain(*width);
                    let op = if *ascending { "+:" } else { "-:" };
                    out.push_str(&format!("[{s} {op} {w}]"));
                }
                LValueSuffix::Field(name) => out.push_str(&format!(".{}", name.text)),
            }
        }
        out
    }

    fn lvalue_sig(&mut self, lv: &LValue) -> Option<Sig> {
        let mut sig = self.symbols.get(&lv.base.text).copied();
        // Dizi tabanında ilk indeks ELEMANI seçer, biti değil (ADR-0035);
        // paketlenmiş port/wire dizisinde eleman imzası (ADR-0056).
        let packed = self.packed_arrays.get(&lv.base.text).copied();
        let mut is_array = self.array_dims.contains_key(&lv.base.text) || packed.is_some();
        for suffix in &lv.suffixes {
            sig = match suffix {
                LValueSuffix::Index(_) if is_array => {
                    is_array = false;
                    packed.map(|(s, _)| s).or(sig)
                }
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
                LValueSuffix::PartSelect { width, .. } => {
                    let w = self.eval_const(*width)?;
                    Some(Sig {
                        width: w as u32,
                        signed: false,
                    })
                }
                LValueSuffix::Field(_) => None,
            };
        }
        sig
    }

    // ═══ Tipler (§2) ══════════════════════════════════════════════

    /// Port / wire imzası: skaler tipler `sig_of_typeref`; `[T; N]`
    /// paketlenmiş vektör olarak kaydedilir (ADR-0056) — toplam genişlik
    /// döner, eleman bilgisi `packed_arrays`'e yazılır.
    pub(crate) fn signal_sig(&mut self, name: &str, ty: Idx<TypeRef>, span: Span) -> Option<Sig> {
        if !matches!(
            self.ast.types[crate::alias::resolve(self.ast, ty)].kind,
            TypeRefKind::Array { .. }
        ) {
            return self.sig_of_typeref(ty, span);
        }
        let (elem, len) = self.array_reg_sig(ty, span)?;
        self.packed_arrays.insert(name.to_string(), (elem, len));
        Some(Sig {
            width: elem.width * len,
            signed: false,
        })
    }

    /// Tanılarda gösterilecek bildirim adı: açılmış `for` gövdesinde
    /// kaynaktaki ad (`pe_0` → `pe`), aksi hâlde kendisi.
    pub(crate) fn shown_name(&self, name: &volt_ast::Name) -> String {
        self.ast
            .generate
            .source_name(name.span, &name.text)
            .to_string()
    }

    /// Bir hedef modül portunun bağlama imzası (örnekleme): dizi port
    /// paketlenmiş toplam genişlik (ADR-0056).
    pub(crate) fn port_sig(&mut self, port: &volt_ast::Port) -> Option<Sig> {
        if reset_sync::is_raw_reset(self.ast, port) {
            return Some(Sig::BIT);
        }
        if !matches!(
            self.ast.types[crate::alias::resolve(self.ast, port.ty)].kind,
            TypeRefKind::Array { .. }
        ) {
            return self.sig_of_typeref(port.ty, port.span);
        }
        let (elem, len) = self.array_reg_sig(port.ty, port.span)?;
        Some(Sig {
            width: elem.width * len,
            signed: false,
        })
    }

    /// `[T; N]` reg tipi (ADR-0035): eleman imzası + eleman sayısı.
    /// Reg bildirimlerinde ve paketlenmiş port/wire dizilerinde
    /// çağrılır; iç içe dizi desteklenmez.
    pub(crate) fn array_reg_sig(&mut self, ty: Idx<TypeRef>, span: Span) -> Option<(Sig, u32)> {
        let TypeRefKind::Array { elem, len } =
            &self.ast.types[crate::alias::resolve(self.ast, ty)].kind
        else {
            return None;
        };
        let (elem, len) = (*elem, *len);
        // Struct dizisi ertelendi (ADR-0077 Karar 2: AoS eleman düzeni ayrı ADR).
        if let Some(decl) = volt_ast::struct_layout::struct_of_type(self.ast, elem) {
            let name = decl.name.text.clone();
            self.future(
                span,
                &lstr!(
                    en: "arrays of structs ('[{name}; N]'; use one signal per element or split the fields into arrays)";
                    tr: "struct dizileri ('[{name}; N]'; eleman başına bir sinyal kullanın ya da alanları dizilere ayırın)"
                ),
            );
            return None;
        }
        if let Some(decl) = self.enum_of_type(elem) {
            let name = decl.name.text.clone();
            self.future(
                span,
                &lstr!(
                    en: "arrays of enum '{name}'";
                    tr: "enum '{name}' dizileri"
                ),
            );
            return None;
        }
        let sig = self.sig_of_typeref(elem, span)?;
        match self.eval_const(len) {
            Some(n) if n >= 1 => Some((sig, n as u32)),
            _ => {
                self.error(
                    ErrorCode::E2005,
                    lstr!(
                        en: "the length of [T; N] cannot be determined at compile time";
                        tr: "[T; N] uzunluğu derleme zamanında belirlenemiyor"
                    ),
                    span,
                    &lstr!(
                        en: "N must be a constant expression (e.g. [u32; 32])";
                        tr: "N sabit bir ifade olmalı (ör. [u32; 32])"
                    ),
                );
                None
            }
        }
    }

    pub(crate) fn sig_of_typeref(&mut self, ty: Idx<TypeRef>, span: Span) -> Option<Sig> {
        match &self.ast.types[crate::alias::resolve(self.ast, ty)].kind {
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
            TypeRefKind::Bits(e) | TypeRefKind::UIntN(e) | TypeRefKind::SIntN(e) => {
                let e = *e;
                let signed = matches!(
                    &self.ast.types[crate::alias::resolve(self.ast, ty)].kind,
                    TypeRefKind::SIntN(_)
                );
                match self.eval_const(e) {
                    Some(n) if n >= 1 => Some(Sig {
                        width: n as u32,
                        signed,
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
            // ADR-0003: 2 bit işaretli depolama (+1 = 01, 0 = 00, -1 = 11).
            TypeRefKind::Trit => Some(Sig {
                width: 2,
                signed: true,
            }),
            // Ham reset GİRİŞ portu (ADR-0065) buraya gelmez; kalan konumlar.
            TypeRefKind::Reset(_) => {
                self.future(
                    span,
                    &lstr!(
                        en: "values of type 'reset' outside an input port";
                        tr: "giriş portu dışında 'reset' tipli değerler"
                    ),
                );
                None
            }
            TypeRefKind::Error => None, // parse tanısı zaten var
            TypeRefKind::Array { .. } => {
                self.future(
                    span,
                    &lstr!(
                        en: "array types here (arrays map to SV only as one-dimensional reg, port and wire types)";
                        tr: "bu konumda dizi tipleri (SV'ye yalnız tek boyutlu reg, port ve wire dizisi iner)"
                    ),
                );
                None
            }
            TypeRefKind::Tuple(_) => {
                self.future(span, &lstr!(en: "tuple types"; tr: "tuple tipleri"));
                None
            }
            TypeRefKind::Path { .. } => {
                if let Some(decl) = self.enum_of_type(ty) {
                    return self.enum_sig(decl, span);
                }
                let what = alias::describe_user_type(self.ast, alias::resolve(self.ast, ty));
                self.future(span, &what);
                None
            }
        }
    }

    /// Struct yaprağının imzası (ADR-0077): dizi alanı paketlenmiş
    /// vektördür (ADR-0056), diğerleri `sig_of_typeref`.
    pub(crate) fn leaf_sig(&mut self, ty: Idx<TypeRef>, span: Span) -> Option<Sig> {
        match self.array_reg_sig(ty, span) {
            Some((elem, len)) => Some(Sig {
                width: elem.width * len,
                signed: false,
            }),
            None => self.sig_of_typeref(ty, span),
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
            BlockStmt::Match(m) => {
                for arm in &m.arms {
                    if let MatchArmBody::Block(b) = &arm.body {
                        collect_written(ast, &ast.blocks[*b], out);
                    }
                }
            }
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

/// Paketlenmiş dizi elemanı seçimi (ADR-0056): `[W*i +: W]`; indeks
/// literalse çarpım katlanır (`[16 +: 8]`).
pub(crate) fn packed_select(index: &str, width: u32) -> String {
    match index.parse::<u64>() {
        Ok(i) => format!("[{} +: {width}]", i * u64::from(width)),
        Err(_) => format!("[{width} * ({index}) +: {width}]"),
    }
}
