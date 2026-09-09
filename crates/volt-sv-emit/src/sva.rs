//! Kontratlardan SVA üretimi (F4a ADIM 2-3).
//!
//! Eşleme (Volt-Butunlesik-Mimari-v3.md kontrat tablosu):
//! invariant/ensures/assert → `assert property`, requires/assume →
//! `assume property`, cover → `cover property`. Saat ve `disable iff`
//! modülün ilk saat portunun alanından gelir; saatsiz modülde SVA
//! üretilmez (formel araçlar saat ister).
//!
//! Varsayılan çıktı ayrı `.sva` dosyasıdır ve hedef modüle `bind` ile
//! bağlanır; `(.*)` bağlama modül kapsamında ada göre çözüldüğünden
//! kontrol modülünün portları register'lara da erişebilir.

use volt_ast::{BinOp, ClockEdge, Contract, ContractKind, Expr, ExprKind, Idx, ModuleDecl, UnOp};

use crate::expr::Sig;
use crate::{header, ClockPort, Emitter};

/// SVA çıktı modu (`volt build --sva=...`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SvaMode {
    /// SVA üretilmez (yalnız RTL).
    None,
    /// Ayrı `build/formal/<modul>.sva` dosyası + bind (varsayılan).
    Separate,
    /// Özellikler SV modül gövdesine gömülür.
    Inline,
    /// Yosys-uyumlu gömülü immediate assertion'lar (`volt verify`).
    ///
    /// Yosys'in Verilog ön ucu adlandırılmış `property/endproperty`
    /// bloklarını AYRIŞTIRAMAZ (TOK_PROPERTY sözdizimi hatası) —
    /// hdlc/formal imajıyla doğrulandı. Bu mod aynı kontratları
    /// `always @(edge) if (!reset) assert (ifade); // volt:<ad>`
    /// kalıbına indirger; satır sonu işareti sby FAIL logunu Volt
    /// kontratına geri eşlemek için kullanılır.
    Immediate,
}

/// Ayrı modda tek modülün SVA dosyası.
#[derive(Debug, Clone)]
pub struct SvaFile {
    /// Bind hedefi olan Volt modülü (ör. "Uart").
    pub module_name: String,
    /// Kontrol modülünün adı (ör. "uart_sva").
    pub checker_name: String,
    pub content: String,
}

/// Üretilen tek bir SVA property'sinin kimliği (F4b, `volt verify`).
///
/// sby FAIL logundaki `assert property (inv_0);` satırını Volt
/// kaynağındaki kontrata geri bağlamak için kullanılır: property adı →
/// kontratın anahtar kelimesi + kaynak konumu.
#[derive(Debug, Clone)]
pub struct SvaProp {
    /// Kontratın ait olduğu Volt modülü (ör. "Counter").
    pub module_name: String,
    /// Property adı (ör. "inv_0").
    pub name: String,
    /// Kontrat anahtar kelimesi ("invariant", "ensures", ...).
    pub keyword: &'static str,
    /// Kontrat ifadesinin Volt kaynağındaki konumu.
    pub span: volt_span::Span,
}

/// `1'b0/1'b1` bağlamı: kontrat ifadeleri 1-bit boolean'dır.
const ONE_BIT: Option<Sig> = Some(Sig {
    width: 1,
    signed: false,
});

impl<'a> Emitter<'a> {
    /// Modülün tüm kontratlarını property bloklarına çevirir; kontrat
    /// ya da saat yoksa None. `indent` blokların taban girintisi.
    pub(crate) fn sva_properties(
        &mut self,
        module: &'a ModuleDecl,
        clocks: &[ClockPort],
        indent: usize,
    ) -> Option<String> {
        if module.contracts.is_empty() {
            return None;
        }
        let clock = clocks.first()?.clone();
        let source_name = self.source_name;
        let ind = " ".repeat(indent);
        let edge = match clock.info.edge {
            ClockEdge::Negedge => "negedge",
            _ => "posedge",
        };
        let event = if clock.info.reset.is_none() {
            format!("@({edge} {})", clock.name)
        } else {
            format!(
                "@({edge} {}) disable iff ({})",
                clock.name,
                clock.info.reset.condition()
            )
        };

        let mut counters = [0u32; 6];
        let mut blocks = Vec::new();
        // F4b: BMC başlangıç durumu kısıtsızdır — reset'i ilk döngüde
        // varsaymak standart formal kalıbıdır; yoksa çözücü sıfırlanmamış
        // register'lı sahte bir 0. adım karşı örneği üretir. Reset'siz
        // alanlarda varsayım da üretilmez (başlangıç tasarımın sorunudur).
        if !clock.info.reset.is_none() {
            blocks.push(format!(
                "{ind}// formal: assume reset in the first cycle (BMC init)\n\
                 {ind}initial assume ({});",
                clock.info.reset.condition()
            ));
        }
        for c in &module.contracts {
            let (prefix, verb) = sva_construct(c.kind);
            let slot = kind_slot(c.kind);
            let name = format!("{prefix}_{}", counters[slot]);
            counters[slot] += 1;
            // F4b: property adı → kontrat eşlemesi (sby FAIL yorumu).
            self.sva_props.push(SvaProp {
                module_name: module.name.text.clone(),
                name: name.clone(),
                keyword: contract_keyword(c.kind),
                span: self.ast.exprs[c.expr].span,
            });
            let line = line_of(self.source, self.ast.exprs[c.expr].span.start);
            let expr = self.sva_expr(c);
            blocks.push(format!(
                "{ind}// {kw} from {source_name}:{line}\n\
                 {ind}property {name};\n\
                 {ind}    {event}\n\
                 {ind}    {expr};\n\
                 {ind}endproperty\n\
                 {ind}{verb} property ({name});",
                kw = contract_keyword(c.kind),
            ));
        }
        Some(blocks.join("\n\n"))
    }

    /// Immediate assertion bloğu (SvaMode::Immediate, F4b).
    ///
    /// Kontrat başına bir `always` üretilir; assertion satırı
    /// `// volt:<ad>` işareti taşır (sby konum → kontrat eşlemesi).
    /// `ensures`'ün `!a || b` deseni tek döngülük örtüşmeli gerektirme
    /// ile eşdeğer olduğundan burada dönüştürülmeden bırakılır.
    pub(crate) fn sva_immediate(
        &mut self,
        module: &'a ModuleDecl,
        clocks: &[ClockPort],
        indent: usize,
    ) -> Option<String> {
        if module.contracts.is_empty() {
            return None;
        }
        let clock = clocks.first()?.clone();
        let source_name = self.source_name;
        let ind = " ".repeat(indent);
        let edge = match clock.info.edge {
            ClockEdge::Negedge => "negedge",
            _ => "posedge",
        };

        let mut counters = [0u32; 6];
        let mut blocks = Vec::new();
        // BMC başlangıç durumu kısıtsız — ilk döngüde reset varsayılır
        // (sva_properties ile aynı gerekçe).
        if !clock.info.reset.is_none() {
            blocks.push(format!(
                "{ind}// formal: assume reset in the first cycle (BMC init)\n\
                 {ind}initial assume ({});",
                clock.info.reset.condition()
            ));
        }
        for c in &module.contracts {
            let (prefix, verb) = sva_construct(c.kind);
            let slot = kind_slot(c.kind);
            let name = format!("{prefix}_{}", counters[slot]);
            counters[slot] += 1;
            self.sva_props.push(SvaProp {
                module_name: module.name.text.clone(),
                name: name.clone(),
                keyword: contract_keyword(c.kind),
                span: self.ast.exprs[c.expr].span,
            });
            let line = line_of(self.source, self.ast.exprs[c.expr].span.start);
            let expr = self.emit_expr(c.expr, ONE_BIT);
            let stmt = if clock.info.reset.is_none() {
                format!("{verb} ({expr}); // volt:{name}")
            } else {
                format!(
                    "if (!({})) {verb} ({expr}); // volt:{name}",
                    clock.info.reset.condition()
                )
            };
            blocks.push(format!(
                "{ind}// {kw} from {source_name}:{line}\n\
                 {ind}always @({edge} {})\n\
                 {ind}    {stmt}",
                clock.name,
                kw = contract_keyword(c.kind),
            ));
        }
        Some(blocks.join("\n\n"))
    }

    /// Kontrat ifadesi → SV metni. Üst düzey `a -> b` implikasyonu her
    /// kontrat türünde, `ensures`'ün eski `!a || b` deseni geriye uyumluluk
    /// için örtüşmeli gerektirmeye (`a |-> b`) çevrilir (ADR-0034).
    fn sva_expr(&mut self, c: &Contract) -> String {
        if let ExprKind::Binary {
            op: BinOp::Imp,
            lhs,
            rhs,
        } = &self.ast.exprs[c.expr].kind
        {
            let (lhs, rhs) = (*lhs, *rhs);
            let a = self.emit_expr(lhs, ONE_BIT);
            let b = self.emit_expr(rhs, ONE_BIT);
            return format!("{a} |-> {b}");
        }
        if c.kind == ContractKind::Ensures {
            if let ExprKind::Binary {
                op: BinOp::Or,
                lhs,
                rhs,
            } = &self.ast.exprs[c.expr].kind
            {
                let (lhs, rhs) = (*lhs, *rhs);
                if let ExprKind::Unary {
                    op: UnOp::Not,
                    operand,
                } = &self.ast.exprs[lhs].kind
                {
                    let operand = *operand;
                    let a = self.emit_expr(operand, ONE_BIT);
                    let b = self.emit_expr(rhs, ONE_BIT);
                    return format!("{a} |-> {b}");
                }
            }
        }
        self.emit_expr(c.expr, ONE_BIT)
    }

    /// Ayrı mod: kontrol modülü + bind. Portlar `(.*)` ile hedef modül
    /// kapsamında ada göre bağlanır — saat, reset ve kontratlarda geçen
    /// tüm sinyaller giriş portu olur.
    pub(crate) fn sva_file(
        &mut self,
        module: &'a ModuleDecl,
        clocks: &[ClockPort],
    ) -> Option<SvaFile> {
        let props = self.sva_properties(module, clocks, 4)?;
        let clock = clocks.first()?.clone();
        let module_name = module.name.text.clone();
        let checker_name = format!("{}_sva", module_name.to_lowercase());

        let mut names: Vec<String> = vec![clock.name.clone()];
        if !clock.info.reset.is_none() {
            names.push(clock.info.reset.port_name().to_string());
        }
        let mut referenced = Vec::new();
        for c in &module.contracts {
            collect_signal_names(self.ast, c.expr, &mut referenced);
        }
        for n in referenced {
            if !names.contains(&n) && self.symbols.contains_key(&n) {
                names.push(n);
            }
        }

        let entries: Vec<(String, String)> = names
            .iter()
            .map(|n| {
                let ty = self
                    .symbols
                    .get(n)
                    .map(|s| s.decl_type())
                    .unwrap_or_else(|| "logic".to_string());
                (ty, n.clone())
            })
            .collect();
        let ty_width = entries.iter().map(|(ty, _)| ty.len()).max().unwrap_or(5);
        let count = entries.len();
        let ports = entries
            .iter()
            .enumerate()
            .map(|(i, (ty, name))| {
                let comma = if i + 1 < count { "," } else { "" };
                format!("    input {ty:<ty_width$} {name}{comma}")
            })
            .collect::<Vec<_>>()
            .join("\n");

        let mut content = header(self.source_name);
        content.push('\n');
        content.push_str(&format!("module {checker_name} (\n{ports}\n);\n\n"));
        content.push_str(&props);
        content.push_str("\n\nendmodule\n\n");
        content.push_str(&format!(
            "bind {module_name} {checker_name} sva_inst (.*);\n"
        ));
        content.push_str("`default_nettype wire\n");

        Some(SvaFile {
            module_name,
            checker_name,
            content,
        })
    }
}

/// Kontrat türü → (isim öneki, SVA fiili).
fn sva_construct(kind: ContractKind) -> (&'static str, &'static str) {
    match kind {
        ContractKind::Invariant => ("inv", "assert"),
        ContractKind::Ensures => ("ens", "assert"),
        ContractKind::Assert => ("ast", "assert"),
        ContractKind::Requires => ("req", "assume"),
        ContractKind::Assume => ("asm", "assume"),
        ContractKind::Cover => ("cov", "cover"),
    }
}

fn kind_slot(kind: ContractKind) -> usize {
    match kind {
        ContractKind::Requires => 0,
        ContractKind::Ensures => 1,
        ContractKind::Invariant => 2,
        ContractKind::Cover => 3,
        ContractKind::Assert => 4,
        ContractKind::Assume => 5,
    }
}

fn contract_keyword(kind: ContractKind) -> &'static str {
    match kind {
        ContractKind::Requires => "requires",
        ContractKind::Ensures => "ensures",
        ContractKind::Invariant => "invariant",
        ContractKind::Cover => "cover",
        ContractKind::Assert => "assert",
        ContractKind::Assume => "assume",
    }
}

/// Bayt konumunun 1-tabanlı satır numarası.
fn line_of(source: &str, byte: u32) -> usize {
    let end = (byte as usize).min(source.len());
    source.as_bytes()[..end]
        .iter()
        .filter(|&&b| b == b'\n')
        .count()
        + 1
}

/// Kontrat ifadesindeki tek segmentli isimler (ilk kullanım sırasıyla).
fn collect_signal_names(ast: &volt_ast::SourceFile, expr: Idx<Expr>, out: &mut Vec<String>) {
    match &ast.exprs[expr].kind {
        ExprKind::Path(p) if p.segments.len() == 1 => {
            let name = p.segments[0].text.clone();
            if !out.contains(&name) {
                out.push(name);
            }
        }
        ExprKind::Binary { lhs, rhs, .. } => {
            collect_signal_names(ast, *lhs, out);
            collect_signal_names(ast, *rhs, out);
        }
        ExprKind::Unary { operand, .. } => collect_signal_names(ast, *operand, out),
        ExprKind::Index { base, index } => {
            collect_signal_names(ast, *base, out);
            collect_signal_names(ast, *index, out);
        }
        ExprKind::Range { base, hi, lo } => {
            collect_signal_names(ast, *base, out);
            collect_signal_names(ast, *hi, out);
            collect_signal_names(ast, *lo, out);
        }
        ExprKind::Field { base, .. } => collect_signal_names(ast, *base, out),
        ExprKind::Call { callee, args } => {
            collect_signal_names(ast, *callee, out);
            for &a in args {
                collect_signal_names(ast, a, out);
            }
        }
        ExprKind::Cast { expr: inner, .. } => collect_signal_names(ast, *inner, out),
        ExprKind::If {
            cond,
            then_expr,
            else_expr,
        } => {
            collect_signal_names(ast, *cond, out);
            collect_signal_names(ast, *then_expr, out);
            collect_signal_names(ast, *else_expr, out);
        }
        _ => {}
    }
}
