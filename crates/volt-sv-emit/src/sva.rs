//! Kontratlardan SVA üretimi (F4a ADIM 2-3).
//!
//! Eşleme (Volt-Butunlesik-Mimari-v3.md kontrat tablosu):
//! invariant/ensures/assert → `assert property`, requires/assume →
//! `assume property`, cover → `cover property`. Saat ve `disable iff`
//! modülün ilk saat portunun alanından gelir; saatsiz modülde SVA
//! üretilmez (formel araçlar saat ister) — `volt verify` bunu E5005 yapar.
//!
//! requires/assume yalnız modül formal tepe iken `assume`'dur; modül bir
//! üst modülün örneğiyken `assert` olur (`VOLT_SUB_<modül>` makrosu,
//! ADR-0097). `volt verify` (Immediate) makroyu görev başına tanımlar;
//! `--emit=sva` (ayrı/gömülü) aynı makroyu kullanır ve dosya başına ticari
//! araçta nasıl tanımlanacağını yazar.
//!
//! Varsayılan çıktı ayrı `.sva` dosyasıdır ve hedef modüle `bind` ile
//! bağlanır; `(.*)` bağlama modül kapsamında ada göre çözüldüğünden
//! kontrol modülünün portları register'lara da erişebilir.

use std::collections::HashMap;

use volt_ast::{
    BinOp, ClockEdge, Contract, ContractKind, Expr, ExprKind, Idx, ItemKind, ModuleDecl, UnOp,
};

use crate::expr::Sig;
use crate::reach::{instance_children, instance_subtree};
use crate::SourceText;
use crate::{header, ClockPort, Emitter};
use volt_span::{FileId, Span};

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
    /// Simülasyon izleyicileri (`volt test`, ADR-0064): Immediate
    /// kalıbının aynısı, ama `assert`/`assume`/`cover` yerine testbench'in
    /// DPI geri çağrıları (`volt_contract_fail`, `volt_cover_report`) —
    /// Verilator'un `$stop`'u yerine test düşer ve kimlik kaynağa eşlenir.
    /// Formal varsayımlar (`initial assume`) ve init blokları üretilmez;
    /// RTL kısmı `None` modundan farksızdır.
    Simulation,
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
    /// Yerleşik primitif kontratıysa primitifin adı (`AsyncFifo`); o
    /// zaman `span` örneğin konumudur, kontrat metni kaynakta yoktur
    /// (ADR-0064 simülasyon raporu bunu ayrı anlatır).
    pub primitive: Option<&'static str>,
    /// Derleyicinin ürettiği kontratın kökeni (Handshake, @mmio, FSM,
    /// sayaç); raporların "generated from" satırı (ADR-0066 §4).
    pub auto: Option<AutoProp>,
}

/// Kontratı olup saat portu olmayan modül (ADR-0097, E5005 girdisi).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnclockedContracts {
    pub module: String,
    /// İlk kontratın ifadesi (tanının birincil konumu).
    pub span: volt_span::Span,
    /// Modülün kontrat sayısı.
    pub count: usize,
}

/// Otomatik kontratın kökeni — kullanıcı kontratı kaynakta yazmadı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoProp {
    /// Kural etiketi (`FSM transition`, `counter bound`, ...).
    pub rule: &'static str,
    /// Kontratın Volt sözdizimiyle metni.
    pub text: String,
    /// Kökenin kısa anlatımı (`match on state_r`).
    pub subject: String,
    /// Kontratı doğuran yapının konumu.
    pub from: volt_span::Span,
    /// Formal koşumda cover'ın yapısal erişilebilirliği (ADR-0086).
    /// Yalnız reset'li alanda bilinir (başlangıç reset'le sabitlenir);
    /// reset'siz alanda başlangıç serbest, her zaman `Unknown`.
    pub reach: CoverReach,
}

/// Otomatik cover'ın formal koşumdaki yapısal erişilebilirliği (ADR-0086).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CoverReach {
    /// Bilinmiyor.
    #[default]
    Unknown,
    /// En az bu sby derinliğinde ulaşılabilir: yapısal kenar alt sınırı +
    /// [`COVER_HARNESS_STEPS`].
    MinDepth(u32),
    /// Hiçbir derinlikte ulaşılamaz (resetten yol yok).
    Never,
}

/// Formal koşumun (`initial assume (rst)` + saat kenarında örneklenen
/// `if (!rst) cover (...)`) ilk saat kenarından sonraki değere eklediği
/// sby adımı. Ölçüldü (ADR-0086): reset değeri zaten sınırda olan sayaç
/// derinlik 3'te, 5 artışlık sayaç 8'de, sabit yüklemeli 6 adımlık sayaç
/// 9'da ulaşılır; senkron ve asenkron (negedge, aktif düşük) reset aynı.
/// Çok saatli koşum (`multiclock on`) kenar başına daha çok adım
/// harcar — değer yine alt sınırdır.
pub const COVER_HARNESS_STEPS: u32 = 3;

/// `1'b0/1'b1` bağlamı: kontrat ifadeleri 1-bit boolean'dır.
pub(crate) const ONE_BIT: Option<Sig> = Some(Sig {
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
        let ind = " ".repeat(indent);
        let edge = match clock.info.edge {
            ClockEdge::Negedge => "negedge",
            // Kenarsız (`clock: none`) alan da posedge yazılır — mevcut davranış.
            ClockEdge::Posedge | ClockEdge::None => "posedge",
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
            let prop = self.contract_prop(module, &name, c, !clock.info.reset.is_none());
            self.sva_props.push(prop);
            let comment = self.contract_comment(c, &ind);
            let expr = self.sva_expr(c);
            // ADR-0097: `volt verify` ile aynı makro — ticari araçta modül
            // tek başına doğrulanırken varsayım, üst bağlamda yükümlülük.
            let directive = if is_obligation(c.kind) {
                format!(
                    "`ifdef {}\n\
                     {ind}assert property ({name});\n\
                     `else\n\
                     {ind}{verb} property ({name});\n\
                     `endif",
                    sub_instance_macro(&module.name.text),
                )
            } else {
                format!("{ind}{verb} property ({name});")
            };
            blocks.push(format!(
                "{comment}\n\
                 {ind}property {name};\n\
                 {ind}    {event}\n\
                 {ind}    {expr};\n\
                 {ind}endproperty\n\
                 {directive}",
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
        let Some(clock) = clocks.first().cloned() else {
            // ADR-0097: saat kenarı olmayan kontrat formal koşudan sessizce
            // düşerdi; doğrulama durur.
            self.unclocked_contracts(module);
            return None;
        };
        let ind = " ".repeat(indent);
        let edge = match clock.info.edge {
            ClockEdge::Negedge => "negedge",
            // Kenarsız (`clock: none`) alan da posedge yazılır — mevcut davranış.
            ClockEdge::Posedge | ClockEdge::None => "posedge",
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
        // ADR-0040: prev() yardımcı register zincirleri (Yosys $past bilmez).
        if let Some(block) = self.past_reg_block(module, &clock, indent) {
            blocks.push(block);
        }
        for c in &module.contracts {
            let (prefix, verb) = sva_construct(c.kind);
            let slot = kind_slot(c.kind);
            let name = format!("{prefix}_{}", counters[slot]);
            counters[slot] += 1;
            let prop = self.contract_prop(module, &name, c, !clock.info.reset.is_none());
            self.sva_props.push(prop);
            let comment = self.contract_comment(c, &ind);
            let expr = self.emit_expr(c.expr, ONE_BIT);
            let stmt = |verb: &str| {
                if clock.info.reset.is_none() {
                    format!("{verb} ({expr}); // volt:{name}")
                } else {
                    format!(
                        "if (!({})) {verb} ({expr}); // volt:{name}",
                        clock.info.reset.condition()
                    )
                }
            };
            // ADR-0097: requires/assume modülün kendi görevinde varsayımdır;
            // modül bir örnekken onu süren üst modülün yükümlülüğüdür. Görev
            // tepesi olmayan modüllerin makrosunu `.sby` tanımlar.
            let body = if is_obligation(c.kind) {
                format!(
                    "`ifdef {}\n\
                     {ind}    {}\n\
                     `else\n\
                     {ind}    {}\n\
                     `endif",
                    sub_instance_macro(&module.name.text),
                    stmt("assert"),
                    stmt(verb),
                )
            } else {
                format!("{ind}    {}", stmt(verb))
            };
            blocks.push(format!(
                "{comment}\n\
                 {ind}always @({edge} {})\n\
                 {body}",
                clock.name,
            ));
        }
        Some(blocks.join("\n\n"))
    }

    /// ADR-0097: saat portu olmayan modülün kontratları formal koşuda
    /// örneklenemez. Kayıt sürücüde E5005 olur (erişilebilir modüller için).
    fn unclocked_contracts(&mut self, module: &ModuleDecl) {
        let Some(first) = module.contracts.first() else {
            return;
        };
        self.unclocked_contracts.push(UnclockedContracts {
            module: module.name.text.clone(),
            span: self.ast.exprs[first.expr].span,
            count: module.contracts.len(),
        });
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
        // Ayrı dosyanın kullandığı enum varyantları kendi localparam'larını
        // taşır; modülün kümesine karışmaz (UNUSEDPARAM, ADR-0074).
        let saved = std::mem::take(&mut self.enum_used);
        let props = self.sva_properties(module, clocks, 4);
        let sva_used = std::mem::replace(&mut self.enum_used, saved);
        let props = props?;
        let clock = clocks.first()?.clone();
        let module_name = module.name.text.clone();
        let checker_name = format!("{}_sva", module_name.to_lowercase());

        let mut names: Vec<String> = vec![clock.name.clone()];
        if !clock.info.reset.is_none() {
            names.push(clock.info.reset.signal().to_string());
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
                // Dizi sinyali: unpacked boyut isimden sonra (ADR-0035).
                let name = match self.array_dims.get(n) {
                    Some(len) => format!("{n} [0:{}]", len - 1),
                    None => n.clone(),
                };
                (ty, name)
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
        if module.contracts.iter().any(|c| is_obligation(c.kind)) {
            content.push_str(&obligation_rule_note(&module_name));
            content.push('\n');
        }
        // Verilator -Wall temizliği (ADR-0079): dosya adı build/formal/
        // <modül>.sva düzenindedir, kontrol modülü `<modül>_sva` adını bind
        // için taşır; portlar sinyalin tamamını gözler, özellik bir kısmını
        // okuyabilir.
        content.push_str(
            "// checker for bind: the file keeps the <module>.sva name and the ports\n\
             // observe whole signals of which a property may read only some bits\n\
             // verilator lint_off DECLFILENAME\n\
             // verilator lint_off UNUSEDSIGNAL\n",
        );
        content.push_str(&format!("module {checker_name} (\n{ports}\n);\n"));
        content
            .push_str("// verilator lint_on UNUSEDSIGNAL\n// verilator lint_on DECLFILENAME\n\n");
        if let Some(params) = self.enum_localparams(&sva_used, module.name.span) {
            content.push_str(&params);
            content.push_str("\n\n");
        }
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

impl Emitter<'_> {
    /// `--emit=sva` modül `.sv` dosyalarının başlık notları (ADR-0097):
    /// gömülü kipte yükümlülüklü modülün kendi makro kuralı, iki kipte de
    /// altında yükümlülüklü örnek bulunan modülün formal tepe makroları.
    /// Ayrı kipte kural `.sva` dosyasının başındadır (`sva_file`). Tepe
    /// notu aşağı doğru kapanıştan kurulur; çıktı kümesi süzülünce
    /// (ADR-0042 ek) tepe kalırsa altındakiler de kalır.
    pub(crate) fn obligation_notes(&self) -> HashMap<String, String> {
        if !matches!(self.sva_mode, SvaMode::Inline | SvaMode::Separate) {
            return HashMap::new();
        }
        let modules: Vec<&ModuleDecl> = self
            .ast
            .items
            .iter()
            .filter_map(|&i| {
                if let ItemKind::Module(m) = &self.ast.items_arena[i].kind {
                    Some(m)
                } else {
                    None
                }
            })
            .collect();
        // Saatsiz modül SVA üretmez (makrosu da yoktur).
        let obligated: Vec<&str> = modules
            .iter()
            .filter(|m| m.contracts.iter().any(|c| is_obligation(c.kind)))
            .filter(|m| !self.collect_clock_ports(m).is_empty())
            .map(|m| m.name.text.as_str())
            .collect();
        let children = instance_children(self.ast);
        let mut notes = HashMap::new();
        for m in &modules {
            let name = m.name.text.as_str();
            let mut note = String::new();
            if self.sva_mode == SvaMode::Inline && obligated.contains(&name) {
                note.push_str(&obligation_rule_note(name));
            }
            let below = instance_subtree(name, &children);
            let macros: Vec<String> = obligated
                .iter()
                .filter(|o| below.iter().any(|b| b == *o))
                .map(|o| sub_instance_macro(o))
                .collect();
            if !macros.is_empty() {
                if !note.is_empty() {
                    note.push('\n');
                }
                note.push_str(&formal_top_note(name, &macros));
            }
            if !note.is_empty() {
                notes.insert(name.to_string(), note);
            }
        }
        notes
    }

    /// Kontratın `SvaProp` kaydı (sby FAIL / sim izleyici eşlemesi).
    /// `has_reset`: kontratın saat alanında reset var mı? Yoksa formal
    /// başlangıç durumu serbesttir, cover derinlik alt sınırı bilinmez.
    pub(crate) fn contract_prop(
        &self,
        module: &ModuleDecl,
        name: &str,
        c: &Contract,
        has_reset: bool,
    ) -> SvaProp {
        SvaProp {
            module_name: module.name.text.clone(),
            name: name.to_string(),
            keyword: contract_keyword(c.kind),
            span: self.ast.exprs[c.expr].span,
            primitive: None,
            auto: c.auto.as_ref().map(|a| AutoProp {
                rule: a.rule.label(),
                text: a.text.clone(),
                subject: a.subject.clone(),
                from: a.from,
                reach: match a.reach {
                    _ if !has_reset => CoverReach::Unknown,
                    volt_ast::AutoReach::Unknown => CoverReach::Unknown,
                    volt_ast::AutoReach::AtLeast(n) => {
                        CoverReach::MinDepth(n.saturating_add(COVER_HARNESS_STEPS))
                    }
                    volt_ast::AutoReach::Never => CoverReach::Never,
                },
            }),
        }
    }

    /// Property üstündeki kaynak yorumu. Otomatik kontratta kural ve
    /// köken yazılır, konum kökenin konumudur (ADR-0066 §4).
    pub(crate) fn contract_comment(&self, c: &Contract, ind: &str) -> String {
        let kw = contract_keyword(c.kind);
        match &c.auto {
            None => {
                let (source_name, line) = self.location_of(self.ast.exprs[c.expr].span);
                format!("{ind}// {kw} from {source_name}:{line}")
            }
            Some(a) => {
                let (source_name, line) = self.location_of(a.from);
                format!(
                    "{ind}// {kw} (auto {}: {}) generated from {source_name}:{line} ({})",
                    a.rule.label(),
                    a.text,
                    a.subject
                )
            }
        }
    }
}

/// Kontrat türü → (isim öneki, SVA fiili).
pub(crate) fn sva_construct(kind: ContractKind) -> (&'static str, &'static str) {
    match kind {
        ContractKind::Invariant => ("inv", "assert"),
        ContractKind::Ensures => ("ens", "assert"),
        ContractKind::Assert => ("ast", "assert"),
        ContractKind::Requires => ("req", "assume"),
        ContractKind::Assume => ("asm", "assume"),
        ContractKind::Cover => ("cov", "cover"),
    }
}

/// Görev tepesi olmayan (bir üst modülün örneği olan) modülün makrosu
/// (ADR-0097). `volt verify` bunu görev başına `read -define` ile tanımlar;
/// tanımlıyken modülün `requires`/`assume` kontratları `assert` olur.
pub fn sub_instance_macro(module: &str) -> String {
    format!("VOLT_SUB_{module}")
}

/// Modülün kendi görevinde varsayılan, örnekken üst modülün yükümlülüğü
/// olan kontrat türü mü (ADR-0097)?
pub fn is_obligation(kind: ContractKind) -> bool {
    matches!(kind, ContractKind::Requires | ContractKind::Assume)
}

/// `--emit=sva` dosya başı notu (ADR-0097): `requires`/`assume`'u olan
/// modülün makrosu ve ticari araçta iki doğrulama biçimi. `volt verify`
/// makroyu `.sby`'de kendisi tanımlar; elle koşuda kullanıcı tanımlar.
/// Değişken uzunluklu adlar (modül, makro) satır sonunda durur; uzun adlı
/// modülde de metin satırları kırılmaz.
pub(crate) fn obligation_rule_note(module: &str) -> String {
    let mac = sub_instance_macro(module);
    format!(
        "// Contract obligations (ADR-0097) of module {module}\n\
         //   Its requires/assume properties switch on the macro {mac}\n\
         //   Verified on its own (the module is the formal top): leave the\n\
         //     macro undefined; requires/assume are assumptions on its inputs.\n\
         //   Verified inside a parent (the parent is the formal top): define\n\
         //     the macro; requires/assume become assertions the parent must meet.\n\
         //     vlog/vcs/xrun: +define+{mac}\n\
         //     Yosys:         read -define {mac}\n"
    )
}

/// `--emit=sva` dosya başı notu (ADR-0097): altında yükümlülüklü örnek
/// bulunan modül formal tepe olduğunda tanımlanacak makrolar — `volt
/// verify`'ın o görevde tanımladığı küme (`verify_plan.rs`).
pub(crate) fn formal_top_note(module: &str, macros: &[String]) -> String {
    format!(
        "// Formal top (ADR-0097): module {module}\n\
         //   When this module is the formal top, define the macros of the\n\
         //   instances below; their requires/assume are then checked as its\n\
         //   obligations.\n\
         //     vlog/vcs/xrun: +define+{}\n\
         //     Yosys:         read -define {}\n",
        macros.join("+"),
        macros.join(" "),
    )
}

pub(crate) fn kind_slot(kind: ContractKind) -> usize {
    match kind {
        ContractKind::Requires => 0,
        ContractKind::Ensures => 1,
        ContractKind::Invariant => 2,
        ContractKind::Cover => 3,
        ContractKind::Assert => 4,
        ContractKind::Assume => 5,
    }
}

pub(crate) fn contract_keyword(kind: ContractKind) -> &'static str {
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
impl<'a> Emitter<'a> {
    /// Span'in dosyası — birimde yoksa ana dosya (tek dosya modu).
    fn source_of(&self, file: FileId) -> &SourceText<'a> {
        self.sources
            .iter()
            .find(|s| s.file == file)
            .or_else(|| self.sources.first())
            .expect("emit: en az bir kaynak dosya")
    }

    pub(crate) fn source_name_of(&self, file: FileId) -> &'a str {
        self.source_of(file).name
    }

    /// SVA yorumları için `dosya:satır` (ADR-0042: dosya span'e göre).
    pub(crate) fn location_of(&self, span: Span) -> (&'a str, usize) {
        let src = self.source_of(span.file);
        (src.name, line_of(src.text, span.start))
    }
}

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
        // Sınanan, muhafızlar ve kollar (ADR-0083: kontratta üçlü zincir).
        ExprKind::Match { .. } => {
            for c in volt_ast::visit::expr_children(&ast.exprs[expr].kind) {
                collect_signal_names(ast, c, out);
            }
        }
        ExprKind::Concat(parts) => {
            for &(p, _) in parts {
                collect_signal_names(ast, p, out);
            }
        }
        // Yapraklar ve inilmeyen düğümler (PartSelect dahil) — mevcut davranış korunur.
        ExprKind::IntLit { .. }
        | ExprKind::BoolLit(_)
        | ExprKind::StringLit(_)
        | ExprKind::Path(_)
        | ExprKind::PartSelect { .. }
        | ExprKind::StructLit { .. }
        | ExprKind::ArrayLit(_)
        | ExprKind::TupleLit(_)
        | ExprKind::Todo { .. }
        | ExprKind::Error => {}
    }
}
